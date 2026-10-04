//! APM trace store: sampled payloads indexed by trace_id + always-on stats hooks.
//! Full traces are expensive — keep an indexed hot map and spill to disk.

use chrono::Utc;
use dashmap::DashMap;
use parking_lot::RwLock;
use serde::{Deserialize, Serialize};
use std::collections::VecDeque;
use std::fs::{self, File};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use thine_common::IntakeSpan;
use tracing::info;

const MAX_TRACES: usize = 10_000;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StoredSpan {
    pub trace_id: String,
    pub span_id: String,
    pub parent_span_id: Option<String>,
    pub service: String,
    pub name: String,
    pub duration_ms: f64,
    pub timestamp_ms: i64,
    pub status: String,
    pub resource: Option<String>,
}

pub struct TraceStore {
    dir: PathBuf,
    by_trace: DashMap<String, Vec<StoredSpan>>,
    order: RwLock<VecDeque<String>>,
    ingested: AtomicU64,
    sampled_kept: AtomicU64,
    sampled_dropped: AtomicU64,
    /// Keep probability when over soft capacity (1.0 = keep all until hard max).
    sample_keep: f64,
}

impl TraceStore {
    pub fn open(data_dir: impl AsRef<Path>) -> std::io::Result<Self> {
        let dir = data_dir.as_ref().join("traces");
        fs::create_dir_all(&dir)?;
        let store = Self {
            dir,
            by_trace: DashMap::new(),
            order: RwLock::new(VecDeque::new()),
            ingested: AtomicU64::new(0),
            sampled_kept: AtomicU64::new(0),
            sampled_dropped: AtomicU64::new(0),
            sample_keep: std::env::var("THINE_TRACE_SAMPLE_KEEP")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(1.0),
        };
        store.load_recent()?;
        Ok(store)
    }

    fn load_recent(&self) -> std::io::Result<()> {
        let mut files: Vec<_> = fs::read_dir(&self.dir)?
            .flatten()
            .filter(|e| e.path().extension().and_then(|s| s.to_str()) == Some("json"))
            .collect();
        files.sort_by_key(|e| std::cmp::Reverse(e.metadata().map(|m| m.modified().ok()).ok()));
        for entry in files.into_iter().take(200) {
            if let Ok(raw) = fs::read_to_string(entry.path()) {
                if let Ok(spans) = serde_json::from_str::<Vec<StoredSpan>>(&raw) {
                    if let Some(tid) = spans.first().map(|s| s.trace_id.clone()) {
                        self.by_trace.insert(tid.clone(), spans);
                        self.order.write().push_back(tid);
                    }
                }
            }
        }
        info!(traces = self.by_trace.len(), "trace store warmed");
        Ok(())
    }

    fn should_keep(&self, spans: &[IntakeSpan]) -> bool {
        // Always keep error traces; otherwise apply sample_keep when large.
        if spans.iter().any(|s| s.status == "error" || s.status == "erroring") {
            return true;
        }
        if self.by_trace.len() < MAX_TRACES / 2 {
            return true;
        }
        use std::collections::hash_map::DefaultHasher;
        use std::hash::{Hash, Hasher};
        let mut h = DefaultHasher::new();
        spans.first().map(|s| s.trace_id.hash(&mut h));
        Utc::now().timestamp_millis().hash(&mut h);
        let r = (h.finish() % 10_000) as f64 / 10_000.0;
        r < self.sample_keep
    }

    pub fn ingest(&self, spans: Vec<IntakeSpan>) -> usize {
        if spans.is_empty() {
            return 0;
        }
        self.ingested.fetch_add(spans.len() as u64, Ordering::Relaxed);
        // Group by trace_id
        let mut groups: std::collections::BTreeMap<String, Vec<IntakeSpan>> =
            std::collections::BTreeMap::new();
        for s in spans {
            groups.entry(s.trace_id.clone()).or_default().push(s);
        }
        let mut accepted = 0usize;
        for (tid, group) in groups {
            if !self.should_keep(&group) {
                self.sampled_dropped.fetch_add(1, Ordering::Relaxed);
                continue;
            }
            self.sampled_kept.fetch_add(1, Ordering::Relaxed);
            let stored: Vec<StoredSpan> = group
                .into_iter()
                .map(|s| StoredSpan {
                    trace_id: s.trace_id,
                    span_id: s.span_id,
                    parent_span_id: s.parent_span_id,
                    service: s.service,
                    name: s.name,
                    duration_ms: s.duration_ms,
                    timestamp_ms: s.timestamp_ms,
                    status: s.status,
                    resource: s.resource,
                })
                .collect();
            accepted += stored.len();
            self.spill(&tid, &stored);
            let is_new = !self.by_trace.contains_key(&tid);
            self.by_trace.insert(tid.clone(), stored);
            if is_new {
                self.order.write().push_back(tid);
            }
            self.evict();
        }
        accepted
    }

    fn spill(&self, tid: &str, spans: &[StoredSpan]) {
        let path = self.dir.join(format!("{tid}.json"));
        if let Ok(mut f) = File::create(path) {
            let _ = f.write_all(serde_json::to_string(spans).unwrap_or_default().as_bytes());
        }
    }

    fn evict(&self) {
        let mut order = self.order.write();
        while order.len() > MAX_TRACES {
            if let Some(tid) = order.pop_front() {
                self.by_trace.remove(&tid);
                let _ = fs::remove_file(self.dir.join(format!("{tid}.json")));
            }
        }
    }

    pub fn list_recent(&self, limit: usize) -> Vec<StoredSpan> {
        let mut out = Vec::new();
        for tid in self.order.read().iter().rev() {
            if let Some(spans) = self.by_trace.get(tid) {
                if let Some(root) = spans.first() {
                    out.push(root.clone());
                }
            }
            if out.len() >= limit {
                break;
            }
        }
        out
    }

    pub fn get_trace(&self, trace_id: &str) -> Option<Vec<StoredSpan>> {
        self.by_trace.get(trace_id).map(|v| v.clone())
    }

    pub fn stats(&self) -> serde_json::Value {
        serde_json::json!({
            "engine": "trace_store",
            "traces": self.by_trace.len(),
            "spans_ingested": self.ingested.load(Ordering::Relaxed),
            "sampled_kept": self.sampled_kept.load(Ordering::Relaxed),
            "sampled_dropped": self.sampled_dropped.load(Ordering::Relaxed),
            "sample_keep": self.sample_keep,
            "dir": self.dir.display().to_string(),
        })
    }
}
