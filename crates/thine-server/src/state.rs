use std::sync::Arc;
use thine_ingest::IngestService;
use thine_storage::{MetricStore, StorageConfig};

#[derive(Clone)]
pub struct AppState {
    pub store: Arc<MetricStore>,
    pub ingest: Arc<IngestService>,
}

impl AppState {
    pub fn new() -> Self {
        let retention_ms = std::env::var("THINE_RETENTION_MS")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(24 * 60 * 60 * 1000);
        let store = MetricStore::new(StorageConfig {
            retention_ms,
            ..StorageConfig::default()
        });
        let ingest = Arc::new(IngestService::new(store.clone()));
        Self { store, ingest }
    }
}
