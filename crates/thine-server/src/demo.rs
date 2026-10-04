use chrono::Utc;
use rand::Rng;
use std::collections::BTreeMap;
use std::sync::Arc;
use std::time::Duration;
use thine_common::{
    Aggregation, BoardAnnotation, BoardLayoutKind, CreateBoardRequest, DashboardWidget,
    EventOverlay, MetricPoint, MetricType, Sample, SavedView, ShareConfig, Tags, TemplateVariable,
    WidgetLayout, WidgetType,
};
use thine_storage::MetricStore;
use uuid::Uuid;

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

    // Late spike so Graph Insights / Watchdog Explains have a clear signal.
    for service in services {
        let tags = tags(service, "prod");
        let _ = store.ingest_point(MetricPoint {
            name: "http.server.duration".into(),
            metric_type: MetricType::Histogram,
            tags,
            sample: Sample {
                timestamp_ms: now,
                value: if service == "api" { 220.0 } else { 95.0 },
            },
            unit: Some("ms".into()),
            description: Some("HTTP request duration".into()),
        });
    }

    let service_board = seed_service_overview(store);
    let gpu_board = seed_gpu_overview(store);
    let timeboard = seed_latency_timeboard(store);
    let unified = seed_full_stack_overview(store);
    seed_recently_deleted(store);

    let _ = store.create_board_list(
        "Ops".into(),
        vec![service_board, gpu_board, timeboard, unified],
    );
    let _ = store.create_board_list("GPU Fleet".into(), vec![gpu_board]);
    let _ = store.create_board_list("Unified".into(), vec![unified, service_board]);

    let _ = store.create_alert(thine_common::CreateAlertRequest {
        name: "High latency".into(),
        metric: "http.server.duration".into(),
        tags: Tags::from([("env".into(), "prod".into())]),
        threshold: 90.0,
        comparator: thine_common::Comparator::Gt,
        window_ms: 60_000,
        enabled: true,
        options: thine_common::AlertOptions {
            detection_method: "threshold".into(),
            recipients: "@slack-ops".into(),
            severity: "critical".into(),
            ..Default::default()
        },
    });
}

fn seed_service_overview(store: &MetricStore) -> Uuid {
    let board = store.create_board(CreateBoardRequest {
        name: "Service Overview".into(),
        description: Some(
            "Executive service health — query values, timeseries, and toplists (Datadog-style)"
                .into(),
        ),
        layout_type: BoardLayoutKind::Dashboard,
        template_variables: vec![
            TemplateVariable {
                name: "env".into(),
                tag: "env".into(),
                default: "prod".into(),
                available_values: vec!["prod".into(), "staging".into(), "*".into()],
                prefix: "filter".into(),
            },
            TemplateVariable {
                name: "service".into(),
                tag: "service".into(),
                default: "*".into(),
                available_values: vec!["api".into(), "worker".into(), "ingest".into(), "*".into()],
                prefix: "filter".into(),
            },
        ],
        tags: vec!["team:platform".into(), "surface:dashboards".into()],
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
                "Latency by service ($env)",
                "http.server.duration",
                Aggregation::Avg,
                Some("service"),
                Some("line"),
                Some("ms"),
                0,
                4,
                8,
                4,
                vec!["exclude_null".into()],
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
                vec![],
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
                vec![],
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
                vec!["rate".into()],
            ),
            w_note(
                "note-tpl",
                "Template variables",
                "Filter this board with $env and $service — selections apply to all widgets.",
                0,
                17,
                12,
                1,
            ),
        ],
    });

    let _ = store.update_board(
        board.id,
        thine_common::UpdateBoardRequest {
            event_overlay: Some(EventOverlay {
                query: "tags:deploy env:$env".into(),
                enabled: true,
            }),
            annotations: Some(vec![BoardAnnotation {
                id: "anno-deploy".into(),
                timestamp_ms: Utc::now().timestamp_millis() - 15 * 60_000,
                label: "deploy".into(),
                color: Some("#F4A261".into()),
            }]),
            saved_views: Some(vec![
                SavedView {
                    id: "view-prod".into(),
                    name: "Production".into(),
                    description: Some("env:prod".into()),
                    selections: BTreeMap::from([("env".into(), "prod".into())]),
                },
                SavedView {
                    id: "view-staging".into(),
                    name: "Staging".into(),
                    description: Some("env:staging".into()),
                    selections: BTreeMap::from([("env".into(), "staging".into())]),
                },
            ]),
            share: Some(ShareConfig {
                public: false,
                token: None,
                refresh_secs: 30,
            }),
            ..Default::default()
        },
    );

    board.id
}

fn seed_gpu_overview(store: &MetricStore) -> Uuid {
    let board = store.create_board(CreateBoardRequest {
        name: "GPU Fleet".into(),
        description: Some(
            "DCGM-style GPU utilization, memory, power, and errors with template variables".into(),
        ),
        layout_type: BoardLayoutKind::Screenboard,
        template_variables: vec![
            TemplateVariable {
                name: "gpu_id".into(),
                tag: "gpu_id".into(),
                default: "*".into(),
                available_values: vec![
                    "*".into(),
                    "gpu-0".into(),
                    "gpu-1".into(),
                    "gpu-2".into(),
                    "gpu-3".into(),
                ],
                prefix: "filter".into(),
            },
            TemplateVariable {
                name: "node".into(),
                tag: "host".into(),
                default: "*".into(),
                available_values: vec!["*".into()],
                prefix: "filter".into(),
            },
        ],
        tags: vec!["team:ml".into(), "surface:gpu".into()],
        widgets: vec![
            w_group("gpu-overview", "GPU Overview", 0, 0, 12, 1),
            w_qv(
                "qv-gpu-util",
                "GPU Utilization",
                "gpu.utilization",
                Aggregation::Avg,
                Some("%"),
                0,
                1,
                3,
                2,
            ),
            w_qv(
                "qv-gpu-mem",
                "Memory Used %",
                "gpu.memory.used_percent",
                Aggregation::Avg,
                Some("%"),
                3,
                1,
                3,
                2,
            ),
            w_qv(
                "qv-gpu-power",
                "Power (W)",
                "gpu.power.usage",
                Aggregation::Avg,
                Some("W"),
                6,
                1,
                3,
                2,
            ),
            w_qv(
                "qv-gpu-temp",
                "Temperature",
                "gpu.temperature",
                Aggregation::Max,
                Some("°C"),
                9,
                1,
                3,
                2,
            ),
            w_group("gpu-mem", "Memory", 0, 3, 12, 1),
            w_ts(
                "ts-gpu-mem-used",
                "Memory used by GPU",
                "gpu.memory.used",
                Aggregation::Avg,
                Some("gpu_id"),
                Some("area"),
                Some("MiB"),
                0,
                4,
                6,
                4,
                vec![],
            ),
            w_ts(
                "ts-gpu-mem-free",
                "Memory free by GPU",
                "gpu.memory.free",
                Aggregation::Avg,
                Some("gpu_id"),
                Some("line"),
                Some("MiB"),
                6,
                4,
                6,
                4,
                vec![],
            ),
            w_group("gpu-util", "Utilization & Clocks", 0, 8, 12, 1),
            w_ts(
                "ts-gpu-util",
                "Utilization by GPU",
                "gpu.utilization",
                Aggregation::Avg,
                Some("gpu_id"),
                Some("line"),
                Some("%"),
                0,
                9,
                8,
                4,
                vec!["top(4)".into()],
            ),
            w_pie(
                "pie-gpu-util",
                "Util share",
                "gpu.utilization",
                Aggregation::Avg,
                Some("gpu_id"),
                Some("%"),
                8,
                9,
                4,
                4,
            ),
            w_ts(
                "ts-gpu-sm",
                "SM active",
                "gpu.sm_active",
                Aggregation::Avg,
                Some("gpu_id"),
                Some("area"),
                Some("%"),
                0,
                13,
                6,
                3,
                vec![],
            ),
            w_ts(
                "ts-gpu-clock",
                "SM clock",
                "gpu.clock.sm",
                Aggregation::Avg,
                Some("gpu_id"),
                Some("line"),
                Some("MHz"),
                6,
                13,
                6,
                3,
                vec![],
            ),
            w_group("gpu-health", "Errors & Health", 0, 16, 12, 1),
            w_top(
                "top-xid",
                "XID errors by GPU",
                "gpu.errors.xid.total",
                Aggregation::Max,
                Some("gpu_id"),
                Some("1"),
                0,
                17,
                6,
                3,
            ),
            w_top(
                "top-ecc",
                "ECC SBE by GPU",
                "gpu.ecc.sbe",
                Aggregation::Max,
                Some("gpu_id"),
                Some("1"),
                6,
                17,
                6,
                3,
            ),
        ],
    });
    board.id
}

fn seed_recently_deleted(store: &MetricStore) {
    let board = store.create_board(CreateBoardRequest {
        name: "James's Dashboard".into(),
        description: Some("Soft-deleted demo board — restore from Recently Deleted".into()),
        layout_type: BoardLayoutKind::Dashboard,
        template_variables: vec![],
        tags: vec!["demo:deleted".into()],
        widgets: vec![w_note(
            "gone",
            "Deleted",
            "This board is in Recently Deleted for 30 days.",
            0,
            0,
            12,
            2,
        )],
    });
    let _ = store.delete_board(board.id);
}

/// Datadog “How to Build A Unified Dashboard” — Full Stack Overview screenboard.
fn seed_full_stack_overview(store: &MetricStore) -> Uuid {
    let board = store.create_board(CreateBoardRequest {
        name: "[Thine] Full Stack Overview".into(),
        description: Some(
            "Unified screenboard — Frontend + Application + Availability/SLAs (Datadog Tips & Tricks parity)"
                .into(),
        ),
        layout_type: BoardLayoutKind::Screenboard,
        template_variables: vec![
            TemplateVariable {
                name: "customer-id".into(),
                tag: "customer".into(),
                default: "*".into(),
                available_values: vec!["*".into(), "acme".into(), "globex".into()],
                prefix: "filter".into(),
            },
            TemplateVariable {
                name: "env".into(),
                tag: "env".into(),
                default: "prod".into(),
                available_values: vec!["prod".into(), "staging".into(), "*".into()],
                prefix: "filter".into(),
            },
        ],
        tags: vec!["team:sre".into(), "surface:unified".into(), "layout:screenboard".into()],
        widgets: vec![
            w_group("fs-frontend", "Frontend", 0, 0, 12, 1),
            w_qv_accent(
                "fs-web-uptime",
                "Web App Uptime",
                "http.server.request.count",
                Aggregation::Avg,
                Some("%"),
                0,
                1,
                2,
                2,
                "green",
            ),
            w_qv_accent(
                "fs-api-uptime",
                "API Uptime",
                "http.server.request.count",
                Aggregation::Avg,
                Some("%"),
                2,
                1,
                2,
                2,
                "green",
            ),
            w_qv_accent(
                "fs-ww-rev",
                "Worldwide Revenue",
                "http.server.request.count",
                Aggregation::Sum,
                Some("1"),
                4,
                1,
                2,
                2,
                "green",
            ),
            w_qv_accent(
                "fs-uk-rev",
                "UK Revenue",
                "http.server.request.count",
                Aggregation::Sum,
                Some("1"),
                6,
                1,
                2,
                2,
                "green",
            ),
            w_ts(
                "fs-dns",
                "DNS, Connect, and SSL Time",
                "http.server.duration",
                Aggregation::Avg,
                Some("service"),
                Some("area"),
                Some("ms"),
                8,
                1,
                4,
                2,
                vec![],
            ),
            w_ts(
                "fs-rtt",
                "Response Time (by location)",
                "http.server.duration",
                Aggregation::Avg,
                Some("service"),
                Some("area"),
                Some("ms"),
                0,
                3,
                4,
                3,
                vec![],
            ),
            w_top(
                "fs-slow-api",
                "My Slowest API Endpoints",
                "http.server.duration",
                Aggregation::Avg,
                Some("service"),
                Some("ms"),
                4,
                3,
                4,
                3,
            ),
            w_note(
                "fs-err-logs",
                "App Error Logs",
                "12:28 · api · PaymentServiceUnavailableError\n12:27 · worker · upstream timeout\n12:26 · ingest · connection reset",
                8,
                3,
                4,
                3,
            ),
            w_group("fs-app", "Application", 0, 6, 12, 1),
            w_qv_accent(
                "fs-rps",
                "Avg Request / sec",
                "http.server.request.count",
                Aggregation::Avg,
                Some("hits"),
                0,
                7,
                2,
                2,
                "red",
            ),
            w_qv_accent(
                "fs-lat",
                "Avg Latency",
                "http.server.duration",
                Aggregation::Avg,
                Some("ms"),
                2,
                7,
                2,
                2,
                "orange",
            ),
            w_ts(
                "fs-time-svc",
                "% of Time Spent by service",
                "process.runtime.cpu.utilization",
                Aggregation::Avg,
                Some("service"),
                Some("area"),
                Some("%"),
                4,
                7,
                4,
                3,
                vec![],
            ),
            w_top(
                "fs-dbq",
                "DB Query duration",
                "http.server.duration",
                Aggregation::Avg,
                Some("service"),
                Some("ms"),
                8,
                7,
                4,
                3,
            ),
            w_top(
                "fs-err-ep",
                "Errors / endpoint",
                "http.server.request.count",
                Aggregation::Sum,
                Some("service"),
                Some("1"),
                0,
                9,
                4,
                3,
            ),
            w_ts(
                "fs-hits",
                "Hit/error count on WebStore",
                "http.server.request.count",
                Aggregation::Sum,
                Some("service"),
                Some("area"),
                Some("1"),
                4,
                10,
                4,
                2,
                vec![],
            ),
            w_group("fs-sla", "App Availability & SLAs", 0, 12, 12, 1),
            w_qv_accent(
                "fs-ssl",
                "Login - SSL certificate",
                "http.server.request.count",
                Aggregation::Avg,
                Some("days"),
                0,
                13,
                2,
                2,
                "green",
            ),
            w_qv_accent(
                "fs-apache",
                "Apache",
                "http.server.request.count",
                Aggregation::Max,
                Some("1"),
                2,
                13,
                2,
                2,
                "red",
            ),
            w_qv_accent(
                "fs-iis",
                "IIS Sites up",
                "http.server.request.count",
                Aggregation::Max,
                Some("1"),
                4,
                13,
                2,
                2,
                "green",
            ),
            w_qv(
                "fs-sla-7d",
                "Audit — Past 7 Days",
                "http.server.request.count",
                Aggregation::Avg,
                Some("%"),
                6,
                13,
                3,
                2,
            ),
            w_qv(
                "fs-sla-30d",
                "Audit — Past 30 Days",
                "http.server.request.count",
                Aggregation::Avg,
                Some("%"),
                9,
                13,
                3,
                2,
            ),
            w_group("fs-os", "OS", 0, 15, 6, 1),
            w_group("fs-net", "Network", 6, 15, 6, 1),
            w_qv(
                "fs-mem",
                "Avg Free Mem",
                "system.memory.usage",
                Aggregation::Avg,
                Some("MiB"),
                0,
                16,
                3,
                2,
            ),
            w_qv(
                "fs-cpu",
                "Avg CPU per User",
                "process.runtime.cpu.utilization",
                Aggregation::Avg,
                Some("%"),
                3,
                16,
                3,
                2,
            ),
            w_ts(
                "fs-vol",
                "Volume Sent by service",
                "http.server.request.count",
                Aggregation::Sum,
                Some("service"),
                Some("bars"),
                Some("1"),
                6,
                16,
                3,
                2,
                vec![],
            ),
            w_ts(
                "fs-tcp",
                "TCP Segments",
                "http.server.request.count",
                Aggregation::Sum,
                Some("service"),
                Some("line"),
                Some("1"),
                9,
                16,
                3,
                2,
                vec![],
            ),
        ],
    });
    board.id
}

fn seed_latency_timeboard(store: &MetricStore) -> Uuid {
    let board = store.create_board(CreateBoardRequest {
        name: "Latency Timeboard".into(),
        description: Some(
            "Shared-time troubleshooting board — correlate latency, volume, and CPU".into(),
        ),
        layout_type: BoardLayoutKind::Timeboard,
        template_variables: vec![TemplateVariable {
            name: "env".into(),
            tag: "env".into(),
            default: "prod".into(),
            available_values: vec!["prod".into(), "staging".into()],
            prefix: "filter".into(),
        }],
        tags: vec!["team:sre".into()],
        widgets: vec![
            w_ts(
                "tb-lat",
                "Latency",
                "http.server.duration",
                Aggregation::Avg,
                Some("service"),
                Some("line"),
                Some("ms"),
                0,
                0,
                12,
                4,
                vec![],
            ),
            w_ts(
                "tb-req",
                "Requests",
                "http.server.request.count",
                Aggregation::Avg,
                Some("service"),
                Some("bars"),
                Some("1"),
                0,
                4,
                12,
                3,
                vec!["rate".into()],
            ),
            w_ts(
                "tb-cpu",
                "CPU",
                "process.runtime.cpu.utilization",
                Aggregation::Avg,
                Some("service"),
                Some("area"),
                Some("%"),
                0,
                7,
                12,
                3,
                vec![],
            ),
        ],
    });
    board.id
}

fn empty_widget(id: &str, widget_type: WidgetType, title: &str) -> DashboardWidget {
    DashboardWidget {
        id: id.into(),
        widget_type,
        title: title.into(),
        metric: String::new(),
        tags: Tags::new(),
        aggregation: Aggregation::Avg,
        group_by: None,
        layout: WidgetLayout::default(),
        unit: None,
        display: None,
        text: None,
        functions: Vec::new(),
        template_vars: Vec::new(),
        tab: None,
        events_query: None,
        hide_anomaly_detection: false,
        accent: None,
    }
}

fn w_qv_accent(
    id: &str,
    title: &str,
    metric: &str,
    aggregation: Aggregation,
    unit: Option<&str>,
    x: u32,
    y: u32,
    w: u32,
    h: u32,
    accent: &str,
) -> DashboardWidget {
    let mut widget = w_qv(id, title, metric, aggregation, unit, x, y, w, h);
    widget.accent = Some(accent.into());
    widget
}

fn w_group(id: &str, title: &str, x: u32, y: u32, w: u32, h: u32) -> DashboardWidget {
    let mut widget = empty_widget(id, WidgetType::Group, title);
    widget.layout = WidgetLayout { x, y, w, h };
    widget.text = Some(title.into());
    widget
}

fn w_note(id: &str, title: &str, text: &str, x: u32, y: u32, w: u32, h: u32) -> DashboardWidget {
    let mut widget = empty_widget(id, WidgetType::Note, title);
    widget.layout = WidgetLayout { x, y, w, h };
    widget.text = Some(text.into());
    widget
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
    let mut widget = empty_widget(id, WidgetType::QueryValue, title);
    widget.metric = metric.into();
    widget.tags = Tags::new(); // template variables supply env/service filters
    widget.aggregation = aggregation;
    widget.layout = WidgetLayout { x, y, w, h };
    widget.unit = unit.map(|s| s.into());
    widget
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
    functions: Vec<String>,
) -> DashboardWidget {
    let mut widget = empty_widget(id, WidgetType::Timeseries, title);
    widget.metric = metric.into();
    widget.tags = Tags::new();
    widget.aggregation = aggregation;
    widget.group_by = group_by.map(|s| s.into());
    widget.layout = WidgetLayout { x, y, w, h };
    widget.unit = unit.map(|s| s.into());
    widget.display = display.map(|s| s.into());
    widget.functions = functions;
    widget
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
    let mut widget = empty_widget(id, WidgetType::Toplist, title);
    widget.metric = metric.into();
    widget.tags = Tags::new();
    widget.aggregation = aggregation;
    widget.group_by = group_by.map(|s| s.into());
    widget.layout = WidgetLayout { x, y, w, h };
    widget.unit = unit.map(|s| s.into());
    widget
}

fn w_pie(
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
    let mut widget = empty_widget(id, WidgetType::PieChart, title);
    widget.metric = metric.into();
    widget.tags = Tags::new();
    widget.aggregation = aggregation;
    widget.group_by = group_by.map(|s| s.into());
    widget.layout = WidgetLayout { x, y, w, h };
    widget.unit = unit.map(|s| s.into());
    widget
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
