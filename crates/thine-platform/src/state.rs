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

use crate::tenant::{TenantHub, DEMO_ORG_ID};

const MAX_LOGS: usize = 10_000;
const MAX_SPANS: usize = 5_000;
const MAX_EVENTS: usize = 5_000;
const MAX_AUDIT: usize = 2_000;

#[derive(Debug)]
pub struct PlatformState {
    pub(crate) metrics: Arc<MetricStore>,
    pub(crate) notebooks: DashMap<Uuid, Notebook>,
    pub(crate) teams: DashMap<Uuid, Team>,
    pub(crate) users: DashMap<Uuid, RbacUser>,
    pub(crate) roles: DashMap<Uuid, RbacRole>,
    pub(crate) incidents: DashMap<Uuid, Incident>,
    pub(crate) workflows: DashMap<Uuid, Workflow>,
    pub(crate) workflow_runs: RwLock<VecDeque<WorkflowRun>>,
    pub(crate) slos: DashMap<Uuid, Slo>,
    pub(crate) catalog: DashMap<String, CatalogService>,
    pub(crate) integrations: DashMap<String, Integration>,
    pub(crate) hosts: DashMap<String, HostInfo>,
    pub(crate) spans: RwLock<VecDeque<SpanRecord>>,
    pub(crate) logs: RwLock<VecDeque<LogEvent>>,
    /// DogStatsD / check events (Husky-adjacent event stream).
    pub(crate) events: RwLock<VecDeque<PlatformEvent>>,
    pub(crate) audit: RwLock<VecDeque<AuditEvent>>,
    pub(crate) work: DashMap<Uuid, WorkItem>,
    pub(crate) policies: DashMap<String, serde_json::Value>,
    pub(crate) agents: DashMap<String, AgentInfo>,
    pub(crate) marketplace: DashMap<String, MarketplaceApp>,
    pub(crate) pipelines: DashMap<String, Pipeline>,
    pub(crate) errors: DashMap<String, ErrorGroup>,
    pub(crate) profiles: DashMap<String, ProfileMeta>,
    pub(crate) containers: DashMap<String, ContainerInfo>,
    pub(crate) functions: DashMap<String, serde_json::Value>,
    pub(crate) volumes: DashMap<String, VolumeInfo>,
    pub(crate) gpus: DashMap<String, crate::gpu::GpuDevice>,
    pub(crate) gpu_processes: DashMap<String, crate::gpu::GpuProcess>,
    pub(crate) gpu_samples: RwLock<VecDeque<crate::gpu::GpuSample>>,
    pub(crate) streams: DashMap<String, StreamInfo>,
    pub(crate) db_instances: DashMap<String, DbInstance>,
    pub(crate) data_assets: DashMap<String, serde_json::Value>,
    pub(crate) sds_rules: DashMap<String, SdsRule>,
    pub(crate) fleet: DashMap<String, FleetAgent>,
    pub(crate) network_flows: RwLock<Vec<serde_json::Value>>,
    pub(crate) deploys: RwLock<VecDeque<(i64, String)>>,
    // deep-module stores
    pub(crate) tokens: DashMap<String, crate::models_ext::ApiToken>,
    pub(crate) flow_records: RwLock<VecDeque<crate::models_ext::NetworkFlow>>,
    pub(crate) autoscalers: DashMap<String, crate::models_ext::Autoscaler>,
    pub(crate) serverless: DashMap<String, crate::models_ext::ServerlessFunction>,
    pub(crate) profile_blobs: DashMap<String, String>,
    pub(crate) probes: DashMap<String, crate::models_ext::DynProbe>,
    pub(crate) db_queries: RwLock<VecDeque<crate::models_ext::DbQuerySample>>,
    pub(crate) byoc_sinks: DashMap<String, crate::models_ext::ByocSink>,
    pub(crate) mobile: RwLock<crate::models_ext::MobileConfig>,
    pub(crate) ide_plugins: DashMap<String, crate::models_ext::IdePlugin>,
    pub(crate) notif_channels: DashMap<String, crate::close_gap::NotifChannel>,
    pub(crate) notif_deliveries: RwLock<VecDeque<crate::close_gap::NotifDelivery>>,
    pub(crate) ebpf_agents: DashMap<String, crate::dd_wins::EbpfAgent>,
    pub(crate) usm_endpoints: RwLock<Vec<crate::dd_wins::UsmaEndpoint>>,
    pub(crate) watchdog_anomalies: RwLock<VecDeque<crate::dd_wins::WatchdogAnomaly>>,
    pub(crate) ha_regions: DashMap<String, crate::dd_wins::HaRegion>,
    pub(crate) cloud_resources: DashMap<String, crate::obs18::CloudResource>,
    pub(crate) cloudcraft_views: DashMap<String, crate::obs18::CloudcraftView>,
    pub(crate) live_processes: DashMap<String, crate::obs18::LiveProcess>,
    pub(crate) explain_plans: DashMap<String, crate::obs18::ExplainPlan>,
    pub(crate) db_schemas: DashMap<String, crate::obs18::DbSchemaTable>,
    pub(crate) data_lineage: DashMap<String, crate::obs18::DataLineageEdge>,
    pub(crate) db_samples: RwLock<VecDeque<crate::dbm::DbHostSample>>,
    pub(crate) db_query_metrics: RwLock<Vec<crate::dbm::DbQueryMetric>>,
    pub(crate) db_wait_events: RwLock<Vec<crate::dbm::DbWaitEvent>>,
    pub(crate) db_blocking: RwLock<Vec<crate::dbm::DbBlockingQuery>>,
    pub(crate) db_activity: RwLock<Vec<crate::dbm::DbActivity>>,
    // AI / LangSmith-parity stores
    pub(crate) ai_projects: DashMap<String, crate::ai_obs::AiProject>,
    pub(crate) ai_runs: RwLock<VecDeque<crate::ai_obs::AiRun>>,
    pub(crate) ai_feedback: RwLock<VecDeque<crate::ai_obs::AiFeedback>>,
    pub(crate) ai_datasets: DashMap<String, crate::ai_obs::AiDataset>,
    pub(crate) ai_examples: DashMap<String, crate::ai_obs::AiExample>,
    pub(crate) ai_experiments: DashMap<String, crate::ai_obs::AiExperiment>,
    pub(crate) ai_eval_results: RwLock<VecDeque<crate::ai_obs::AiEvalResult>>,
    pub(crate) ai_graders: DashMap<String, crate::ai_obs::AiGrader>,
    pub(crate) ai_samples: RwLock<VecDeque<crate::ai_obs::AiHostSample>>,
    pub tenants: TenantHub,
}

impl PlatformState {
    pub fn new(metrics: Arc<MetricStore>) -> Arc<Self> {
        let tenants = TenantHub::new();
        tenants.ensure_demo_org();
        Arc::new(Self {
            metrics,
            tenants,
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
            events: RwLock::new(VecDeque::with_capacity(512)),
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
            gpu_processes: DashMap::new(),
            gpu_samples: RwLock::new(VecDeque::new()),
            streams: DashMap::new(),
            db_instances: DashMap::new(),
            data_assets: DashMap::new(),
            sds_rules: DashMap::new(),
            fleet: DashMap::new(),
            network_flows: RwLock::new(Vec::new()),
            deploys: RwLock::new(VecDeque::new()),
            tokens: DashMap::new(),
            flow_records: RwLock::new(VecDeque::new()),
            autoscalers: DashMap::new(),
            serverless: DashMap::new(),
            profile_blobs: DashMap::new(),
            probes: DashMap::new(),
            db_queries: RwLock::new(VecDeque::new()),
            byoc_sinks: DashMap::new(),
            mobile: RwLock::new(crate::models_ext::MobileConfig {
                app_name: "Thine Mobile".into(),
                min_ios: "16.0".into(),
                min_android: "12".into(),
                push_enabled: false,
                deep_links: Vec::new(),
            }),
            ide_plugins: DashMap::new(),
            notif_channels: DashMap::new(),
            notif_deliveries: RwLock::new(VecDeque::new()),
            ebpf_agents: DashMap::new(),
            usm_endpoints: RwLock::new(Vec::new()),
            watchdog_anomalies: RwLock::new(VecDeque::new()),
            ha_regions: DashMap::new(),
            cloud_resources: DashMap::new(),
            cloudcraft_views: DashMap::new(),
            live_processes: DashMap::new(),
            explain_plans: DashMap::new(),
            db_schemas: DashMap::new(),
            data_lineage: DashMap::new(),
            db_samples: RwLock::new(VecDeque::new()),
            db_query_metrics: RwLock::new(Vec::new()),
            db_wait_events: RwLock::new(Vec::new()),
            db_blocking: RwLock::new(Vec::new()),
            db_activity: RwLock::new(Vec::new()),
            ai_projects: DashMap::new(),
            ai_runs: RwLock::new(VecDeque::with_capacity(1024)),
            ai_feedback: RwLock::new(VecDeque::new()),
            ai_datasets: DashMap::new(),
            ai_examples: DashMap::new(),
            ai_experiments: DashMap::new(),
            ai_eval_results: RwLock::new(VecDeque::new()),
            ai_graders: DashMap::new(),
            ai_samples: RwLock::new(VecDeque::new()),
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
        self.create_notebook_for(DEMO_ORG_ID, req)
    }

    pub fn create_notebook_for(&self, org_id: &str, req: CreateNotebook) -> Notebook {
        let nb = Notebook {
            id: Uuid::new_v4(),
            title: req.title,
            cells: req.cells,
            updated_at_ms: Utc::now().timestamp_millis(),
            share_token: None,
            org_id: org_id.into(),
        };
        self.notebooks.insert(nb.id, nb.clone());
        self.audit("create", &format!("notebook:{}", nb.id), "system");
        nb
    }

    pub fn list_notebooks(&self) -> Vec<Notebook> {
        self.list_notebooks_for(DEMO_ORG_ID)
    }

    pub fn list_notebooks_for(&self, org_id: &str) -> Vec<Notebook> {
        let mut v: Vec<_> = self
            .notebooks
            .iter()
            .filter(|e| e.value().org_id == org_id)
            .map(|e| e.value().clone())
            .collect();
        v.sort_by(|a, b| b.updated_at_ms.cmp(&a.updated_at_ms));
        v
    }

    pub fn get_notebook(&self, id: Uuid) -> Option<Notebook> {
        self.notebooks.get(&id).map(|e| e.value().clone())
    }

    pub fn update_notebook(&self, id: Uuid, title: Option<String>, cells: Option<Vec<NotebookCell>>) -> Option<Notebook> {
        let mut entry = self.notebooks.get_mut(&id)?;
        if let Some(t) = title {
            entry.title = t;
        }
        if let Some(c) = cells {
            entry.cells = c;
        }
        entry.updated_at_ms = Utc::now().timestamp_millis();
        self.audit("update", &format!("notebook:{id}"), "system");
        Some(entry.clone())
    }

    pub fn delete_notebook(&self, id: Uuid) -> bool {
        let ok = self.notebooks.remove(&id).is_some();
        if ok {
            self.audit("delete", &format!("notebook:{id}"), "system");
        }
        ok
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
        self.list_catalog_for(DEMO_ORG_ID)
    }

    pub fn list_catalog_for(&self, org_id: &str) -> Vec<CatalogService> {
        self.catalog
            .iter()
            .filter(|e| e.value().org_id == org_id)
            .map(|e| e.value().clone())
            .collect()
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
    pub fn list_gpus(&self) -> Vec<crate::gpu::GpuDevice> {
        self.gpus.iter().map(|e| e.value().clone()).collect()
    }
    pub fn list_volumes(&self) -> Vec<VolumeInfo> {
        self.volumes.iter().map(|e| e.value().clone()).collect()
    }
    pub fn list_functions(&self) -> Vec<serde_json::Value> {
        let detailed = self.list_serverless_detailed();
        if !detailed.is_empty() {
            return detailed
                .into_iter()
                .map(|f| serde_json::to_value(f).unwrap_or_default())
                .collect();
        }
        self.functions.iter().map(|e| e.value().clone()).collect()
    }
    pub fn cost_summary(&self) -> CostSummary {
        let lines = self.cost_detail();
        let mut by_service = BTreeMap::new();
        for line in &lines {
            by_service.insert(line.service.clone(), line.total);
        }
        let total: f64 = lines.iter().map(|l| l.total).sum();
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
        self.recommend_autoscalers()
            .into_iter()
            .map(|a| serde_json::to_value(a).unwrap_or_default())
            .collect()
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
        self.list_apm_services_for(DEMO_ORG_ID)
    }

    pub fn list_apm_services_for(&self, org_id: &str) -> Vec<String> {
        let mut set = std::collections::BTreeSet::new();
        for meta in self.metrics.list_metrics(None) {
            if meta.tags.get("org_id").map(String::as_str) == Some(org_id) {
                if let Some(svc) = meta.tags.get("service") {
                    set.insert(svc.clone());
                }
            }
        }
        for s in self.spans.read().iter() {
            if s.org_id == org_id {
                set.insert(s.service.clone());
            }
        }
        for c in self.catalog.iter() {
            if c.value().org_id == org_id {
                set.insert(c.value().name.clone());
            }
        }
        set.into_iter().collect()
    }
    pub fn usm_services(&self) -> Vec<serde_json::Value> {
        let map = self.usm_map();
        map.get("services")
            .and_then(|s| s.as_array())
            .cloned()
            .unwrap_or_default()
            .into_iter()
            .map(|s| {
                serde_json::json!({
                    "service": s,
                    "protocol": "http",
                    "detected": true,
                    "map": "see /api/v1/usm/map"
                })
            })
            .collect()
    }

    /// Intake from dual-path hub (OTel or native) — same SpanRecord model.
    pub fn ingest_intake_spans(&self, spans: Vec<thine_common::IntakeSpan>) -> usize {
        let mapped = spans
            .into_iter()
            .map(|s| {
                let org_id = s
                    .tags
                    .get("org_id")
                    .cloned()
                    .unwrap_or_else(|| DEMO_ORG_ID.into());
                SpanRecord {
                    trace_id: s.trace_id,
                    span_id: s.span_id,
                    parent_span_id: s.parent_span_id,
                    service: s.service,
                    name: s.name,
                    duration_ms: s.duration_ms,
                    timestamp_ms: s.timestamp_ms,
                    status: s.status,
                    resource: s.resource,
                    org_id,
                }
            })
            .collect();
        self.ingest_spans(mapped)
    }

    pub fn ingest_intake_logs(&self, logs: Vec<thine_common::IntakeLog>) -> usize {
        let n = logs.len();
        for l in logs {
            self.ingest_log(LogEvent {
                timestamp_ms: l.timestamp_ms,
                level: l.level,
                service: l.service,
                message: l.message,
                attrs: l.attrs,
            });
        }
        n
    }

    pub fn ingest_events(&self, events: Vec<thine_common::IntakeEvent>) -> usize {
        let mut q = self.events.write();
        let n = events.len();
        for e in events {
            q.push_back(PlatformEvent {
                timestamp_ms: e.timestamp_ms,
                title: e.title,
                text: e.text,
                alert_type: e.alert_type,
                tags: e.tags,
                source: e.source,
            });
        }
        while q.len() > MAX_EVENTS {
            q.pop_front();
        }
        n
    }

    pub fn list_events(&self, limit: usize) -> Vec<PlatformEvent> {
        self.events.read().iter().rev().take(limit).cloned().collect()
    }

    // —— Logs / errors ——
    pub fn ingest_log(&self, event: LogEvent) {
        let mut q = self.logs.write();
        if event.level == "error" || event.level == "fatal" {
            let key = format!("{}:{}", event.service, &event.message[..event.message.len().min(80)]);
            let frames = stack_frames_from_log(&event);
            let trace_id = event
                .attrs
                .get("trace_id")
                .cloned()
                .or_else(|| event.attrs.get("dd.trace_id").cloned());
            self.errors
                .entry(key.clone())
                .and_modify(|e| {
                    e.count += 1;
                    e.last_seen_ms = event.timestamp_ms;
                    // Auto-regress: any new occurrence after resolve reopens as regressing.
                    if e.status == "resolved" {
                        e.status = "regressing".into();
                        e.resolved_at_ms = None;
                    }
                    if e.stack_frames.is_empty() && !frames.is_empty() {
                        e.stack_frames = frames.clone();
                    }
                    if e.linked_trace_id.is_none() {
                        e.linked_trace_id = trace_id.clone();
                    }
                })
                .or_insert_with(|| ErrorGroup {
                    id: key,
                    service: event.service.clone(),
                    message: event.message.clone(),
                    count: 1,
                    last_seen_ms: event.timestamp_ms,
                    status: "open".into(),
                    assignee: None,
                    first_seen_ms: event.timestamp_ms,
                    resolved_at_ms: None,
                    stack_frames: frames,
                    linked_trace_id: trace_id,
                    linked_issues: Vec::new(),
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
        self.search_logs_query(None, service, level, limit, false).0
    }

    /// Datadog Log Explorer / Live Tail search.
    /// Returns (logs, sample_rate) — sample_rate < 1 when Live Tail samples under load.
    pub fn search_logs_query(
        &self,
        query: Option<&str>,
        service: Option<&str>,
        level: Option<&str>,
        limit: usize,
        live_tail: bool,
    ) -> (Vec<LogEvent>, f64) {
        let clauses = query
            .filter(|q| !q.trim().is_empty())
            .map(crate::logs_query::parse_log_query)
            .unwrap_or_default();
        let mut matched: Vec<LogEvent> = self
            .logs
            .read()
            .iter()
            .rev()
            .filter(|e| service.map(|s| e.service == s).unwrap_or(true))
            .filter(|e| level.map(|l| e.level == l).unwrap_or(true))
            .filter(|e| crate::logs_query::log_matches(e, &clauses))
            .cloned()
            .collect();
        let cap = if live_tail { limit.min(200) } else { limit.min(1000) };
        if live_tail && matched.len() > cap {
            crate::logs_query::sample_logs(matched, cap)
        } else {
            matched.truncate(cap);
            (matched, 1.0)
        }
    }

    pub fn list_errors(&self) -> Vec<ErrorGroup> {
        let mut v: Vec<_> = self.errors.iter().map(|e| e.value().clone()).collect();
        v.sort_by(|a, b| b.count.cmp(&a.count));
        v
    }

    pub fn update_error_group(
        &self,
        id: &str,
        status: Option<String>,
        assignee: Option<String>,
    ) -> Option<ErrorGroup> {
        let mut entry = self.errors.get_mut(id)?;
        if let Some(s) = status {
            let s = s.to_ascii_lowercase();
            if matches!(s.as_str(), "open" | "ignored" | "resolved" | "regressing") {
                if s == "resolved" {
                    entry.resolved_at_ms = Some(Utc::now().timestamp_millis());
                }
                if s == "open" || s == "regressing" {
                    entry.resolved_at_ms = None;
                }
                entry.status = s;
            }
        }
        if let Some(a) = assignee {
            entry.assignee = if a.is_empty() { None } else { Some(a) };
        }
        Some(entry.clone())
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
        self.list_fleet_for(DEMO_ORG_ID)
    }

    pub fn list_fleet_for(&self, org_id: &str) -> Vec<FleetAgent> {
        self.fleet
            .iter()
            .filter(|e| e.value().org_id == org_id)
            .map(|e| e.value().clone())
            .collect()
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
            serde_json::json!({"name": "run_tutorial", "description": "Run Datadog-style monitoring tutorial (agent + dashboard + monitors)"}),
            serde_json::json!({"name": "install_agent", "description": "Bootstrap a fleet agent and seed host metrics"}),
            serde_json::json!({"name": "create_dashboard", "description": "Create Golden Signals timeboard with template variables"}),
            serde_json::json!({"name": "create_monitor", "description": "Create CPU/latency monitors with recovery thresholds"}),
            serde_json::json!({"name": "list_fleet", "description": "List fleet agents and summary"}),
        ]
    }
    pub fn cli_info(&self) -> serde_json::Value {
        self.cli_schema()
    }
    pub fn bits_chat(&self, message: &str) -> serde_json::Value {
        self.bits_chat_deep(message)
    }

    pub fn list_functions_json(&self) -> Vec<serde_json::Value> {
        self.list_serverless_detailed()
            .into_iter()
            .map(|f| serde_json::to_value(f).unwrap_or_default())
            .collect()
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
        self.seed_demo_for(DEMO_ORG_ID);
    }

    pub fn seed_demo_for(&self, org_id: &str) {
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

        self.create_notebook_for(
            org_id,
            CreateNotebook {
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
            },
        );

        for (name, owner) in [("api", &eng.name), ("worker", &eng.name), ("ingest", &eng.name)] {
            self.catalog.insert(
                format!("{org_id}:{name}"),
                CatalogService {
                    name: name.into(),
                    team: owner.clone(),
                    tier: "critical".into(),
                    languages: vec!["rust".into(), "go".into()],
                    org_id: org_id.into(),
                    links: BTreeMap::from([
                        ("repo".into(), format!("https://github.com/thine/{name}")),
                        ("runbook".into(), format!("https://runbooks.thine.local/{name}")),
                    ]),
                    lifecycle: Some("production".into()),
                    definition_yaml: None,
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
                    az: "us-east-1a".into(),
                    instance_type: "m6i.large".into(),
                    agent_version: "0.2.0".into(),
                    cores: 2.0,
                    load_15: 0.55,
                    disk_pct: 48.0,
                    container_count: 1,
                    tags: BTreeMap::from([
                        ("env".into(), "prod".into()),
                        ("service".into(), svc.into()),
                        ("cloud_provider".into(), "aws".into()),
                    ]),
                    alias: format!(
                        "ip-10-0-{}-10",
                        match svc {
                            "api" => 1,
                            "worker" => 2,
                            _ => 3,
                        }
                    ),
                    apps: vec!["aws".into(), "docker".into(), svc.into()],
                },
            );
            self.containers.insert(
                format!("{host}-ctr"),
                ContainerInfo {
                    id: format!("{host}-ctr"),
                    image: format!("thine/{svc}:1.4.2"),
                    host: host.into(),
                    status: "running".into(),
                    name: svc.into(),
                    env: "prod".into(),
                    service: svc.into(),
                    version: "1.4.2".into(),
                    runtime: "docker".into(),
                    kube_namespace: None,
                    pod_name: None,
                    kube_deployment: None,
                    cpu_pct: 22.0,
                    cpu_limit: 1.0,
                    mem_usage_mb: 420.0,
                    mem_limit_mb: 1024.0,
                    mem_rss_mb: 380.0,
                    net_rx_bps: 120_000.0,
                    net_tx_bps: 80_000.0,
                    restarts: 0,
                    started_ms: Utc::now().timestamp_millis() - 86_400_000,
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
        self.create_slo(CreateSlo {
            name: "Checkout availability".into(),
            metric: "http.server.request.count".into(),
            target: 99.9,
            window_ms: Some(86_400_000),
            tags: Some(Tags::from([
                ("env".into(), "prod".into()),
                ("service".into(), "api".into()),
            ])),
        });
        self.create_slo(CreateSlo {
            name: "Worker success rate".into(),
            metric: "http.server.duration".into(),
            target: 99.5,
            window_ms: Some(86_400_000),
            tags: Some(Tags::from([
                ("env".into(), "prod".into()),
                ("service".into(), "worker".into()),
            ])),
        });
        self.create_slo(CreateSlo {
            name: "Ingest freshness".into(),
            metric: "system.memory.usage".into(),
            target: 99.0,
            window_ms: Some(3_600_000),
            tags: Some(Tags::from([
                ("env".into(), "prod".into()),
                ("service".into(), "ingest".into()),
            ])),
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
        // GPUs seeded via seed_gpu_fleet (granular DCGM metrics)
        self.streams.insert(
            "orders".into(),
            StreamInfo {
                name: "orders".into(),
                lag: 42,
                throughput: 1500.0,
            },
        );
        // DB instances expanded in seed_dbm_fleet()
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
            format!("{org_id}:agent-1"),
            FleetAgent {
                id: "agent-1".into(),
                version: "0.1.0".into(),
                host: "i-api-1".into(),
                status: "healthy".into(),
                platform: "linux".into(),
                last_seen_ms: Utc::now().timestamp_millis(),
                config_profile: "standard".into(),
                checks: vec!["cpu".into(), "memory".into(), "disk".into()],
                metrics_enabled: true,
                logs_enabled: true,
                apm_enabled: true,
                org_id: org_id.into(),
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
                parent_span_id: None,
                service: "api".into(),
                name: "http.request".into(),
                duration_ms: 45.0,
                timestamp_ms: now,
                status: "ok".into(),
                resource: None,
                org_id: org_id.into(),
            },
            SpanRecord {
                trace_id: "t1".into(),
                span_id: "s2".into(),
                parent_span_id: Some("s1".into()),
                service: "worker".into(),
                name: "queue.process".into(),
                duration_ms: 12.0,
                timestamp_ms: now,
                status: "ok".into(),
                resource: None,
                org_id: org_id.into(),
            },
        ]);
        self.ingest_log(LogEvent {
            timestamp_ms: now,
            level: "info".into(),
            service: "api".into(),
            message: "request completed".into(),
            attrs: BTreeMap::from([
                ("route".into(), "/pay".into()),
                ("http.status_code".into(), "200".into()),
                ("env".into(), "prod".into()),
            ]),
        });
        self.ingest_log(LogEvent {
            timestamp_ms: now,
            level: "error".into(),
            service: "api".into(),
            message: "upstream timeout talking to payments".into(),
            attrs: BTreeMap::from([
                ("http.status_code".into(), "500".into()),
                ("env".into(), "prod".into()),
                ("http.method".into(), "POST".into()),
            ]),
        });
        self.ingest_log(LogEvent {
            timestamp_ms: now - 30_000,
            level: "error".into(),
            service: "api".into(),
            message: "payment gateway 502".into(),
            attrs: BTreeMap::from([
                ("http.status_code".into(), "502".into()),
                ("env".into(), "prod".into()),
            ]),
        });
        self.ingest_log(LogEvent {
            timestamp_ms: now - 60_000,
            level: "warn".into(),
            service: "api".into(),
            message: "slow canary in staging".into(),
            attrs: BTreeMap::from([
                ("http.status_code".into(), "200".into()),
                ("env".into(), "dev".into()),
            ]),
        });
        for i in 0..24 {
            let code = if i % 7 == 0 { "500" } else if i % 5 == 0 { "503" } else { "200" };
            let level = if code == "200" { "info" } else { "error" };
            self.ingest_log(LogEvent {
                timestamp_ms: now - i * 45_000,
                level: level.into(),
                service: if i % 3 == 0 { "worker".into() } else { "api".into() },
                message: format!("handled request #{i} status={code}"),
                attrs: BTreeMap::from([
                    ("http.status_code".into(), code.into()),
                    ("env".into(), if i % 4 == 0 { "dev".into() } else { "prod".into() }),
                    ("http.method".into(), if i % 2 == 0 { "GET".into() } else { "POST".into() }),
                ]),
            });
        }
        {
            let story: Vec<(i64, &str, &str, &str, &str, Vec<(&str, &str)>)> = vec![
                (6 * 3600_000, "deploy", "info", "Deploy started", "api@1.4.2 canary 5% in us-east-1", vec![("service", "api"), ("version", "1.4.2"), ("impact", "low")]),
                (5 * 3600_000 + 1_800_000, "deploy", "info", "Canary expanded", "api@1.4.2 → 25% traffic", vec![("service", "api"), ("version", "1.4.2"), ("impact", "medium")]),
                (5 * 3600_000, "change", "warning", "Feature flag", "payments.retry_v2 enabled for 25% of traffic", vec![("service", "api"), ("change", "feature_flag"), ("impact", "high")]),
                (4 * 3600_000 + 2_400_000, "monitor", "error", "Monitor triggered", "High latency p95 > 90ms for 5m on api", vec![("service", "api"), ("monitor", "high-latency"), ("impact", "critical")]),
                (4 * 3600_000 + 1_800_000, "watchdog", "error", "Watchdog anomaly", "http.server.duration 5.4σ above baseline for api", vec![("service", "api"), ("metric", "http.server.duration"), ("impact", "critical")]),
                (4 * 3600_000 + 1_200_000, "deploy", "info", "Deploy completed", "api@1.4.2 rolled out to prod (canary → 100%)", vec![("service", "api"), ("version", "1.4.2"), ("impact", "high")]),
                (3 * 3600_000 + 3_000_000, "incident", "error", "Incident opened", "Elevated API latency — sev high, on-call alice", vec![("service", "api"), ("severity", "high"), ("impact", "critical")]),
                (3 * 3600_000 + 2_400_000, "monitor", "error", "Monitor triggered", "High CPU on i-api-1 crossed 85% for 5m", vec![("service", "api"), ("host", "i-api-1"), ("impact", "high")]),
                (3 * 3600_000 + 1_800_000, "agent", "warning", "Agent check warn", "disk.usage on i-ingest-1 at 81%", vec![("host", "i-ingest-1"), ("check", "disk"), ("impact", "medium")]),
                (3 * 3600_000, "change", "info", "Autoscaler scale-out", "api HPA 3 → 6 replicas (cpu target)", vec![("service", "api"), ("change", "hpa"), ("impact", "medium")]),
                (2 * 3600_000 + 2_400_000, "monitor", "success", "Monitor recovered", "High CPU on i-api-1 recovered below 60%", vec![("service", "api"), ("host", "i-api-1"), ("impact", "low")]),
                (2 * 3600_000 + 1_800_000, "watchdog", "warning", "Watchdog anomaly", "worker request count −3.6σ — queue stall suspected", vec![("service", "worker"), ("metric", "http.server.request.count"), ("impact", "high")]),
                (2 * 3600_000 + 900_000, "deploy", "info", "Worker deploy", "worker@2.1.0 started rollout (blue/green)", vec![("service", "worker"), ("version", "2.1.0"), ("impact", "medium")]),
                (2 * 3600_000, "agent", "info", "Agent check", "disk.usage on i-worker-1 is OK (62%)", vec![("host", "i-worker-1"), ("check", "disk"), ("impact", "low")]),
                (90 * 60_000, "synthetic", "error", "Synthetic failed", "Checkout critical path failed in aws:us-east-1 (6.1s)", vec![("test", "syn-checkout"), ("impact", "critical")]),
                (75 * 60_000, "rum", "warning", "RUM spike", "Checkout bounce rate +18pts vs 1h baseline", vec![("view", "/checkout"), ("impact", "high")]),
                (60 * 60_000, "monitor", "error", "SLO burn alert", "API latency SLO burning 4.2× — 2h window", vec![("slo", "API latency SLO"), ("impact", "critical")]),
                (45 * 60_000, "change", "warning", "Config change", "Circuit breaker payments.timeout 2s → 800ms", vec![("service", "api"), ("change", "config"), ("impact", "high")]),
                (30 * 60_000, "watchdog", "error", "Watchdog anomaly", "USM error rate spike on api without matching APM tags", vec![("service", "api"), ("impact", "high")]),
                (20 * 60_000, "incident", "warning", "Incident update", "Mitigation: retry_v2 rolled back to 0%", vec![("service", "api"), ("impact", "high")]),
                (12 * 60_000, "monitor", "success", "Monitor recovered", "High latency recovered under 70ms p95", vec![("service", "api"), ("impact", "medium")]),
                (8 * 60_000, "deploy", "info", "Rollback complete", "payments.retry_v2 disabled globally", vec![("service", "api"), ("impact", "high")]),
                (5 * 60_000, "agent", "info", "Fleet heartbeat", "3 agents healthy — DogStatsD 6.0/s", vec![("fleet", "ok"), ("impact", "low")]),
                (2 * 60_000, "synthetic", "success", "Synthetic recovered", "Checkout critical path OK (2.2s)", vec![("test", "syn-checkout"), ("impact", "medium")]),
                (60_000, "incident", "success", "Incident mitigated", "API latency back to baseline — monitoring", vec![("service", "api"), ("impact", "high")]),
            ];
            let mut batch = Vec::with_capacity(story.len());
            for (ago, source, alert_type, title, text, tag_pairs) in story {
                let mut tags = Tags::from([("env".into(), "prod".into())]);
                for (k, v) in tag_pairs {
                    tags.insert(k.into(), v.into());
                }
                batch.push(thine_common::IntakeEvent {
                    timestamp_ms: now - ago,
                    title: title.into(),
                    text: text.into(),
                    alert_type: alert_type.into(),
                    tags,
                    source: source.into(),
                });
            }
            self.ingest_events(batch);
        }
        self.audit("seed", "platform", "system");
        self.seed_deep();
        self.seed_close_gap();
        self.seed_dd_wins();
        self.seed_obs18();
        self.seed_gpu_fleet();
        self.seed_dbm_fleet();
        self.seed_ai_obs();
        self.seed_infra_fleet();
    }
}

// —— Types ——

fn default_org_id() -> String {
    DEMO_ORG_ID.into()
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Notebook {
    pub id: Uuid,
    pub title: String,
    pub cells: Vec<NotebookCell>,
    pub updated_at_ms: i64,
    #[serde(default)]
    pub share_token: Option<String>,
    #[serde(default = "default_org_id")]
    pub org_id: String,
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
    #[serde(default)]
    pub lifecycle: Option<String>,
    #[serde(default)]
    pub definition_yaml: Option<String>,
    #[serde(default = "default_org_id")]
    pub org_id: String,
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
    #[serde(default)]
    pub az: String,
    #[serde(default)]
    pub instance_type: String,
    #[serde(default)]
    pub agent_version: String,
    #[serde(default)]
    pub cores: f64,
    #[serde(default)]
    pub load_15: f64,
    #[serde(default)]
    pub disk_pct: f64,
    #[serde(default)]
    pub container_count: u32,
    #[serde(default)]
    pub tags: BTreeMap<String, String>,
    #[serde(default)]
    pub alias: String,
    #[serde(default)]
    pub apps: Vec<String>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContainerInfo {
    pub id: String,
    pub image: String,
    pub host: String,
    pub status: String,
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub env: String,
    #[serde(default)]
    pub service: String,
    #[serde(default)]
    pub version: String,
    #[serde(default)]
    pub runtime: String,
    #[serde(default)]
    pub kube_namespace: Option<String>,
    #[serde(default)]
    pub pod_name: Option<String>,
    #[serde(default)]
    pub kube_deployment: Option<String>,
    #[serde(default)]
    pub cpu_pct: f64,
    #[serde(default)]
    pub cpu_limit: f64,
    #[serde(default)]
    pub mem_usage_mb: f64,
    #[serde(default)]
    pub mem_limit_mb: f64,
    #[serde(default)]
    pub mem_rss_mb: f64,
    #[serde(default)]
    pub net_rx_bps: f64,
    #[serde(default)]
    pub net_tx_bps: f64,
    #[serde(default)]
    pub restarts: u32,
    #[serde(default)]
    pub started_ms: i64,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SpanRecord {
    pub trace_id: String,
    pub span_id: String,
    #[serde(default)]
    pub parent_span_id: Option<String>,
    pub service: String,
    pub name: String,
    pub duration_ms: f64,
    pub timestamp_ms: i64,
    pub status: String,
    #[serde(default)]
    pub resource: Option<String>,
    #[serde(default = "default_org_id")]
    pub org_id: String,
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
pub struct PlatformEvent {
    pub timestamp_ms: i64,
    pub title: String,
    pub text: String,
    #[serde(default)]
    pub alert_type: String,
    #[serde(default)]
    pub tags: BTreeMap<String, String>,
    #[serde(default)]
    pub source: String,
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
    #[serde(default = "default_error_status")]
    pub status: String, // open | ignored | resolved | regressing
    #[serde(default)]
    pub assignee: Option<String>,
    #[serde(default)]
    pub first_seen_ms: i64,
    #[serde(default)]
    pub resolved_at_ms: Option<i64>,
    /// Stack frames (top → bottom) — Datadog Error Tracking detail.
    #[serde(default)]
    pub stack_frames: Vec<String>,
    #[serde(default)]
    pub linked_trace_id: Option<String>,
    #[serde(default)]
    pub linked_issues: Vec<String>,
}

fn default_error_status() -> String {
    "open".into()
}

fn stack_frames_from_log(event: &LogEvent) -> Vec<String> {
    if let Some(stack) = event.attrs.get("stack").or_else(|| event.attrs.get("error.stack")) {
        return stack
            .lines()
            .map(|l| l.trim().to_string())
            .filter(|l| !l.is_empty())
            .take(24)
            .collect();
    }
    // Synthesize a Datadog-like frame list from service + message for demo fidelity.
    vec![
        format!("{}::handler", event.service),
        "middleware::trace".into(),
        "runtime::block_on".into(),
        format!("caused by: {}", event.message.chars().take(80).collect::<String>()),
    ]
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
    #[serde(default)]
    pub host: String,
    #[serde(default = "default_primary")]
    pub role: String,
    #[serde(default)]
    pub version: String,
    #[serde(default)]
    pub port: u16,
    #[serde(default)]
    pub connections: f64,
    #[serde(default)]
    pub active_connections: f64,
    #[serde(default)]
    pub idle_connections: f64,
    #[serde(default)]
    pub waiting_connections: f64,
    #[serde(default)]
    pub max_connections: f64,
    #[serde(default)]
    pub connections_pct: f64,
    #[serde(default)]
    pub tps: f64,
    #[serde(default)]
    pub rollbacks_per_sec: f64,
    #[serde(default)]
    pub avg_query_ms: f64,
    #[serde(default)]
    pub p95_query_ms: f64,
    #[serde(default)]
    pub p99_query_ms: f64,
    #[serde(default)]
    pub rows_returned_per_sec: f64,
    #[serde(default)]
    pub rows_fetched_per_sec: f64,
    #[serde(default)]
    pub rows_inserted_per_sec: f64,
    #[serde(default)]
    pub rows_updated_per_sec: f64,
    #[serde(default)]
    pub rows_deleted_per_sec: f64,
    #[serde(default)]
    pub buffer_hit_ratio: f64,
    #[serde(default)]
    pub blocks_hit_per_sec: f64,
    #[serde(default)]
    pub blocks_read_per_sec: f64,
    #[serde(default)]
    pub temp_bytes_per_sec: f64,
    #[serde(default)]
    pub deadlocks_per_sec: f64,
    #[serde(default)]
    pub locks_waiting: f64,
    #[serde(default)]
    pub avg_lock_wait_ms: f64,
    #[serde(default)]
    pub replication_lag_ms: f64,
    #[serde(default)]
    pub disk_read_ops: f64,
    #[serde(default)]
    pub disk_write_ops: f64,
    #[serde(default)]
    pub cpu_pct: f64,
    #[serde(default)]
    pub mem_used_pct: f64,
    #[serde(default)]
    pub dead_rows: u64,
    #[serde(default)]
    pub live_rows: u64,
    #[serde(default)]
    pub autovacuum_workers: u32,
    #[serde(default)]
    pub index_bloat_pct: f64,
    #[serde(default)]
    pub table_bloat_pct: f64,
    #[serde(default)]
    pub buffer_pool_utilization: f64,
    #[serde(default)]
    pub buffer_pool_bytes: f64,
    #[serde(default)]
    pub buffer_pool_dirty_bytes: f64,
    #[serde(default)]
    pub tmp_tables_per_sec: f64,
    #[serde(default)]
    pub tmp_disk_tables_per_sec: f64,
    #[serde(default)]
    pub open_files: u32,
    #[serde(default)]
    pub connection_errors_per_sec: f64,
    #[serde(default)]
    pub uptime_hours: f64,
    #[serde(default)]
    pub size_gb: f64,
}

fn default_primary() -> String {
    "primary".into()
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
    #[serde(default)]
    pub platform: String,
    #[serde(default)]
    pub last_seen_ms: i64,
    #[serde(default)]
    pub config_profile: String,
    #[serde(default)]
    pub checks: Vec<String>,
    #[serde(default)]
    pub metrics_enabled: bool,
    #[serde(default)]
    pub logs_enabled: bool,
    #[serde(default)]
    pub apm_enabled: bool,
    #[serde(default = "default_org_id")]
    pub org_id: String,
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
