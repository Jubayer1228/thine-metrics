use chrono::Utc;
use rand::Rng;
use std::collections::BTreeMap;
use std::sync::Arc;
use std::time::Duration;
use thine_common::{
    Aggregation, CreateAlertRequest, CreateBoardRequest, DashboardWidget, MetricPoint, MetricType,
    Sample, Tags, WidgetLayout, WidgetType,
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

    seed_service_overview(store);

    let _ = store.create_alert(CreateAlertRequest {
        name: "High latency".into(),
        metric: "http.server.duration".into(),
        tags: Tags::from([("env".into(), "prod".into())]),
        threshold: 90.0,
        comparator: thine_common::Comparator::Gt,
        window_ms: 60_000,
        enabled: true,
    });
}

fn seed_service_overview(store: &MetricStore) {
    // Datadog-style screenboard: groups + query values + timeseries + toplist
    let _ = store.create_board(CreateBoardRequest {
        name: "Service Overview".into(),
        description: Some(
            "Executive service health — query values, timeseries, and toplists (Datadog-style)"
                .into(),
        ),
        widgets: vec![
            w_group("overview", "Overview", 0, 0, 12, 1),
            w_qv(
                "qv-latency",
                "Avg Latency",
                "http.server.duration",
                Aggregation::Avg,
                Some("ms"),
                0,
                1,
                3,
                2,
            ),
            w_qv(
                "qv-req",
                "Request Volume",
                "http.server.request.count",
                Aggregation::Avg,
                Some("req"),
                3,
                1,
                3,
                2,
            ),
            w_qv(
                "qv-cpu",
                "CPU Utilization",
                "process.runtime.cpu.utilization",
                Aggregation::Avg,
                Some("%"),
                6,
                1,
                3,
                2,
            ),
            w_qv(
                "qv-mem",
                "Memory Usage",
                "system.memory.usage",
                Aggregation::Last,
                Some("MiB"),
                9,
                1,
                3,
                2,
            ),
            w_group("traffic", "Traffic", 0, 3, 12, 1),
            w_ts(
                "ts-latency",
                "Latency by service",
                "http.server.duration",
                Aggregation::Avg,
                Some("service"),
                Some("line"),
                Some("ms"),
                0,
                4,
                8,
                4,
            ),
            w_top(
                "top-latency",
                "Top services by latency",
                "http.server.duration",
                Aggregation::Avg,
                Some("service"),
                Some("ms"),
                8,
                4,
                4,
                4,
            ),
            w_group("resources", "Resources", 0, 8, 12, 1),
            w_ts(
                "ts-cpu",
                "CPU by service",
                "process.runtime.cpu.utilization",
                Aggregation::Avg,
                Some("service"),
                Some("area"),
                Some("1"),
                0,
                9,
                6,
                4,
            ),
            w_ts(
                "ts-mem",
                "Memory by service",
                "system.memory.usage",
                Aggregation::Avg,
                Some("service"),
                Some("area"),
                Some("MiB"),
                6,
                9,
                6,
                4,
            ),
            w_ts(
                "ts-req",
                "Request count by service",
                "http.server.request.count",
                Aggregation::Avg,
                Some("service"),
                Some("bars"),
                Some("1"),
                0,
                13,
                12,
                4,
            ),
        ],
    });
}

fn w_group(id: &str, title: &str, x: u32, y: u32, w: u32, h: u32) -> DashboardWidget {
    DashboardWidget {
        id: id.into(),
        widget_type: WidgetType::Group,
        title: title.into(),
        metric: String::new(),
        tags: Tags::new(),
        aggregation: Aggregation::Avg,
        group_by: None,
        layout: WidgetLayout { x, y, w, h },
        unit: None,
        display: None,
        text: Some(title.into()),
    }
}

fn w_qv(
    id: &str,
    title: &str,
    metric: &str,
    aggregation: Aggregation,
    unit: Option<&str>,
    x: u32,
    y: u32,
    w: u32,
    h: u32,
) -> DashboardWidget {
    DashboardWidget {
        id: id.into(),
        widget_type: WidgetType::QueryValue,
        title: title.into(),
        metric: metric.into(),
        tags: Tags::from([("env".into(), "prod".into())]),
        aggregation,
        group_by: None,
        layout: WidgetLayout { x, y, w, h },
        unit: unit.map(|s| s.into()),
        display: None,
        text: None,
    }
}

fn w_ts(
    id: &str,
    title: &str,
    metric: &str,
    aggregation: Aggregation,
    group_by: Option<&str>,
    display: Option<&str>,
    unit: Option<&str>,
    x: u32,
    y: u32,
    w: u32,
    h: u32,
) -> DashboardWidget {
    DashboardWidget {
        id: id.into(),
        widget_type: WidgetType::Timeseries,
        title: title.into(),
        metric: metric.into(),
        tags: Tags::from([("env".into(), "prod".into())]),
        aggregation,
        group_by: group_by.map(|s| s.into()),
        layout: WidgetLayout { x, y, w, h },
        unit: unit.map(|s| s.into()),
        display: display.map(|s| s.into()),
        text: None,
    }
}

fn w_top(
    id: &str,
    title: &str,
    metric: &str,
    aggregation: Aggregation,
    group_by: Option<&str>,
    unit: Option<&str>,
    x: u32,
    y: u32,
    w: u32,
    h: u32,
) -> DashboardWidget {
    DashboardWidget {
        id: id.into(),
        widget_type: WidgetType::Toplist,
        title: title.into(),
        metric: metric.into(),
        tags: Tags::from([("env".into(), "prod".into())]),
        aggregation,
        group_by: group_by.map(|s| s.into()),
        layout: WidgetLayout { x, y, w, h },
        unit: unit.map(|s| s.into()),
        display: None,
        text: None,
    }
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
                    value: (0.2 + (tick as f64 / 15.0).sin().abs() * 0.55).clamp(0.0, 1.0),
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
