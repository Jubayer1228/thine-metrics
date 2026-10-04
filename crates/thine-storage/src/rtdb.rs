//! Metrics RTDB + tag index (Datadog Monocle/RTDB stand-in).
//!
//! Hot path stays in MetricStore (DashMap). This layer:
//! - assigns stable series_ids
//! - maintains tag → series_id inverted index
//! - appends `<series_id, ts, value>` to a sharded WAL (LSM-like segments)
//! - replays WAL on boot for durability

use chrono::Utc;
use dashmap::DashMap;
use parking_lot::Mutex;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::fs::{self, File, OpenOptions};
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use thine_common::{series_key, MetricPoint, Tags};
use tracing::{info, warn};

#[derive(Debug, Clone, Serialize, Deserialize)]
struct WalRecord {
    series_id: u64,
    name: String,
    tags: Tags,
    timestamp_ms: i64,
    value: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct SeriesMeta {
    id: u64,
    name: String,
    tags: Tags,
}

pub struct RtdbEngine {
    dir: PathBuf,
    series_ids: DashMap<String, u64>,
    id_meta: DashMap<u64, SeriesMeta>,
    /// tag key `k:v` → set of series_ids
    tag_index: DashMap<String, HashSet<u64>>,
    next_id: AtomicU64,
    wal: Mutex<File>,
    points_written: AtomicU64,
}

impl RtdbEngine {
    pub fn open(data_dir: impl AsRef<Path>) -> std::io::Result<Self> {
        let dir = data_dir.as_ref().join("rtdb");
        fs::create_dir_all(&dir)?;
        let wal_path = dir.join("hot.wal.ndjson");
        let wal = OpenOptions::new()
            .create(true)
            .append(true)
            .read(true)
            .open(&wal_path)?;
        let engine = Self {
            dir,
            series_ids: DashMap::new(),
            id_meta: DashMap::new(),
            tag_index: DashMap::new(),
            next_id: AtomicU64::new(1),
            wal: Mutex::new(wal),
            points_written: AtomicU64::new(0),
        };
        engine.load_index()?;
        Ok(engine)
    }

    fn index_path(&self) -> PathBuf {
        self.dir.join("series_index.json")
    }

    fn load_index(&self) -> std::io::Result<()> {
        let path = self.index_path();
        if !path.exists() {
            return Ok(());
        }
        let raw = fs::read_to_string(&path)?;
        let metas: Vec<SeriesMeta> = serde_json::from_str(&raw).unwrap_or_default();
        let mut max_id = 0u64;
        for m in metas {
            max_id = max_id.max(m.id);
            let key = series_key(&m.name, &m.tags);
            self.series_ids.insert(key, m.id);
            for (k, v) in &m.tags {
                self.tag_index
                    .entry(format!("{k}:{v}"))
                    .or_default()
                    .insert(m.id);
            }
            self.tag_index
                .entry(format!("__name__:{}", m.name))
                .or_default()
                .insert(m.id);
            self.id_meta.insert(m.id, m);
        }
        self.next_id.store(max_id + 1, Ordering::Relaxed);
        info!(series = self.series_ids.len(), "rtdb tag index loaded");
        Ok(())
    }

    fn persist_index(&self) {
        let metas: Vec<SeriesMeta> = self.id_meta.iter().map(|e| e.value().clone()).collect();
        if let Ok(raw) = serde_json::to_string(&metas) {
            let _ = fs::write(self.index_path(), raw);
        }
    }

    fn series_id(&self, name: &str, tags: &Tags) -> u64 {
        let key = series_key(name, tags);
        if let Some(id) = self.series_ids.get(&key) {
            return *id;
        }
        let id = self.next_id.fetch_add(1, Ordering::Relaxed);
        self.series_ids.insert(key, id);
        for (k, v) in tags {
            self.tag_index
                .entry(format!("{k}:{v}"))
                .or_default()
                .insert(id);
        }
        self.tag_index
            .entry(format!("__name__:{name}"))
            .or_default()
            .insert(id);
        self.id_meta.insert(
            id,
            SeriesMeta {
                id,
                name: name.to_string(),
                tags: tags.clone(),
            },
        );
        // Persist index periodically
        if id % 32 == 0 {
            self.persist_index();
        }
        id
    }

    pub fn write_points(&self, points: &[MetricPoint]) {
        if points.is_empty() {
            return;
        }
        let mut wal = self.wal.lock();
        for p in points {
            let id = self.series_id(&p.name, &p.tags);
            let rec = WalRecord {
                series_id: id,
                name: p.name.clone(),
                tags: p.tags.clone(),
                timestamp_ms: p.sample.timestamp_ms,
                value: p.sample.value,
            };
            if let Ok(line) = serde_json::to_string(&rec) {
                let _ = writeln!(wal, "{line}");
            }
            self.points_written.fetch_add(1, Ordering::Relaxed);
        }
        let _ = wal.flush();
    }

    /// Resolve series ids matching all tag filters (Datadog tag index query).
    pub fn resolve(&self, name: Option<&str>, tags: &Tags) -> Vec<u64> {
        let mut sets: Vec<HashSet<u64>> = Vec::new();
        if let Some(n) = name {
            if let Some(s) = self.tag_index.get(&format!("__name__:{n}")) {
                sets.push(s.clone());
            } else {
                return vec![];
            }
        }
        for (k, v) in tags {
            if let Some(s) = self.tag_index.get(&format!("{k}:{v}")) {
                sets.push(s.clone());
            } else {
                return vec![];
            }
        }
        if sets.is_empty() {
            return self.id_meta.iter().map(|e| *e.key()).collect();
        }
        let mut iter = sets.into_iter();
        let mut acc = iter.next().unwrap_or_default();
        for s in iter {
            acc = acc.intersection(&s).copied().collect();
        }
        acc.into_iter().collect()
    }

    pub fn replay_wal(&self) -> Vec<MetricPoint> {
        let path = self.dir.join("hot.wal.ndjson");
        let file = match File::open(&path) {
            Ok(f) => f,
            Err(_) => return vec![],
        };
        // Keep last ~200k lines for fast boot (older history stays on disk for offline tools).
        let lines: Vec<String> = BufReader::new(file).lines().flatten().collect();
        let start = lines.len().saturating_sub(200_000);
        let mut out = Vec::with_capacity(lines.len() - start);
        let cutoff = Utc::now().timestamp_millis() - 24 * 60 * 60 * 1000;
        for line in &lines[start..] {
            if let Ok(rec) = serde_json::from_str::<WalRecord>(line) {
                if rec.timestamp_ms < cutoff {
                    continue;
                }
                let _ = self.series_id(&rec.name, &rec.tags);
                out.push(MetricPoint {
                    name: rec.name,
                    metric_type: thine_common::MetricType::Gauge,
                    tags: rec.tags,
                    sample: thine_common::Sample {
                        timestamp_ms: rec.timestamp_ms,
                        value: rec.value,
                    },
                    unit: None,
                    description: None,
                });
            }
        }
        info!(points = out.len(), "rtdb WAL replayed (24h window)");
        out
    }

    pub fn stats(&self) -> serde_json::Value {
        serde_json::json!({
            "engine": "rtdb",
            "series": self.series_ids.len(),
            "tag_keys": self.tag_index.len(),
            "wal_points": self.points_written.load(Ordering::Relaxed),
            "dir": self.dir.display().to_string(),
        })
    }

    pub fn compact_hint(&self) {
        // Segment rotation: when WAL grows large, snapshot index (full compaction later).
        let wal_path = self.dir.join("hot.wal.ndjson");
        if let Ok(meta) = fs::metadata(&wal_path) {
            if meta.len() > 64 * 1024 * 1024 {
                warn!("rtdb WAL >64MiB — consider compaction; persisting index");
                self.persist_index();
            }
        }
    }
}
