//! Bits AI — agentic assistant that can *run* the Datadog-style tutorial
//! (install agent → dashboard + golden signals → template vars → monitors).

use chrono::Utc;
use serde_json::{json, Value};
use thine_common::{
    Aggregation, AlertOptions, BoardLayoutKind, Comparator, CreateAlertRequest, CreateBoardRequest,
    DashboardWidget, MetricPoint, MetricType, Sample, Tags, TemplateVariable, WidgetLayout,
    WidgetType,
};

use crate::state::PlatformState;

impl PlatformState {
    pub fn bits_chat_deep(&self, message: &str) -> Value {
        let lower = message.to_ascii_lowercase();
        let mut actions: Vec<Value> = Vec::new();
        let mut citations: Vec<&str> = Vec::new();

        // —— Agentic tutorial / mutation intents (order matters: specific first) ——
        if is_tutorial(&lower) {
            let result = self.bits_run_tutorial();
            citations.extend([
                "/api/v1/fleet/bootstrap",
                "/api/v1/boards",
                "/api/v1/alerts",
            ]);
            return bits_reply(
                "tutorial",
                result["reply"].as_str().unwrap_or("Tutorial complete."),
                &citations,
                result["actions"].as_array().cloned().unwrap_or_default(),
                result.get("artifacts").cloned(),
            );
        }

        if looks_like(&lower, &["install agent", "bootstrap agent", "fleet install", "add agent"]) {
            let r = self.bits_install_agent(None);
            let reply = r["reply"].as_str().unwrap_or("Agent installed.").to_string();
            citations.push("/api/v1/fleet/bootstrap");
            actions.push(json!({"type":"bootstrap_agent","platform":"linux","id": r["agent_id"]}));
            actions.push(json!({"type":"navigate","page":"fleet"}));
            return bits_reply("install_agent", &reply, &citations, actions, Some(r));
        }

        if looks_like(
            &lower,
            &[
                "create dashboard",
                "new dashboard",
                "golden signals",
                "essential widgets",
                "timeboard",
            ],
        ) {
            let r = self.bits_create_tutorial_dashboard();
            let reply = r["reply"].as_str().unwrap_or("Dashboard created.").to_string();
            citations.push("/api/v1/boards");
            actions.push(json!({"type":"navigate","page":"dashboards","board_id": r["board_id"]}));
            return bits_reply("create_dashboard", &reply, &citations, actions, Some(r));
        }

        if looks_like(
            &lower,
            &[
                "create monitor",
                "new monitor",
                "high cpu",
                "latency alert",
                "alert when",
            ],
        ) {
            let r = self.bits_create_tutorial_monitors(&lower);
            let reply = r["reply"].as_str().unwrap_or("Monitor created.").to_string();
            citations.push("/api/v1/alerts");
            actions.push(json!({"type":"navigate","page":"monitors"}));
            return bits_reply("create_monitor", &reply, &citations, actions, Some(r));
        }

        if looks_like(
            &lower,
            &[
                "template variable",
                "add variable",
                "filter by env",
            ],
        ) {
            let r = self.bits_ensure_template_vars(None);
            let reply = r["reply"].as_str().unwrap_or("Variables added.").to_string();
            citations.push("/api/v1/boards");
            actions.push(json!({"type":"navigate","page":"dashboards","board_id": r["board_id"]}));
            return bits_reply("template_variables", &reply, &citations, actions, Some(r));
        }

        if looks_like(
            &lower,
            &[
                "alert fatigue",
                "recovery threshold",
                "monitor tip",
                "multi-alert",
            ],
        ) {
            citations.push("/api/v1/alerts");
            return bits_reply(
                "alert_best_practices",
                "Avoid alert fatigue (Datadog tutorial):\n\
                 • Multi-alert: one monitor `avg:cpu{*} by {host} > 80` instead of 50 host monitors\n\
                 • Recovery hysteresis: alert >80, recover <70 (not the same threshold)\n\
                 • Alert on symptoms (latency / errors), not only causes (CPU)\n\
                 • Windows: 5m for prod, 1h for capacity — avoid 1m noise\n\
                 • Schedule downtime for deploys; attach a runbook link in the message\n\
                 Ask me to “create a CPU monitor with recovery” and I’ll publish one.",
                &citations,
                vec![json!({"type":"navigate","page":"monitors"})],
                None,
            );
        }

        // —— Read / investigate intents ——
        if lower.contains("fleet") || lower.contains("agent status") {
            let s = self.fleet_summary();
            citations.push("/api/v1/fleet/agents");
            actions.push(json!({"type":"navigate","page":"fleet"}));
            return bits_reply(
                "fleet_status",
                &format!(
                    "Fleet: {} agents ({} healthy, {} configured). Platforms: {}",
                    s["total"], s["healthy"], s["configured"], s["platforms"]
                ),
                &citations,
                actions,
                Some(s),
            );
        }

        if lower.contains("slo") {
            let slos = self.list_slos();
            let breaching: Vec<_> = slos
                .iter()
                .filter(|s| s.status == "breaching")
                .map(|s| format!("{} ({:.1}%)", s.slo.name, s.current_pct))
                .collect();
            citations.push("/api/v1/slos");
            let body = if breaching.is_empty() {
                format!("All {} SLOs are within target.", slos.len())
            } else {
                format!("Breaching SLOs: {}", breaching.join(", "))
            };
            return bits_reply("slo_status", &body, &citations, vec![], None);
        }

        if lower.contains("incident") {
            let open: Vec<_> = self
                .list_incidents()
                .into_iter()
                .filter(|i| i.status == "open")
                .collect();
            citations.push("/api/v1/incidents");
            return bits_reply(
                "incidents",
                &format!(
                    "{} open incident(s). Latest: {}",
                    open.len(),
                    open.first().map(|i| i.title.as_str()).unwrap_or("none")
                ),
                &citations,
                vec![],
                None,
            );
        }

        if lower.contains("log") || lower.contains("error") {
            let errs = self.list_errors();
            citations.extend(["/api/v1/errors", "/api/v1/logs/search"]);
            return bits_reply(
                "errors",
                &format!(
                    "{} error group(s). Top: {}",
                    errs.len(),
                    errs.first()
                        .map(|e| format!("{}× {}", e.count, e.message))
                        .unwrap_or_else(|| "none".into())
                ),
                &citations,
                vec![],
                None,
            );
        }

        if lower.contains("cost") {
            let c = self.cost_detail();
            citations.push("/api/v1/cost/summary");
            return bits_reply(
                "cost",
                &format!(
                    "Month-to-date ${:.2}. Top service: {}",
                    c.iter().map(|l| l.total).sum::<f64>(),
                    c.first()
                        .map(|l| format!("{} (${:.2})", l.service, l.total))
                        .unwrap_or_else(|| "n/a".into())
                ),
                &citations,
                vec![],
                None,
            );
        }

        if lower.contains("deploy") || lower.contains("dora") {
            let d = self.dora();
            citations.push("/api/v1/dora");
            return bits_reply(
                "dora",
                &format!(
                    "Deploy freq {:.2}/day · CFR {:.0}% · MTTR {:.1}h · AI impact {:.0}%",
                    d.deploy_frequency_per_day,
                    d.change_failure_rate * 100.0,
                    d.mttr_hours,
                    d.ai_impact_score * 100.0
                ),
                &citations,
                vec![],
                None,
            );
        }

        if lower.contains("latency")
            || lower.contains("why is")
            || lower.contains("investigate")
            || lower.contains("rca")
            || lower.contains("notebook")
        {
            let dash = self.metrics.dashboard();
            let alerts = self.metrics.list_alerts();
            let firing = self.metrics.list_alert_events(20);
            let firing_n = firing
                .iter()
                .filter(|e| matches!(e.status, thine_common::AlertStatus::Firing))
                .count();
            let worst = self
                .apm_service_stats()
                .into_iter()
                .max_by(|a, b| a.p95_ms.partial_cmp(&b.p95_ms).unwrap_or(std::cmp::Ordering::Equal));
            let svc = worst
                .as_ref()
                .map(|s| s.service.clone())
                .unwrap_or_else(|| "api".into());
            let p95 = worst.as_ref().map(|s| s.p95_ms).unwrap_or(0.0);
            let nb = self.create_notebook(crate::CreateNotebook {
                title: format!("Bits RCA — {svc} latency"),
                cells: vec![
                    crate::NotebookCell {
                        kind: "markdown".into(),
                        content: format!(
                            "## Bits RCA\nService **{svc}** p95 ≈ {:.1}ms · {:.1}/s ingest · {} firing events.\nOpen APM Service Page and Live Tail for `{svc}`.",
                            p95, dash.ingest_rate_per_sec, firing_n
                        ),
                    },
                    crate::NotebookCell {
                        kind: "metric".into(),
                        content: format!("avg:http.server.duration{{service:{svc}}} by {{resource}}"),
                    },
                    crate::NotebookCell {
                        kind: "metric".into(),
                        content: format!("avg:process.runtime.cpu.utilization{{service:{svc}}}"),
                    },
                    crate::NotebookCell {
                        kind: "markdown".into(),
                        content: "### Next\n1. Error Tracking lifecycle\n2. Deploy / DORA correlation\n3. Container Map for saturation".into(),
                    },
                ],
            });
            citations.extend([
                "/api/v1/notebooks",
                "/api/v1/dashboard",
                "/api/v1/query",
                "/api/v1/alerts",
                "/api/v1/apm/stats",
            ]);
            return bits_reply(
                "rca",
                &format!(
                    "Opened RCA notebook **{}** for `{svc}` (p95 {:.1}ms). \
                     {} monitors · {} recent fires · {:.1}/s ingest across {} series. \
                     Notebook cells query live metrics — edit, run, or deep-link from Notebooks.",
                    nb.title,
                    p95,
                    alerts.len(),
                    firing_n,
                    dash.ingest_rate_per_sec,
                    dash.series_count
                ),
                &citations,
                vec![
                    json!({"type":"navigate","page":"notebooks","notebook_id": nb.id}),
                    json!({"type":"navigate","page":"observability"}),
                ],
                Some(json!({"notebook_id": nb.id, "service": svc, "p95_ms": p95})),
            );
        }

        if lower.contains("metric") || lower.contains("dashboard") {
            let boards = self.metrics.list_boards();
            let dash = self.metrics.dashboard();
            citations.extend(["/api/v1/dashboard", "/api/v1/boards"]);
            return bits_reply(
                "metrics",
                &format!(
                    "{} series · {:.1}/s ingest · {} boards · {} active alerts",
                    dash.series_count,
                    dash.ingest_rate_per_sec,
                    boards.len(),
                    dash.active_alerts
                ),
                &citations,
                vec![json!({"type":"navigate","page":"dashboards"})],
                None,
            );
        }

        let stats = crate::feature_stats();
        bits_reply(
            "general",
            &format!(
                "I’m Thine Bits — I can *do* the Datadog tutorial, not just talk about it.\n\
                 Try:\n\
                 • “Run the monitoring tutorial”\n\
                 • “Install an agent”\n\
                 • “Create a dashboard with golden signals”\n\
                 • “Create a high CPU monitor with recovery”\n\
                 • “How do I avoid alert fatigue?”\n\
                 Coverage {}/{} features done+partial.",
                stats.done + stats.partial,
                stats.total
            ),
            &["/api/v1/features", "/api/v1/bits/chat"],
            vec![],
            None,
        )
    }

    /// Full tutorial path: agent → dashboard + vars + golden signals → monitors.
    pub fn bits_run_tutorial(&self) -> Value {
        let agent = self.bits_install_agent(Some("linux"));
        let board = self.bits_create_tutorial_dashboard();
        let monitors = self.bits_create_tutorial_monitors("cpu latency recovery");
        let actions = vec![
            json!({"type":"bootstrap_agent","platform":"linux","id": agent["agent_id"]}),
            json!({"type":"navigate","page":"fleet"}),
            json!({"type":"navigate","page":"dashboards","board_id": board["board_id"]}),
            json!({"type":"navigate","page":"monitors"}),
        ];
        json!({
            "reply": format!(
                "Tutorial complete.\n\
                 1. Agent `{}` registered + host metrics seeded\n\
                 2. Dashboard `{}` with Golden Signals + $env/$service/$region\n\
                 3. Monitors created: {}\n\
                 Open Fleet → Dashboards → Monitors to review. Continuous DogStatsD starts when the UI runs bootstrap.",
                agent["agent_id"].as_str().unwrap_or("?"),
                board["name"].as_str().unwrap_or("?"),
                monitors["names"].as_array().map(|a| a.iter().filter_map(|v| v.as_str()).collect::<Vec<_>>().join(", ")).unwrap_or_default()
            ),
            "actions": actions,
            "artifacts": {
                "agent_id": agent["agent_id"],
                "board_id": board["board_id"],
                "alert_ids": monitors["alert_ids"],
            }
        })
    }

    pub fn bits_install_agent(&self, platform: Option<&str>) -> Value {
        let plat = platform.unwrap_or("linux");
        let id = format!("bits-agent-{}", &uuid::Uuid::new_v4().to_string()[..8]);
        let host = format!("{}-bits", plat);
        self.fleet_heartbeat(&id, "0.2.0", &host, Some(plat));
        self.fleet_configure("standard", Some(&[id.clone()]));
        // Seed host metrics so dashboards light up immediately
        let now = Utc::now().timestamp_millis();
        let mut tags = thine_common::Tags::new();
        tags.insert("host".into(), host.clone());
        tags.insert("platform".into(), plat.into());
        tags.insert("agent".into(), id.clone());
        tags.insert("env".into(), "prod".into());
        for i in 0..12 {
            let t = now - (11 - i) * 10_000;
            for (name, base, unit) in [
                ("system.cpu.user", 18.0 + i as f64, Some("%")),
                ("system.mem.pct", 45.0 + (i % 5) as f64, Some("%")),
                ("system.load.1", 0.6 + i as f64 * 0.05, None),
                ("system.disk.used", 55.0 + (i % 4) as f64, Some("%")),
            ] {
                let _ = self.metrics.ingest_point(MetricPoint {
                    name: name.into(),
                    metric_type: MetricType::Gauge,
                    tags: tags.clone(),
                    sample: Sample {
                        timestamp_ms: t,
                        value: base,
                    },
                    unit: unit.map(|u| u.into()),
                    description: None,
                });
            }
        }
        json!({
            "agent_id": id,
            "host": host,
            "platform": plat,
            "reply": format!("Installed agent `{id}` on host `{host}` ({plat}) with standard checks. Host metrics seeded — UI bootstrap keeps DogStatsD flowing."),
        })
    }

    pub fn bits_create_tutorial_dashboard(&self) -> Value {
        let name = "Platform - API Gateway - Performance".to_string();
        let widgets = vec![
            qv("Traffic", "http.server.request.count", Aggregation::Sum, "1", "purple", 0, 0),
            qv("Errors", "http.server.duration", Aggregation::Avg, "%", "green", 3, 0),
            qv("Latency", "http.server.duration", Aggregation::Avg, "ms", "orange", 6, 0),
            qv("Saturation", "system.cpu.user", Aggregation::Avg, "%", "yellow", 9, 0),
            ts("CPU usage (%)", "system.cpu.user", Some("host"), 0, 2),
            ts("Latency", "http.server.duration", Some("service"), 0, 5),
            DashboardWidget {
                id: "toplist-cpu".into(),
                widget_type: WidgetType::Toplist,
                title: "Top hosts by CPU".into(),
                metric: "system.cpu.user".into(),
                tags: Default::default(),
                aggregation: Aggregation::Avg,
                group_by: Some("host".into()),
                layout: WidgetLayout { x: 0, y: 8, w: 6, h: 3 },
                unit: Some("%".into()),
                display: None,
                text: None,
                functions: vec![],
                template_vars: vec!["env".into(), "service".into()],
                tab: None,
                events_query: None,
                hide_anomaly_detection: false,
                accent: None,
            },
            DashboardWidget {
                id: "note-gs".into(),
                widget_type: WidgetType::Note,
                title: "Golden Signals".into(),
                metric: String::new(),
                tags: Default::default(),
                aggregation: Aggregation::Avg,
                group_by: None,
                layout: WidgetLayout { x: 6, y: 8, w: 6, h: 3 },
                unit: None,
                display: None,
                text: Some("Traffic · Errors · Latency · Saturation — Bits scaffolded this from the Datadog tutorial.".into()),
                functions: vec![],
                template_vars: vec![],
                tab: None,
                events_query: None,
                hide_anomaly_detection: true,
                accent: None,
            },
        ];
        let board = self.metrics.create_board(CreateBoardRequest {
            name: name.clone(),
            description: Some(
                "Bits tutorial timeboard — Golden Signals + host CPU (Datadog Dashboards & Alerts path)"
                    .into(),
            ),
            layout_type: BoardLayoutKind::Timeboard,
            widgets,
            template_variables: tutorial_vars(),
            tags: vec!["bits".into(), "tutorial".into(), "golden-signals".into()],
        });
        json!({
            "board_id": board.id.to_string(),
            "name": board.name,
            "widgets": board.widgets.len(),
            "template_variables": board.template_variables.iter().map(|t| format!("${}", t.name)).collect::<Vec<_>>(),
            "reply": format!(
                "Created timeboard `{}` with {} widgets and template vars {}.",
                board.name,
                board.widgets.len(),
                board.template_variables.iter().map(|t| format!("${}", t.name)).collect::<Vec<_>>().join(", ")
            ),
        })
    }

    pub fn bits_ensure_template_vars(&self, board_id: Option<&str>) -> Value {
        let boards = self.metrics.list_boards();
        let target = if let Some(id) = board_id {
            boards.into_iter().find(|b| b.id.to_string() == id)
        } else {
            boards
                .into_iter()
                .find(|b| b.tags.iter().any(|t| t == "bits" || t == "tutorial"))
                .or_else(|| self.metrics.list_boards().into_iter().next())
        };
        let Some(board) = target else {
            let created = self.bits_create_tutorial_dashboard();
            return created;
        };
        let mut tvs = board.template_variables.clone();
        for v in tutorial_vars() {
            if !tvs.iter().any(|t| t.name == v.name) {
                tvs.push(v);
            }
        }
        let updated = self
            .metrics
            .update_board(
                board.id,
                thine_common::UpdateBoardRequest {
                    template_variables: Some(tvs.clone()),
                    ..Default::default()
                },
            )
            .unwrap_or(board);
        json!({
            "board_id": updated.id.to_string(),
            "template_variables": tvs.iter().map(|t| format!("${}", t.name)).collect::<Vec<_>>(),
            "reply": format!(
                "Template variables on `{}`: {}. Use $env / $service in widget queries instead of cloning boards.",
                updated.name,
                tvs.iter().map(|t| format!("${}", t.name)).collect::<Vec<_>>().join(", ")
            ),
        })
    }

    pub fn bits_create_tutorial_monitors(&self, lower: &str) -> Value {
        let mut names = Vec::new();
        let mut ids = Vec::new();

        let want_cpu = lower.contains("cpu") || lower.contains("recovery") || !lower.contains("latency");
        let want_lat = lower.contains("latency") || lower.contains("p95") || lower.contains("tutorial");

        if want_cpu {
            let rule = self.metrics.create_alert(CreateAlertRequest {
                name: "CPU usage is high for host {{host.name}}".into(),
                metric: "system.cpu.user".into(),
                tags: Tags::new(),
                threshold: 80.0,
                comparator: Comparator::Gt,
                window_ms: 5 * 60_000,
                enabled: true,
                    options: AlertOptions {
                    detection_method: "threshold".into(),
                    warning_threshold: Some(70.0),
                    recovery_threshold: Some(60.0),
                    recipients: "@slack-ops".into(),
                    evaluate: "avg".into(),
                    group_by: Some("host".into()),
                    severity: "critical".into(),
                    message: "High CPU on {{host.name}}".into(),
                    ..Default::default()
                },
            });
            names.push(rule.name.clone());
            ids.push(rule.id.to_string());
        }

        if want_lat {
            let rule = self.metrics.create_alert(CreateAlertRequest {
                name: "P95 Latency > 500ms".into(),
                metric: "http.server.duration".into(),
                tags: tags_map(&[("env", "prod"), ("service", "api")]),
                threshold: 500.0,
                comparator: Comparator::Gt,
                window_ms: 10 * 60_000,
                enabled: true,
                options: AlertOptions {
                    detection_method: "threshold".into(),
                    warning_threshold: Some(300.0),
                    recovery_threshold: Some(250.0),
                    recipients: "@slack-ops @pagerduty-primary".into(),
                    evaluate: "avg".into(),
                    severity: "warning".into(),
                    message: "P95 latency elevated".into(),
                    ..Default::default()
                },
            });
            names.push(rule.name.clone());
            ids.push(rule.id.to_string());
        }

        json!({
            "names": names,
            "alert_ids": ids,
            "reply": format!(
                "Published {} monitor(s): {}. Recovery thresholds + severity tags stored on each rule (alert fatigue pattern from the tutorial).",
                names.len(),
                names.join("; ")
            ),
        })
    }
}

fn tutorial_vars() -> Vec<TemplateVariable> {
    vec![
        TemplateVariable {
            name: "env".into(),
            tag: "env".into(),
            default: "prod".into(),
            available_values: vec!["prod".into(), "staging".into(), "dev".into(), "*".into()],
            prefix: "filter".into(),
        },
        TemplateVariable {
            name: "service".into(),
            tag: "service".into(),
            default: "*".into(),
            available_values: vec!["*".into(), "api".into(), "web".into(), "worker".into()],
            prefix: "filter".into(),
        },
        TemplateVariable {
            name: "region".into(),
            tag: "region".into(),
            default: "*".into(),
            available_values: vec!["*".into(), "us-east-1".into(), "eu-west-1".into()],
            prefix: "filter".into(),
        },
    ]
}

fn qv(title: &str, metric: &str, agg: Aggregation, unit: &str, accent: &str, x: u32, y: u32) -> DashboardWidget {
    DashboardWidget {
        id: format!("qv-{}", title.to_ascii_lowercase()),
        widget_type: WidgetType::QueryValue,
        title: title.into(),
        metric: metric.into(),
        tags: Default::default(),
        aggregation: agg,
        group_by: None,
        layout: WidgetLayout { x, y, w: 3, h: 2 },
        unit: Some(unit.into()),
        display: None,
        text: None,
        functions: vec![],
        template_vars: vec!["env".into(), "service".into()],
        tab: None,
        events_query: None,
        hide_anomaly_detection: false,
        accent: Some(accent.into()),
    }
}

fn ts(title: &str, metric: &str, group_by: Option<&str>, x: u32, y: u32) -> DashboardWidget {
    DashboardWidget {
        id: format!("ts-{}", title.to_ascii_lowercase().replace(' ', "-")),
        widget_type: WidgetType::Timeseries,
        title: title.into(),
        metric: metric.into(),
        tags: Default::default(),
        aggregation: Aggregation::Avg,
        group_by: group_by.map(|s| s.into()),
        layout: WidgetLayout { x, y, w: 12, h: 3 },
        unit: None,
        display: Some("line".into()),
        text: None,
        functions: vec![],
        template_vars: vec!["env".into(), "service".into()],
        tab: None,
        events_query: None,
        hide_anomaly_detection: false,
        accent: None,
    }
}

fn tags_map(pairs: &[(&str, &str)]) -> thine_common::Tags {
    let mut t = thine_common::Tags::new();
    for (k, v) in pairs {
        t.insert((*k).into(), (*v).into());
    }
    t
}

fn is_tutorial(lower: &str) -> bool {
    looks_like(
        lower,
        &[
            "run the monitoring tutorial",
            "run the tutorial",
            "monitoring tutorial",
            "datadog tutorial",
            "get started monitoring",
            "setup monitoring",
            "set up monitoring",
            "scaffold",
        ],
    )
}

fn looks_like(hay: &str, needles: &[&str]) -> bool {
    needles.iter().any(|n| hay.contains(n))
}

fn bits_reply(
    intent: &str,
    reply: &str,
    citations: &[&str],
    actions: Vec<Value>,
    artifacts: Option<Value>,
) -> Value {
    json!({
        "role": "assistant",
        "intent": intent,
        "reply": reply,
        "citations": citations,
        "actions": actions,
        "artifacts": artifacts,
        "model": "thine-bits-agent-v2",
    })
}
