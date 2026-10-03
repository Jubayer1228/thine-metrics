use chrono::Utc;
use rand::Rng;
use std::collections::BTreeMap;
use std::sync::Arc;
use std::time::Duration;
use thine_common::{
    Aggregation, CreateBoardRequest, DashboardWidget, MetricPoint, MetricType, Sample, Tags,
};
use thine_storage::MetricStore;

pub fn seed(store: &MetricStore) {
    let now = Utc::now().timestamp_millis();
    let services = ["api", "worker", "ingest"];
    let envs = ["prod", "staging"];

    for service in services {
        for env in envs {
            for i in 0..60 {
                let ts = now - (60 - i) * 15_000;
                let tags = tags(service, env);
                let _ = store.ingest_point(MetricPoint {
                    name: "http.server.duration".into(),
                    metric_type: MetricType::Histogram,
                    tags: tags.clone(),
                    sample: Sample {
                        timestamp_ms: ts,
                        value: 40.0 + (i as f64 % 17.0) * 3.5,
                    },
                    unit: Some("ms".into()),
                    description: Some("HTTP request duration".into()),
                });
                let _ = store.ingest_point(MetricPoint {
                    name: "http.server.request.count".into(),
                    metric_type: MetricType::Counter,
                    tags: tags.clone(),
                    sample: Sample {
                        timestamp_ms: ts,
                        value: 100.0 + i as f64 * 4.0,
                    },
                    unit: Some("1".into()),
                    description: Some("HTTP requests".into()),
                });
                let _ = store.ingest_point(MetricPoint {
                    name: "process.runtime.cpu.utilization".into(),
                    metric_type: MetricType::Gauge,
                    tags: tags.clone(),
                    sample: Sample {
                        timestamp_ms: ts,
                        value: 0.15 + ((i as f64).sin().abs() * 0.5),
                    },
                    unit: Some("1".into()),
                    description: Some("CPU utilization".into()),
                });
                let _ = store.ingest_point(MetricPoint {
                    name: "system.memory.usage".into(),
                    metric_type: MetricType::Gauge,
                    tags,
                    sample: Sample {
                        timestamp_ms: ts,
                        value: 512.0 + i as f64 * 2.5,
                    },
                    unit: Some("MiB".into()),
                    description: Some("Resident memory".into()),
                });
            }
        }
    }

    let _ = store.create_board(CreateBoardRequest {
        name: "Service health".into(),
        widgets: vec![
            DashboardWidget {
                title: "Latency".into(),
                metric: "http.server.duration".into(),
                tags: tags("api", "prod"),
                aggregation: Aggregation::Avg,
            },
            DashboardWidget {
                title: "CPU".into(),
                metric: "process.runtime.cpu.utilization".into(),
                tags: tags("api", "prod"),
                aggregation: Aggregation::Avg,
            },
            DashboardWidget {
                title: "Memory".into(),
                metric: "system.memory.usage".into(),
                tags: tags("api", "prod"),
                aggregation: Aggregation::Last,
            },
        ],
    });
}

pub async fn run_live_generator(store: Arc<MetricStore>) {
    let mut tick = 0u64;
    loop {
        tokio::time::sleep(Duration::from_secs(2)).await;
        tick += 1;
        let mut rng = rand::rng();
        let now = Utc::now().timestamp_millis();
        for service in ["api", "worker", "ingest"] {
            let tags = tags(service, "prod");
            let _ = store.ingest_point(MetricPoint {
                name: "http.server.duration".into(),
                metric_type: MetricType::Histogram,
                tags: tags.clone(),
                sample: Sample {
                    timestamp_ms: now,
                    value: 35.0 + rng.random_range(0.0..80.0),
                },
                unit: Some("ms".into()),
                description: None,
            });
            let _ = store.ingest_point(MetricPoint {
                name: "http.server.request.count".into(),
                metric_type: MetricType::Counter,
                tags: tags.clone(),
                sample: Sample {
                    timestamp_ms: now,
                    value: 120.0 + tick as f64 + rng.random_range(0.0..20.0),
                },
                unit: Some("1".into()),
                description: None,
            });
            let _ = store.ingest_point(MetricPoint {
                name: "process.runtime.cpu.utilization".into(),
                metric_type: MetricType::Gauge,
                tags: tags.clone(),
                sample: Sample {
                    timestamp_ms: now,
                    value: (0.2 + (tick as f64 / 15.0).sin().abs() * 0.55)
                        .clamp(0.0, 1.0),
                },
                unit: Some("1".into()),
                description: None,
            });
            let _ = store.ingest_point(MetricPoint {
                name: "system.memory.usage".into(),
                metric_type: MetricType::Gauge,
                tags,
                sample: Sample {
                    timestamp_ms: now,
                    value: 500.0 + (tick % 40) as f64 * 3.0,
                },
                unit: Some("MiB".into()),
                description: None,
            });
        }
    }
}

fn tags(service: &str, env: &str) -> Tags {
    BTreeMap::from([
        ("service".into(), service.into()),
        ("env".into(), env.into()),
        ("telemetry.sdk.language".into(), "rust".into()),
    ])
}
