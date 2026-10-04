use std::path::PathBuf;
use std::sync::Arc;
use thine_ingest::{spawn_intake_bus, DurableBundle, IngestService};
use thine_platform::PlatformState;
use thine_storage::{HuskyStore, MetricStore, RtdbEngine, StorageConfig, TraceStore};
use tracing::info;

#[derive(Clone)]
pub struct AppState {
    pub store: Arc<MetricStore>,
    pub ingest: Arc<IngestService>,
    pub platform: Arc<PlatformState>,
    pub rtdb: Arc<RtdbEngine>,
    pub husky: Arc<HuskyStore>,
    pub traces: Arc<TraceStore>,
    pub data_dir: PathBuf,
}

impl AppState {
    pub fn new() -> Self {
        let data_dir = PathBuf::from(
            std::env::var("THINE_DATA_DIR").unwrap_or_else(|_| "./data".into()),
        );
        let _ = std::fs::create_dir_all(&data_dir);

        let retention_ms = std::env::var("THINE_RETENTION_MS")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(24 * 60 * 60 * 1000);
        let store = MetricStore::new(StorageConfig {
            retention_ms,
            ..StorageConfig::default()
        });

        let rtdb = Arc::new(
            RtdbEngine::open(&data_dir).expect("open RTDB engine"),
        );
        let husky = Arc::new(HuskyStore::open(&data_dir).expect("open Husky store"));
        let traces = Arc::new(TraceStore::open(&data_dir).expect("open TraceStore"));

        // Replay durable metrics WAL into hot MetricStore (Datadog RTDB recovery).
        let replayed = rtdb.replay_wal();
        if !replayed.is_empty() {
            let stats = store.ingest_batch(replayed);
            info!(
                accepted = stats.accepted,
                series = stats.series_count,
                "replayed RTDB WAL into hot MetricStore"
            );
        }

        let ingest = Arc::new(IngestService::new(store.clone()));
        let platform = PlatformState::new(store.clone());

        let p_spans = platform.clone();
        let p_logs = platform.clone();
        let p_events = platform.clone();
        ingest.set_sinks(
            Arc::new(move |spans| {
                let _ = p_spans.ingest_intake_spans(spans);
            }),
            Arc::new(move |logs| {
                let _ = p_logs.ingest_intake_logs(logs);
            }),
            Arc::new(move |events| {
                let _ = p_events.ingest_events(events);
            }),
        );

        let bus = spawn_intake_bus(DurableBundle {
            rtdb: rtdb.clone(),
            husky: husky.clone(),
            traces: traces.clone(),
        });
        ingest.set_bus(bus);
        ingest.set_durable_stats(serde_json::json!({
            "rtdb": rtdb.stats(),
            "husky": husky.stats(),
            "traces": traces.stats(),
            "data_dir": data_dir.display().to_string(),
        }));

        info!(dir = %data_dir.display(), "durable signal stores ready");

        Self {
            store,
            ingest,
            platform,
            rtdb,
            husky,
            traces,
            data_dir,
        }
    }

    pub fn refresh_durable_stats(&self) {
        self.ingest.set_durable_stats(serde_json::json!({
            "rtdb": self.rtdb.stats(),
            "husky": self.husky.stats(),
            "traces": self.traces.stats(),
            "data_dir": self.data_dir.display().to_string(),
        }));
    }
}

