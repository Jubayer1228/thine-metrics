//! Husky-style event/log store: writers build fragments → object dir + catalog metadata.
//! Query scans recent hot ring + relevant fragments (compute ↔ storage decoupled).

use chrono::Utc;
use parking_lot::RwLock;
use serde::{Deserialize, Serialize};
use std::collections::VecDeque;
use std::fs::{self, File, OpenOptions};
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use thine_common::{IntakeEvent, IntakeLog, Tags};
use tracing::info;
use uuid::Uuid;

const MAX_HOT: usize = 20_000;
const FRAGMENT_FLUSH: usize = 500;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HuskyLog {
    pub timestamp_ms: i64,
    pub level: String,
    pub service: String,
    pub message: String,
    #[serde(default)]
    pub attrs: Tags,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HuskyEvent {
    pub timestamp_ms: i64,
    pub title: String,
    pub text: String,
    pub alert_type: String,
    pub tags: Tags,
    pub source: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct FragmentMeta {
    id: String,
    kind: String, // logs | events
    path: String,
    count: usize,
    min_ts: i64,
    max_ts: i64,
    created_at_ms: i64,
}

pub struct HuskyStore {
    dir: PathBuf,
    catalog: RwLock<Vec<FragmentMeta>>,
    hot_logs: RwLock<VecDeque<HuskyLog>>,
    hot_events: RwLock<VecDeque<HuskyEvent>>,
    pending_logs: RwLock<Vec<HuskyLog>>,
    pending_events: RwLock<Vec<HuskyEvent>>,
    fragments_written: AtomicU64,
}

impl HuskyStore {
    pub fn open(data_dir: impl AsRef<Path>) -> std::io::Result<Self> {
        let dir = data_dir.as_ref().join("husky");
        fs::create_dir_all(dir.join("fragments"))?;
        let store = Self {
            dir,
            catalog: RwLock::new(Vec::new()),
            hot_logs: RwLock::new(VecDeque::with_capacity(1024)),
            hot_events: RwLock::new(VecDeque::with_capacity(512)),
            pending_logs: RwLock::new(Vec::new()),
            pending_events: RwLock::new(Vec::new()),
            fragments_written: AtomicU64::new(0),
        };
        store.load_catalog()?;
        store.warm_hot_from_fragments()?;
        Ok(store)
    }

    fn catalog_path(&self) -> PathBuf {
        self.dir.join("catalog.json")
    }

    fn load_catalog(&self) -> std::io::Result<()> {
        let path = self.catalog_path();
        if !path.exists() {
            return Ok(());
        }
        let raw = fs::read_to_string(path)?;
        let metas: Vec<FragmentMeta> = serde_json::from_str(&raw).unwrap_or_default();
        *self.catalog.write() = metas;
        info!(fragments = self.catalog.read().len(), "husky catalog loaded");
        Ok(())
    }

    fn persist_catalog(&self) {
        let metas = self.catalog.read().clone();
        if let Ok(raw) = serde_json::to_string_pretty(&metas) {
            let _ = fs::write(self.catalog_path(), raw);
        }
    }

    fn warm_hot_from_fragments(&self) -> std::io::Result<()> {
        let metas = self.catalog.read().clone();
        for meta in metas.iter().rev().take(8) {
            let path = self.dir.join(&meta.path);
            if !path.exists() {
                continue;
            }
            let file = File::open(path)?;
            for line in BufReader::new(file).lines().flatten() {
                if meta.kind == "logs" {
                    if let Ok(l) = serde_json::from_str::<HuskyLog>(&line) {
                        let mut hot = self.hot_logs.write();
                        hot.push_back(l);
                        while hot.len() > MAX_HOT {
                            hot.pop_front();
                        }
                    }
                } else if let Ok(e) = serde_json::from_str::<HuskyEvent>(&line) {
                    let mut hot = self.hot_events.write();
                    hot.push_back(e);
                    while hot.len() > MAX_HOT / 2 {
                        hot.pop_front();
                    }
                }
            }
        }
        Ok(())
    }

    pub fn ingest_logs(&self, logs: Vec<IntakeLog>) {
        if logs.is_empty() {
            return;
        }
        let mapped: Vec<HuskyLog> = logs
            .into_iter()
            .map(|l| HuskyLog {
                timestamp_ms: l.timestamp_ms,
                level: l.level,
                service: l.service,
                message: l.message,
                attrs: l.attrs,
            })
            .collect();
        {
            let mut hot = self.hot_logs.write();
            for l in &mapped {
                hot.push_back(l.clone());
            }
            while hot.len() > MAX_HOT {
                hot.pop_front();
            }
        }
        let mut pending = self.pending_logs.write();
        pending.extend(mapped);
        if pending.len() >= FRAGMENT_FLUSH {
            let batch = std::mem::take(&mut *pending);
            drop(pending);
            self.flush_fragment("logs", &batch);
        }
    }

    pub fn ingest_events(&self, events: Vec<IntakeEvent>) {
        if events.is_empty() {
            return;
        }
        let mapped: Vec<HuskyEvent> = events
            .into_iter()
            .map(|e| HuskyEvent {
                timestamp_ms: e.timestamp_ms,
                title: e.title,
                text: e.text,
                alert_type: e.alert_type,
                tags: e.tags,
                source: e.source,
            })
            .collect();
        {
            let mut hot = self.hot_events.write();
            for e in &mapped {
                hot.push_back(e.clone());
            }
            while hot.len() > MAX_HOT / 2 {
                hot.pop_front();
            }
        }
        let mut pending = self.pending_events.write();
        pending.extend(mapped);
        if pending.len() >= FRAGMENT_FLUSH / 2 {
            let batch = std::mem::take(&mut *pending);
            drop(pending);
            self.flush_events_fragment(&batch);
        }
    }

    fn flush_fragment(&self, kind: &str, rows: &[HuskyLog]) {
        if rows.is_empty() {
            return;
        }
        let id = Uuid::new_v4().to_string();
        let rel = format!("fragments/{id}.ndjson");
        let path = self.dir.join(&rel);
        if let Ok(mut f) = OpenOptions::new().create(true).write(true).open(&path) {
            let mut min_ts = i64::MAX;
            let mut max_ts = i64::MIN;
            for r in rows {
                min_ts = min_ts.min(r.timestamp_ms);
                max_ts = max_ts.max(r.timestamp_ms);
                let _ = writeln!(f, "{}", serde_json::to_string(r).unwrap_or_default());
            }
            let _ = f.flush();
            self.catalog.write().push(FragmentMeta {
                id,
                kind: kind.into(),
                path: rel,
                count: rows.len(),
                min_ts,
                max_ts,
                created_at_ms: Utc::now().timestamp_millis(),
            });
            self.fragments_written.fetch_add(1, Ordering::Relaxed);
            self.persist_catalog();
        }
    }

    fn flush_events_fragment(&self, rows: &[HuskyEvent]) {
        if rows.is_empty() {
            return;
        }
        let id = Uuid::new_v4().to_string();
        let rel = format!("fragments/{id}.ndjson");
        let path = self.dir.join(&rel);
        if let Ok(mut f) = OpenOptions::new().create(true).write(true).open(&path) {
            let mut min_ts = i64::MAX;
            let mut max_ts = i64::MIN;
            for r in rows {
                min_ts = min_ts.min(r.timestamp_ms);
                max_ts = max_ts.max(r.timestamp_ms);
                let _ = writeln!(f, "{}", serde_json::to_string(r).unwrap_or_default());
            }
            let _ = f.flush();
            self.catalog.write().push(FragmentMeta {
                id,
                kind: "events".into(),
                path: rel,
                count: rows.len(),
                min_ts,
                max_ts,
                created_at_ms: Utc::now().timestamp_millis(),
            });
            self.fragments_written.fetch_add(1, Ordering::Relaxed);
            self.persist_catalog();
        }
    }

    pub fn flush(&self) {
        let logs = std::mem::take(&mut *self.pending_logs.write());
        if !logs.is_empty() {
            self.flush_fragment("logs", &logs);
        }
        let events = std::mem::take(&mut *self.pending_events.write());
        if !events.is_empty() {
            self.flush_events_fragment(&events);
        }
    }

    pub fn search_logs(&self, service: Option<&str>, level: Option<&str>, limit: usize) -> Vec<HuskyLog> {
        self.hot_logs
            .read()
            .iter()
            .rev()
            .filter(|l| service.map(|s| l.service == s).unwrap_or(true))
            .filter(|l| level.map(|lv| l.level == lv).unwrap_or(true))
            .take(limit)
            .cloned()
            .collect()
    }

    pub fn list_events(&self, limit: usize) -> Vec<HuskyEvent> {
        self.hot_events.read().iter().rev().take(limit).cloned().collect()
    }

    pub fn stats(&self) -> serde_json::Value {
        serde_json::json!({
            "engine": "husky",
            "fragments": self.catalog.read().len(),
            "fragments_written": self.fragments_written.load(Ordering::Relaxed),
            "hot_logs": self.hot_logs.read().len(),
            "hot_events": self.hot_events.read().len(),
            "dir": self.dir.display().to_string(),
        })
    }
}
