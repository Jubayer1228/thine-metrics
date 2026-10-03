//! In-memory platform stores — lock-scoped, ring-buffered, performance-conscious.

use chrono::Utc;
use dashmap::DashMap;
use parking_lot::RwLock;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, VecDeque};
use std::sync::Arc;
use thine_common::{Aggregation, QueryRequest, Tags};
use thine_storage::MetricStore;
use uuid::Uuid;

const MAX_LOGS: usize = 10_000;
const MAX_SPANS: usize = 5_000;
const MAX_AUDIT: usize = 2_000;

#[derive(Debug)]
pub struct PlatformState {
    metrics: Arc<MetricStore>,
    notebooks: DashMap<Uuid, Notebook>,
    teams: DashMap<Uuid, Team>,
    users: DashMap<Uuid, RbacUser>,
    roles: DashMap<Uuid, RbacRole>,
    incidents: DashMap<Uuid, Incident>,
    workflows: DashMap<Uuid, Workflow>,
    workflow_runs: RwLock<VecDeque<WorkflowRun>>,
    slos: DashMap<Uuid, Slo>,
    catalog: DashMap<String, CatalogService>,
    integrations: DashMap<String, Integration>,
    hosts: DashMap<String, HostInfo>,
    spans: RwLock<VecDeque<SpanRecord>>,
    logs: RwLock<VecDeque<LogEvent>>,
    audit: RwLock<VecDeque<AuditEvent>>,
    work: DashMap<Uuid, WorkItem>,
    policies: DashMap<String, serde_json::Value>,
    agents: DashMap<String, AgentInfo>,
    marketplace: DashMap<String, MarketplaceApp>,
    pipelines: DashMap<String, Pipeline>,
    errors: DashMap<String, ErrorGroup>,
    profiles: DashMap<String, ProfileMeta>,
    containers: DashMap<String, ContainerInfo>,
    functions: DashMap<String, serde_json::Value>,
    volumes: DashMap<String, VolumeInfo>,
    gpus: DashMap<String, GpuDevice>,
    streams: DashMap<String, StreamInfo>,
    db_instances: DashMap<String, DbInstance>,
    data_assets: DashMap<String, serde_json::Value>,
    sds_rules: DashMap<String, SdsRule>,
    fleet: DashMap<String, FleetAgent>,
    network_flows: RwLock<Vec<serde_json::Value>>,
    deploys: RwLock<VecDeque<(i64, String)>>,
}

impl PlatformState {
    pub fn new(metrics: Arc<MetricStore>) -> Arc<Self> {
        Arc::new(Self {
            metrics,
            notebooks: DashMap::new(),
            teams: DashMap::new(),
            users: DashMap::new(),
            roles: DashMap::new(),
            incidents: DashMap::new(),
            workflows: DashMap::new(),
            workflow_runs: RwLock::new(VecDeque::new()),
            slos: DashMap::new(),
            catalog: DashMap::new(),
            integrations: DashMap::new(),
            hosts: DashMap::new(),
            spans: RwLock::new(VecDeque::with_capacity(1024)),
            logs: RwLock::new(VecDeque::with_capacity(1024)),
            audit: RwLock::new(VecDeque::with_capacity(256)),
            work: DashMap::new(),
            policies: DashMap::new(),
            agents: DashMap::new(),
            marketplace: DashMap::new(),
            pipelines: DashMap::new(),
            errors: DashMap::new(),
            profiles: DashMap::new(),
            containers: DashMap::new(),
            functions: DashMap::new(),
            volumes: DashMap::new(),
            gpus: DashMap::new(),
            streams: DashMap::new(),
            db_instances: DashMap::new(),
            data_assets: DashMap::new(),
            sds_rules: DashMap::new(),
            fleet: DashMap::new(),
            network_flows: RwLock::new(Vec::new()),
            deploys: RwLock::new(VecDeque::new()),
        })
    }

    pub fn audit(&self, action: &str, resource: &str, actor: &str) {
        let mut q = self.audit.write();
        q.push_back(AuditEvent {
            id: Uuid::new_v4(),
            timestamp_ms: Utc::now().timestamp_millis(),
            action: action.into(),
            resource: resource.into(),
            actor: actor.into(),
        });
        while q.len() > MAX_AUDIT {
            q.pop_front();
        }
    }

    pub fn list_audit(&self, limit: usize) -> Vec<AuditEvent> {
        self.audit.read().iter().rev().take(limit).cloned().collect()
    }

    // —— Notebooks ——
    pub fn create_notebook(&self, req: CreateNotebook) -> Notebook {
        let nb = Notebook {
            id: Uuid::new_v4(),
            title: req.title,
            cells: req.cells,
            updated_at_ms: Utc::now().timestamp_millis(),
        };
        self.notebooks.insert(nb.id, nb.clone());
        self.audit("create", &format!("notebook:{}", nb.id), "system");
        nb
    }
    pub fn list_notebooks(&self) -> Vec<Notebook> {
        let mut v: Vec<_> = self.notebooks.iter().map(|e| e.value().clone()).collect();
        v.sort_by(|a, b| b.updated_at_ms.cmp(&a.updated_at_ms));
        v
    }

    // —— Teams / RBAC ——
    pub fn create_team(&self, req: CreateTeam) -> Team {
        let t = Team {
            id: Uuid::new_v4(),
            name: req.name,
            members: req.members,
        };
        self.teams.insert(t.id, t.clone());
        t
    }
    pub fn list_teams(&self) -> Vec<Team> {
        self.teams.iter().map(|e| e.value().clone()).collect()
    }
    pub fn list_users(&self) -> Vec<RbacUser> {
        self.users.iter().map(|e| e.value().clone()).collect()
    }
    pub fn list_roles(&self) -> Vec<RbacRole> {
        self.roles.iter().map(|e| e.value().clone()).collect()
    }

    // —— Incidents / work / workflows ——
    pub fn create_incident(&self, req: CreateIncident) -> Incident {
        let i = Incident {
            id: Uuid::new_v4(),
            title: req.title,
            severity: req.severity,
            status: "open".into(),
            service: req.service,
            created_at_ms: Utc::now().timestamp_millis(),
        };
        self.incidents.insert(i.id, i.clone());
        self.audit("create", &format!("incident:{}", i.id), "system");
        i
    }
    pub fn list_incidents(&self) -> Vec<Incident> {
        let mut v: Vec<_> = self.incidents.iter().map(|e| e.value().clone()).collect();
        v.sort_by(|a, b| b.created_at_ms.cmp(&a.created_at_ms));
        v
    }
    pub fn list_work(&self) -> Vec<WorkItem> {
        self.work.iter().map(|e| e.value().clone()).collect()
    }
    pub fn create_workflow(&self, req: CreateWorkflow) -> Workflow {
        let w = Workflow {
            id: Uuid::new_v4(),
            name: req.name,
            trigger: req.trigger,
            steps: req.steps,
            enabled: true,
        };
        self.workflows.insert(w.id, w.clone());
        w
    }
    pub fn list_workflows(&self) -> Vec<Workflow> {
        self.workflows.iter().map(|e| e.value().clone()).collect()
    }
    pub fn list_workflow_runs(&self, limit: usize) -> Vec<WorkflowRun> {
        self.workflow_runs
            .read()
            .iter()
            .rev()
            .take(limit)
            .cloned()
            .collect()
    }

    // —— SLOs ——
    pub fn create_slo(&self, req: CreateSlo) -> Slo {
        let s = Slo {
            id: Uuid::new_v4(),
            name: req.name,
            metric: req.metric,
            target: req.target,
            window_ms: req.window_ms.unwrap_or(7 * 24 * 3600 * 1000),
            tags: req.tags.unwrap_or_default(),
        };
        self.slos.insert(s.id, s.clone());
        s
    }
    pub fn list_slos(&self) -> Vec<SloStatus> {
        self.slos
            .iter()
            .map(|e| self.eval_slo(e.value()))
            .collect()
    }
    fn eval_slo(&self, slo: &Slo) -> SloStatus {
        let end = Utc::now().timestamp_millis();
        let start = end - slo.window_ms;
        let results = self
            .metrics
            .query(QueryRequest {
                metric: slo.metric.clone(),
                tags: slo.tags.clone(),
                start_ms: Some(start),
                end_ms: Some(end),
                step_ms: (slo.window_ms / 100).max(60_000),
                aggregation: Aggregation::Avg,
            })
            .unwrap_or_default();
        let vals: Vec<f64> = results
            .iter()
            .flat_map(|r| r.points.iter().map(|p| p.value))
            .collect();
        // Treat metric as "badness" (e.g. error ratio or latency); uptime = clamp(1 - normalized)
        let current = if vals.is_empty() {
            slo.target
        } else {
            let avg = vals.iter().sum::<f64>() / vals.len() as f64;
            // heuristic: if values look like ratios (<=1), invert; else assume latency ms vs 200ms budget
            if avg <= 1.0 {
                ((1.0 - avg) * 100.0).clamp(0.0, 100.0)
            } else {
                (100.0 - (avg / 2.0)).clamp(0.0, 100.0)
            }
        };
        let budget_left = current - slo.target;
        SloStatus {
            slo: slo.clone(),
            current_pct: current,
            budget_left,
            status: if current >= slo.target {
                "ok"
            } else {
                "breaching"
            }
            .into(),
        }
    }

    // —— Catalog / integrations ——
    pub fn list_catalog(&self) -> Vec<CatalogService> {
        self.catalog.iter().map(|e| e.value().clone()).collect()
    }
    pub fn list_integrations(&self) -> Vec<Integration> {
        self.integrations.iter().map(|e| e.value().clone()).collect()
    }
    pub fn set_integration(&self, id: &str, enabled: bool) -> Option<Integration> {
        let mut e = self.integrations.get_mut(id)?;
        e.enabled = enabled;
        Some(e.clone())
    }

    // —— Infra ——
    pub fn list_hosts(&self) -> Vec<HostInfo> {
        self.hosts.iter().map(|e| e.value().clone()).collect()
    }
    pub fn list_containers(&self) -> Vec<ContainerInfo> {
        self.containers.iter().map(|e| e.value().clone()).collect()
    }
    pub fn list_gpus(&self) -> Vec<GpuDevice> {
        self.gpus.iter().map(|e| e.value().clone()).collect()
    }
    pub fn list_volumes(&self) -> Vec<VolumeInfo> {
        self.volumes.iter().map(|e| e.value().clone()).collect()
    }
    pub fn list_functions(&self) -> Vec<serde_json::Value> {
        self.functions.iter().map(|e| e.value().clone()).collect()
    }
    pub fn cost_summary(&self) -> CostSummary {
        let mut by_service = BTreeMap::new();
        for svc in self.catalog.iter() {
            by_service.insert(svc.key().clone(), 120.0 + (svc.key().len() as f64) * 17.5);
        }
        let total: f64 = by_service.values().sum();
        CostSummary {
            currency: "USD".into(),
            total_month: total,
            by_service,
        }
    }
    pub fn network_flows(&self) -> Vec<serde_json::Value> {
        self.network_flows.read().clone()
    }
    pub fn list_autoscalers(&self) -> Vec<serde_json::Value> {
        vec![serde_json::json!({
            "name": "api-hpa",
            "namespace": "default",
            "min": 2,
            "max": 20,
            "current": 4,
            "recommended": 5,
            "metric": "cpu"
        })]
    }

    // —— APM ——
    pub fn ingest_spans(&self, spans: Vec<SpanRecord>) -> usize {
        let mut q = self.spans.write();
        let n = spans.len();
        for s in spans {
            q.push_back(s);
        }
        while q.len() > MAX_SPANS {
            q.pop_front();
        }
        n
    }
    pub fn list_traces(&self, limit: usize) -> Vec<SpanRecord> {
        self.spans.read().iter().rev().take(limit).cloned().collect()
    }
    pub fn list_apm_services(&self) -> Vec<String> {
        let mut set = std::collections::BTreeSet::new();
        for s in self.spans.read().iter() {
            set.insert(s.service.clone());
        }
        for c in self.catalog.iter() {
            set.insert(c.key().clone());
        }
        set.into_iter().collect()
    }
    pub fn usm_services(&self) -> Vec<serde_json::Value> {
        self.list_apm_services()
            .into_iter()
            .map(|s| serde_json::json!({"service": s, "protocol": "http", "detected": true}))
            .collect()
    }

    // —— Logs / errors ——
    pub fn ingest_log(&self, event: LogEvent) {
        let mut q = self.logs.write();
        if event.level == "error" || event.level == "fatal" {
            let key = format!("{}:{}", event.service, &event.message[..event.message.len().min(80)]);
            self.errors
                .entry(key.clone())
                .and_modify(|e| {
                    e.count += 1;
                    e.last_seen_ms = event.timestamp_ms;
                })
                .or_insert_with(|| ErrorGroup {
                    id: key,
                    service: event.service.clone(),
                    message: event.message.clone(),
                    count: 1,
                    last_seen_ms: event.timestamp_ms,
                });
        }
        q.push_back(event);
        while q.len() > MAX_LOGS {
            q.pop_front();
        }
    }
    pub fn search_logs(
        &self,
        service: Option<&str>,
        level: Option<&str>,
        limit: usize,
    ) -> Vec<LogEvent> {
        self.logs
            .read()
            .iter()
            .rev()
            .filter(|e| service.map(|s| e.service == s).unwrap_or(true))
            .filter(|e| level.map(|l| e.level == l).unwrap_or(true))
            .take(limit.min(1000))
            .cloned()
            .collect()
    }
    pub fn list_errors(&self) -> Vec<ErrorGroup> {
        let mut v: Vec<_> = self.errors.iter().map(|e| e.value().clone()).collect();
        v.sort_by(|a, b| b.count.cmp(&a.count));
        v
    }

    // —— Misc lists ——
    pub fn list_agents(&self) -> Vec<AgentInfo> {
        self.agents.iter().map(|e| e.value().clone()).collect()
    }
    pub fn list_marketplace(&self) -> Vec<MarketplaceApp> {
        self.marketplace.iter().map(|e| e.value().clone()).collect()
    }
    pub fn list_pipelines(&self) -> Vec<Pipeline> {
        self.pipelines.iter().map(|e| e.value().clone()).collect()
    }
    pub fn list_profiles(&self) -> Vec<ProfileMeta> {
        self.profiles.iter().map(|e| e.value().clone()).collect()
    }
    pub fn list_streams(&self) -> Vec<StreamInfo> {
        self.streams.iter().map(|e| e.value().clone()).collect()
    }
    pub fn list_db_instances(&self) -> Vec<DbInstance> {
        self.db_instances.iter().map(|e| e.value().clone()).collect()
    }
    pub fn list_data_assets(&self) -> Vec<serde_json::Value> {
        self.data_assets.iter().map(|e| e.value().clone()).collect()
    }
    pub fn list_sds_rules(&self) -> Vec<SdsRule> {
        self.sds_rules.iter().map(|e| e.value().clone()).collect()
    }
    pub fn list_fleet(&self) -> Vec<FleetAgent> {
        self.fleet.iter().map(|e| e.value().clone()).collect()
    }
    pub fn list_policies(&self) -> Vec<(String, serde_json::Value)> {
        self.policies
            .iter()
            .map(|e| (e.key().clone(), e.value().clone()))
            .collect()
    }
    pub fn mcp_tools(&self) -> Vec<serde_json::Value> {
        vec![
            serde_json::json!({"name": "query_metrics", "description": "Query Thine metrics"}),
            serde_json::json!({"name": "list_slos", "description": "List SLO statuses"}),
            serde_json::json!({"name": "search_logs", "description": "Search ingested logs"}),
            serde_json::json!({"name": "list_incidents", "description": "List open incidents"}),
        ]
    }
    pub fn cli_info(&self) -> serde_json::Value {
        serde_json::json!({
            "name": "thine",
            "version": env!("CARGO_PKG_VERSION"),
            "commands": ["status", "query", "logs", "slo", "catalog"]
        })
    }
    pub fn bits_chat(&self, message: &str) -> serde_json::Value {
        let features = crate::feature_stats();
        serde_json::json!({
            "role": "assistant",
            "reply": format!(
                "Thine Bits (stub): you said «{}». Platform coverage: {}/{} features done+partial.",
                message,
                features.done + features.partial,
                features.total
            ),
            "citations": ["/api/v1/features", "/api/v1/dashboard"]
        })
    }
    pub fn dora(&self) -> DoraMetrics {
        let deploys = self.deploys.read();
        let incidents = self.list_incidents();
        let deploy_n = deploys.len().max(1) as f64;
        let fail = incidents.iter().filter(|i| i.severity == "critical").count() as f64;
        DoraMetrics {
            deploy_frequency_per_day: deploy_n / 7.0,
            lead_time_hours: 4.2,
            change_failure_rate: (fail / deploy_n).min(1.0),
            mttr_hours: 1.5,
            ai_impact_score: 0.62,
        }
    }

    pub fn seed_demo(&self) {
        let eng = self.create_team(CreateTeam {
            name: "Platform".into(),
            members: vec!["alice@thine.dev".into(), "bob@thine.dev".into()],
        });
        self.users.insert(
            Uuid::new_v4(),
            RbacUser {
                id: Uuid::new_v4(),
                email: "alice@thine.dev".into(),
                roles: vec!["admin".into()],
            },
        );
        self.roles.insert(
            Uuid::new_v4(),
            RbacRole {
                id: Uuid::new_v4(),
                name: "admin".into(),
                permissions: vec!["*".into()],
            },
        );
        self.roles.insert(
            Uuid::new_v4(),
            RbacRole {
                id: Uuid::new_v4(),
                name: "viewer".into(),
                permissions: vec!["metrics:read".into(), "dashboards:read".into()],
            },
        );

        self.create_notebook(CreateNotebook {
            title: "Latency investigation".into(),
            cells: vec![
                NotebookCell {
                    kind: "markdown".into(),
                    content: "## Latency spikes\nCheck `http.server.duration` by service.".into(),
                },
                NotebookCell {
                    kind: "metric".into(),
                    content: "avg:http.server.duration{env:prod} by {service}".into(),
                },
            ],
        });

        for (name, owner) in [("api", &eng.name), ("worker", &eng.name), ("ingest", &eng.name)] {
            self.catalog.insert(
                name.into(),
                CatalogService {
                    name: name.into(),
                    team: owner.clone(),
                    tier: "critical".into(),
                    languages: vec!["rust".into(), "go".into()],
                    links: BTreeMap::from([("repo".into(), format!("https://github.com/thine/{name}"))]),
                },
            );
        }

        for (id, title) in [
            ("aws", "Amazon Web Services"),
            ("postgres", "PostgreSQL"),
            ("kubernetes", "Kubernetes"),
            ("otel", "OpenTelemetry"),
        ] {
            self.integrations.insert(
                id.into(),
                Integration {
                    id: id.into(),
                    title: title.into(),
                    enabled: id == "otel" || id == "kubernetes",
                    category: "cloud".into(),
                },
            );
        }

        for (host, svc) in [("i-api-1", "api"), ("i-worker-1", "worker"), ("i-ingest-1", "ingest")] {
            self.hosts.insert(
                host.into(),
                HostInfo {
                    name: host.into(),
                    service: svc.into(),
                    env: "prod".into(),
                    cpu: 0.42,
                    memory_mib: 2048.0,
                    status: "up".into(),
                },
            );
            self.containers.insert(
                format!("{host}-ctr"),
                ContainerInfo {
                    id: format!("{host}-ctr"),
                    image: format!("thine/{svc}:latest"),
                    host: host.into(),
                    status: "running".into(),
                },
            );
        }

        self.create_slo(CreateSlo {
            name: "API latency SLO".into(),
            metric: "http.server.duration".into(),
            target: 99.0,
            window_ms: Some(3600_000),
            tags: Some(Tags::from([("env".into(), "prod".into())])),
        });
        self.create_slo(CreateSlo {
            name: "CPU budget".into(),
            metric: "process.runtime.cpu.utilization".into(),
            target: 95.0,
            window_ms: Some(3600_000),
            tags: Some(Tags::from([("env".into(), "prod".into())])),
        });

        self.create_incident(CreateIncident {
            title: "Elevated API latency".into(),
            severity: "high".into(),
            service: "api".into(),
        });
        self.work.insert(
            Uuid::new_v4(),
            WorkItem {
                id: Uuid::new_v4(),
                title: "Tune p99 latency".into(),
                status: "in_progress".into(),
                assignee: "alice@thine.dev".into(),
            },
        );
        let wf = self.create_workflow(CreateWorkflow {
            name: "Page on critical incident".into(),
            trigger: "incident.critical".into(),
            steps: vec!["notify_slack".into(), "create_war_room".into()],
        });
        self.workflow_runs.write().push_back(WorkflowRun {
            id: Uuid::new_v4(),
            workflow_id: wf.id,
            status: "succeeded".into(),
            started_at_ms: Utc::now().timestamp_millis(),
        });

        self.policies.insert(
            "metric_retention".into(),
            serde_json::json!({"days": 15, "high_cardinality_tags": ["user_id"]}),
        );

        self.agents.insert(
            "bits-sre".into(),
            AgentInfo {
                id: "bits-sre".into(),
                name: "SRE Copilot".into(),
                capabilities: vec!["metrics".into(), "logs".into(), "incidents".into()],
            },
        );
        self.marketplace.insert(
            "slack-notify".into(),
            MarketplaceApp {
                id: "slack-notify".into(),
                name: "Slack Notify".into(),
                installed: false,
            },
        );
        self.pipelines.insert(
            "default-logs".into(),
            Pipeline {
                id: "default-logs".into(),
                name: "Default logs pipeline".into(),
                processors: vec!["grok".into(), "remap".into()],
            },
        );
        self.profiles.insert(
            "api-cpu".into(),
            ProfileMeta {
                id: "api-cpu".into(),
                service: "api".into(),
                profile_type: "cpu".into(),
                duration_ms: 60_000,
            },
        );
        self.functions.insert(
            "checkout".into(),
            serde_json::json!({"name":"checkout","runtime":"provided.al2023","cold_starts_24h":12}),
        );
        self.volumes.insert(
            "vol-data".into(),
            VolumeInfo {
                id: "vol-data".into(),
                size_gb: 500.0,
                used_gb: 320.0,
            },
        );
        self.gpus.insert(
            "gpu-0".into(),
            GpuDevice {
                id: "gpu-0".into(),
                model: "A10G".into(),
                util: 0.55,
                memory_used_mb: 8192.0,
            },
        );
        self.streams.insert(
            "orders".into(),
            StreamInfo {
                name: "orders".into(),
                lag: 42,
                throughput: 1500.0,
            },
        );
        self.db_instances.insert(
            "pg-primary".into(),
            DbInstance {
                name: "pg-primary".into(),
                engine: "postgres".into(),
                qps: 240.0,
                slow_queries: 3,
            },
        );
        self.data_assets.insert(
            "orders_fact".into(),
            serde_json::json!({"name":"orders_fact","freshness_min":12,"owner":"Platform"}),
        );
        self.sds_rules.insert(
            "email".into(),
            SdsRule {
                id: "email".into(),
                pattern: r"[a-zA-Z0-9._%+-]+@[a-zA-Z0-9.-]+\.[a-zA-Z]{2,}".into(),
                enabled: true,
            },
        );
        self.fleet.insert(
            "agent-1".into(),
            FleetAgent {
                id: "agent-1".into(),
                version: "0.1.0".into(),
                host: "i-api-1".into(),
                status: "healthy".into(),
            },
        );
        *self.network_flows.write() = vec![
            serde_json::json!({"src":"api","dst":"postgres","bytes":1_200_000,"protocol":"tcp"}),
            serde_json::json!({"src":"worker","dst":"redis","bytes":400_000,"protocol":"tcp"}),
        ];

        let now = Utc::now().timestamp_millis();
        {
            let mut d = self.deploys.write();
            for i in 0..14 {
                d.push_back((now - i * 12 * 3600 * 1000, "api".into()));
            }
        }

        self.ingest_spans(vec![
            SpanRecord {
                trace_id: "t1".into(),
                span_id: "s1".into(),
                service: "api".into(),
                name: "http.request".into(),
                duration_ms: 45.0,
                timestamp_ms: now,
                status: "ok".into(),
            },
            SpanRecord {
                trace_id: "t1".into(),
                span_id: "s2".into(),
                service: "worker".into(),
                name: "queue.process".into(),
                duration_ms: 12.0,
                timestamp_ms: now,
                status: "ok".into(),
            },
        ]);
        self.ingest_log(LogEvent {
            timestamp_ms: now,
            level: "info".into(),
            service: "api".into(),
            message: "request completed".into(),
            attrs: BTreeMap::from([("route".into(), "/pay".into())]),
        });
        self.ingest_log(LogEvent {
            timestamp_ms: now,
            level: "error".into(),
            service: "api".into(),
            message: "upstream timeout talking to payments".into(),
            attrs: BTreeMap::new(),
        });
        self.audit("seed", "platform", "system");
    }
}

// —— Types ——

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Notebook {
    pub id: Uuid,
    pub title: String,
    pub cells: Vec<NotebookCell>,
    pub updated_at_ms: i64,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NotebookCell {
    pub kind: String,
    pub content: String,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateNotebook {
    pub title: String,
    #[serde(default)]
    pub cells: Vec<NotebookCell>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Team {
    pub id: Uuid,
    pub name: String,
    pub members: Vec<String>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateTeam {
    pub name: String,
    #[serde(default)]
    pub members: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RbacUser {
    pub id: Uuid,
    pub email: String,
    pub roles: Vec<String>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RbacRole {
    pub id: Uuid,
    pub name: String,
    pub permissions: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Incident {
    pub id: Uuid,
    pub title: String,
    pub severity: String,
    pub status: String,
    pub service: String,
    pub created_at_ms: i64,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateIncident {
    pub title: String,
    pub severity: String,
    pub service: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkItem {
    pub id: Uuid,
    pub title: String,
    pub status: String,
    pub assignee: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Workflow {
    pub id: Uuid,
    pub name: String,
    pub trigger: String,
    pub steps: Vec<String>,
    pub enabled: bool,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateWorkflow {
    pub name: String,
    pub trigger: String,
    #[serde(default)]
    pub steps: Vec<String>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkflowRun {
    pub id: Uuid,
    pub workflow_id: Uuid,
    pub status: String,
    pub started_at_ms: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Slo {
    pub id: Uuid,
    pub name: String,
    pub metric: String,
    pub target: f64,
    pub window_ms: i64,
    pub tags: Tags,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateSlo {
    pub name: String,
    pub metric: String,
    pub target: f64,
    pub window_ms: Option<i64>,
    pub tags: Option<Tags>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SloStatus {
    #[serde(flatten)]
    pub slo: Slo,
    pub current_pct: f64,
    pub budget_left: f64,
    pub status: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CatalogService {
    pub name: String,
    pub team: String,
    pub tier: String,
    pub languages: Vec<String>,
    pub links: BTreeMap<String, String>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Integration {
    pub id: String,
    pub title: String,
    pub enabled: bool,
    pub category: String,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HostInfo {
    pub name: String,
    pub service: String,
    pub env: String,
    pub cpu: f64,
    pub memory_mib: f64,
    pub status: String,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContainerInfo {
    pub id: String,
    pub image: String,
    pub host: String,
    pub status: String,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SpanRecord {
    pub trace_id: String,
    pub span_id: String,
    pub service: String,
    pub name: String,
    pub duration_ms: f64,
    pub timestamp_ms: i64,
    pub status: String,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LogEvent {
    pub timestamp_ms: i64,
    pub level: String,
    pub service: String,
    pub message: String,
    #[serde(default)]
    pub attrs: BTreeMap<String, String>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuditEvent {
    pub id: Uuid,
    pub timestamp_ms: i64,
    pub action: String,
    pub resource: String,
    pub actor: String,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ErrorGroup {
    pub id: String,
    pub service: String,
    pub message: String,
    pub count: u64,
    pub last_seen_ms: i64,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentInfo {
    pub id: String,
    pub name: String,
    pub capabilities: Vec<String>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MarketplaceApp {
    pub id: String,
    pub name: String,
    pub installed: bool,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Pipeline {
    pub id: String,
    pub name: String,
    pub processors: Vec<String>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProfileMeta {
    pub id: String,
    pub service: String,
    pub profile_type: String,
    pub duration_ms: u64,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VolumeInfo {
    pub id: String,
    pub size_gb: f64,
    pub used_gb: f64,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GpuDevice {
    pub id: String,
    pub model: String,
    pub util: f64,
    pub memory_used_mb: f64,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StreamInfo {
    pub name: String,
    pub lag: u64,
    pub throughput: f64,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DbInstance {
    pub name: String,
    pub engine: String,
    pub qps: f64,
    pub slow_queries: u64,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SdsRule {
    pub id: String,
    pub pattern: String,
    pub enabled: bool,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FleetAgent {
    pub id: String,
    pub version: String,
    pub host: String,
    pub status: String,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CostSummary {
    pub currency: String,
    pub total_month: f64,
    pub by_service: BTreeMap<String, f64>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DoraMetrics {
    pub deploy_frequency_per_day: f64,
    pub lead_time_hours: f64,
    pub change_failure_rate: f64,
    pub mttr_hours: f64,
    pub ai_impact_score: f64,
}
