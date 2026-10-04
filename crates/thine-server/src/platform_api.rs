//! Routes for Datadog-parity platform modules.

use crate::state::AppState;
use axum::extract::ws::{Message, WebSocket, WebSocketUpgrade};
use axum::extract::{Path, Query, State};
use axum::http::{HeaderMap, StatusCode};
use axum::response::IntoResponse;
use axum::routing::{delete, get, post};
use axum::{Json, Router};
use futures::{SinkExt, StreamExt};
use serde::Deserialize;
use serde_json::json;
use thine_platform::{
    all_features, feature_stats_with_parity, AgentInfo, AiCreateRun, Autoscaler, ByocSink,
    CloudcraftView, ContainerInfo, CreateIncident, CreateNotebook, CreateSlo, CreateTeam,
    CreateWorkflow, DynProbe, GpuDevice, LogEvent, MobileConfig, NetworkFlow, NotifChannel,
    Pipeline, ProfileMeta, SdsRule, ServerlessFunction, SpanRecord, VolumeInfo,
};

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/api/v1/features", get(features))
        .route("/api/v1/features/stats", get(features_stats))
        .route("/api/v1/notebooks", get(list_notebooks).post(create_notebook))
        .route(
            "/api/v1/notebooks/{id}",
            get(get_notebook).put(update_notebook).delete(delete_notebook),
        )
        .route("/api/v1/notebooks/{id}/share", post(share_notebook))
        .route("/api/v1/shared/notebooks/{token}", get(get_shared_notebook))
        .route("/api/v1/teams", get(list_teams).post(create_team))
        .route("/api/v1/rbac/users", get(list_users))
        .route("/api/v1/rbac/roles", get(list_roles))
        .route("/api/v1/auth/token", post(issue_token))
        .route("/api/v1/auth/me", get(auth_me))
        .route("/api/v1/incidents", get(list_incidents).post(create_incident))
        .route("/api/v1/incidents/{id}/escalate", post(escalate_incident))
        .route("/api/v1/work", get(list_work))
        .route("/api/v1/workflows", get(list_workflows).post(create_workflow))
        .route("/api/v1/workflows/runs", get(list_workflow_runs))
        .route("/api/v1/workflows/{id}/run", post(run_workflow))
        .route("/api/v1/slos", get(list_slos).post(create_slo))
        .route("/api/v1/slos/budgets", get(slo_budgets))
        .route("/api/v1/catalog/services", get(list_catalog))
        .route("/api/v1/catalog/scorecards", get(list_scorecards))
        .route(
            "/api/v1/catalog/services/{name}/definition",
            get(get_service_definition).put(put_service_definition),
        )
        .route(
            "/api/v1/catalog/services/{name}/scorecard",
            get(get_service_scorecard),
        )
        .route("/api/v1/integrations", get(list_integrations))
        .route(
            "/api/v1/integrations/{id}",
            axum::routing::patch(patch_integration),
        )
        .route("/api/v1/infra/hosts", get(list_hosts))
        .route("/api/v1/infra/hosts/live", get(host_live))
        .route("/api/v1/containers", get(list_containers).post(upsert_container))
        .route("/api/v1/containers/stats", get(container_stats))
        .route("/api/v1/containers/explorer", get(containers_explorer))
        .route("/api/v1/infra/envs", get(infra_envs))
        .route("/api/v1/infra/hostmap", get(infra_hostmap))
        .route("/api/v1/infra/containermap", get(kube_container_map))
        .route("/api/v1/infra/k8s/utilization", get(k8s_utilization))
        .route("/api/v1/gpu/devices", get(list_gpus).post(upsert_gpu))
        .route("/api/v1/gpu/summary", get(gpu_summary))
        .route("/api/v1/gpu/fleet", get(gpu_fleet))
        .route("/api/v1/gpu/health", get(gpu_health))
        .route("/api/v1/gpu/processes", get(gpu_processes))
        .route("/api/v1/gpu/samples", get(gpu_samples))
        .route("/api/v1/gpu/metrics", get(gpu_metrics_catalog))
        .route("/api/v1/gpu/emit", post(gpu_emit_metrics))
        .route("/api/v1/storage/volumes", get(list_volumes).post(upsert_volume))
        .route(
            "/api/v1/serverless/functions",
            get(list_functions).post(upsert_function),
        )
        .route("/api/v1/serverless/overview", get(serverless_overview))
        .route("/api/v1/cost/summary", get(cost_summary))
        .route("/api/v1/cost/detail", get(cost_detail))
        .route("/api/v1/network/flows", get(network_flows).post(ingest_flows))
        .route(
            "/api/v1/k8s/autoscalers",
            get(autoscalers).post(upsert_autoscaler),
        )
        .route("/api/v1/apm/traces", get(list_traces).post(ingest_traces))
        .route("/api/v1/apm/traces/{id}", get(trace_tree))
        .route("/api/v1/apm/services", get(apm_services))
        .route("/api/v1/apm/stats", get(apm_stats))
        .route("/api/v1/apm/services/{name}", get(apm_service_page))
        .route("/api/v1/usm/services", get(usm_services))
        .route("/api/v1/usm/map", get(usm_map))
        .route("/api/v1/usm/ebpf", get(usm_ebpf))
        .route("/api/v1/usm/discover", post(usm_discover))
        .route("/api/v1/service-map", get(service_map))
        .route("/api/v1/watchdog/scan", post(watchdog_scan))
        .route("/api/v1/watchdog/anomalies", get(watchdog_anomalies))
        .route("/api/v1/watchdog/summary", get(watchdog_summary))
        .route("/api/v1/ha/status", get(ha_status))
        .route("/api/v1/ha/regions", get(ha_regions))
        .route("/api/v1/ha/failover", post(ha_failover))
        .route("/api/v1/ha/replicate", post(ha_replicate))
        .route("/api/v1/integrations/search", get(integrations_search))
        .route(
            "/api/v1/integrations/{id}/health",
            get(integration_health),
        )
        .route("/api/v1/logs/ingest", post(ingest_logs))
        .route("/api/v1/logs/search", get(search_logs))
        .route("/api/v1/logs/tail/ws", get(logs_tail_ws))
        .route("/api/v1/logs/patterns", get(list_log_patterns))
        .route("/api/v1/logs/facets", get(log_facets))
        .route("/api/v1/logs/aggregate", get(log_aggregate))
        .route("/api/v1/query/formula", get(query_formula).post(query_formula_post))
        .route("/api/v1/notify/channels", get(list_notif).post(upsert_notif))
        .route("/api/v1/notify/deliveries", get(list_deliveries))
        .route("/api/v1/notify/sync", post(sync_notify))
        .route("/api/v1/compare/datadog", get(compare_datadog))
        .route("/api/v1/compare", get(compare_competitors))
        .route("/api/v1/compare/competitors", get(compare_competitors))
        .route("/api/v1/audit", get(list_audit))
        .route("/api/v1/errors", get(list_errors))
        .route("/api/v1/errors/{id}", axum::routing::patch(patch_error))
        .route("/api/v1/errors/{id}/link", post(link_error_issue))
        .route("/api/v1/notifications/test", post(test_notification))
        .route("/api/v1/agents", get(list_agents).post(upsert_agent))
        .route("/api/v1/agents/{id}", delete(delete_agent))
        .route("/api/v1/marketplace", get(list_marketplace))
        .route("/api/v1/marketplace/{id}/install", post(install_app))
        .route("/api/v1/marketplace/{id}/uninstall", post(uninstall_app))
        .route("/api/v1/pipelines", get(list_pipelines).post(upsert_pipeline))
        .route("/api/v1/pipelines/{id}/run", post(run_pipeline))
        .route(
            "/api/v1/profiler/profiles",
            get(list_profiles).post(upload_profile),
        )
        .route("/api/v1/profiler/profiles/{id}/blob", get(profile_blob))
        .route("/api/v1/streams", get(list_streams))
        .route("/api/v1/dbm/instances", get(list_db))
        .route("/api/v1/dbm/queries", get(top_queries))
        .route("/api/v1/dbm/summary", get(dbm_summary))
        .route("/api/v1/dbm/health", get(dbm_health))
        .route("/api/v1/dbm/metrics", get(dbm_metrics_catalog))
        .route("/api/v1/dbm/samples", get(dbm_samples))
        .route("/api/v1/dbm/query_metrics", get(dbm_query_metrics))
        .route("/api/v1/dbm/waits", get(dbm_waits))
        .route("/api/v1/dbm/blocking", get(dbm_blocking))
        .route("/api/v1/dbm/activity", get(dbm_activity))
        .route("/api/v1/dbm/emit", post(dbm_emit_metrics))
        .route("/api/v1/ai/summary", get(ai_summary))
        .route("/api/v1/ai/projects", get(ai_projects))
        .route("/api/v1/ai/runs", get(ai_runs).post(ai_create_run))
        .route("/api/v1/ai/runs/batch", post(ai_runs_batch))
        .route("/api/v1/ai/feedback", get(ai_feedback))
        .route("/api/v1/ai/datasets", get(ai_datasets))
        .route("/api/v1/ai/examples", get(ai_examples))
        .route("/api/v1/ai/experiments", get(ai_experiments))
        .route("/api/v1/ai/experiments/run", post(ai_run_experiment))
        .route("/api/v1/ai/evals", get(ai_evals))
        .route("/api/v1/ai/graders", get(ai_graders))
        .route("/api/v1/ai/samples", get(ai_samples))
        .route("/api/v1/ai/metrics", get(ai_metrics_catalog))
        .route("/api/v1/ai/health", get(ai_health))
        .route("/api/v1/ai/emit", post(ai_emit_metrics))
        .route("/api/v1/data/assets", get(list_data_assets))
        .route("/api/v1/sds/rules", get(list_sds).post(upsert_sds))
        .route("/api/v1/sds/scan", get(sds_scan))
        .route("/api/v1/fleet/agents", get(list_fleet))
        .route("/api/v1/fleet/summary", get(fleet_summary))
        .route("/api/v1/fleet/heartbeat", post(fleet_heartbeat))
        .route("/api/v1/fleet/configure", post(fleet_configure))
        .route("/api/v1/fleet/rollout", post(fleet_rollout))
        .route("/api/v1/fleet/bootstrap", post(crate::fleet_agent::fleet_bootstrap))
        .route(
            "/api/v1/fleet/bootstrap/stop",
            post(crate::fleet_agent::fleet_bootstrap_stop),
        )
        .route("/install/{name}", get(crate::fleet_agent::install_script))
        .route("/api/v1/governance/policies", get(list_policies))
        .route("/api/v1/dora", get(dora))
        .route("/api/v1/ux/overview", get(ux_overview))
        .route("/api/v1/ux/sessions", get(ux_sessions))
        .route("/api/v1/ux/vitals", get(ux_vitals))
        .route("/api/v1/ux/synthetics", get(ux_synthetics))
        .route("/api/v1/ux/timeseries", get(ux_timeseries))
        .route("/api/v1/ux/pages", get(ux_pages))
        .route("/api/v1/ux/funnel", get(ux_funnel))
        .route("/api/v1/bits/chat", post(bits_chat))
        .route("/api/v1/mcp/tools", get(mcp_tools))
        .route("/api/v1/mcp/invoke", post(mcp_invoke))
        .route("/api/v1/cli/info", get(cli_info))
        .route("/api/v1/probes", get(list_probes).post(upsert_probe))
        .route("/api/v1/probes/{id}", delete(delete_probe))
        .route("/api/v1/byoc/sinks", get(list_byoc).post(upsert_byoc))
        .route("/api/v1/byoc/sinks/{id}/forward", post(forward_byoc))
        .route("/api/v1/mobile/config", get(mobile_config).post(set_mobile))
        .route("/api/v1/ide/plugins", get(list_ide))
        .route("/api/v1/cloudcraft/diagram", get(cloudcraft_diagram))
        .route("/api/v1/cloudcraft/resources", get(cloudcraft_resources))
        .route("/api/v1/cloudcraft/views", get(cloudcraft_views).post(save_cloudcraft_view))
        .route("/api/v1/infra/processes", get(live_processes))
        .route("/api/v1/infra/processes/explorer", get(processes_explorer))
        .route("/api/v1/usm/red", get(usm_red))
        .route("/api/v1/dbm/explain", get(dbm_explain))
        .route("/api/v1/dbm/schema", get(dbm_schema))
        .route("/api/v1/dbm/apm", get(dbm_apm))
        .route("/api/v1/data/lineage", get(data_lineage))
        .route("/api/v1/cost/recommendations", get(cost_recs))
        .route(
            "/api/v1/profiler/profiles/{id}/flame",
            get(profiler_flame),
        )
        .route("/api/v1/pipelines/workers", get(pipeline_workers))
}

fn bearer(headers: &HeaderMap) -> Option<&str> {
    headers
        .get(axum::http::header::AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.strip_prefix("Bearer ").or_else(|| v.strip_prefix("bearer ")))
}

async fn features() -> impl IntoResponse {
    Json(all_features())
}
async fn features_stats() -> impl IntoResponse {
    Json(feature_stats_with_parity())
}

async fn list_notebooks(State(s): State<AppState>) -> impl IntoResponse {
    Json(s.platform.list_notebooks())
}
async fn create_notebook(
    State(s): State<AppState>,
    Json(req): Json<CreateNotebook>,
) -> impl IntoResponse {
    (StatusCode::CREATED, Json(s.platform.create_notebook(req)))
}
async fn get_notebook(State(s): State<AppState>, Path(id): Path<uuid::Uuid>) -> impl IntoResponse {
    match s.platform.get_notebook(id) {
        Some(nb) => Json(nb).into_response(),
        None => (StatusCode::NOT_FOUND, Json(json!({ "error": "notebook not found" }))).into_response(),
    }
}
#[derive(Deserialize)]
struct UpdateNotebookBody {
    title: Option<String>,
    cells: Option<Vec<thine_platform::NotebookCell>>,
}
async fn update_notebook(
    State(s): State<AppState>,
    Path(id): Path<uuid::Uuid>,
    Json(body): Json<UpdateNotebookBody>,
) -> impl IntoResponse {
    match s.platform.update_notebook(id, body.title, body.cells) {
        Some(nb) => Json(nb).into_response(),
        None => (StatusCode::NOT_FOUND, Json(json!({ "error": "notebook not found" }))).into_response(),
    }
}
async fn delete_notebook(State(s): State<AppState>, Path(id): Path<uuid::Uuid>) -> impl IntoResponse {
    if s.platform.delete_notebook(id) {
        StatusCode::NO_CONTENT.into_response()
    } else {
        (StatusCode::NOT_FOUND, Json(json!({ "error": "notebook not found" }))).into_response()
    }
}
async fn apm_service_page(State(s): State<AppState>, Path(name): Path<String>) -> impl IntoResponse {
    match s.platform.apm_service_page(&name) {
        Some(v) => Json(v).into_response(),
        None => (StatusCode::NOT_FOUND, Json(json!({ "error": "service not found" }))).into_response(),
    }
}
#[derive(Deserialize)]
struct PatchErrorBody {
    status: Option<String>,
    assignee: Option<String>,
}
async fn patch_error(
    State(s): State<AppState>,
    Path(id): Path<String>,
    Json(body): Json<PatchErrorBody>,
) -> impl IntoResponse {
    // Path may URL-encode id with colons
    let id = urlencoding_decode(&id);
    match s.platform.update_error_group(&id, body.status, body.assignee) {
        Some(e) => Json(e).into_response(),
        None => (StatusCode::NOT_FOUND, Json(json!({ "error": "error group not found" }))).into_response(),
    }
}
#[derive(Deserialize)]
struct TestNotifyBody {
    recipients: String,
    rule_name: String,
    metric: String,
}
async fn test_notification(
    State(s): State<AppState>,
    Json(body): Json<TestNotifyBody>,
) -> impl IntoResponse {
    Json(s.platform.test_notification(&body.recipients, &body.rule_name, &body.metric))
}
fn urlencoding_decode(s: &str) -> String {
    // Minimal decode for %3A → :
    s.replace("%3A", ":").replace("%3a", ":").replace("%2F", "/").replace("%20", " ")
}

async fn list_teams(State(s): State<AppState>) -> impl IntoResponse {
    Json(s.platform.list_teams())
}
async fn create_team(State(s): State<AppState>, Json(req): Json<CreateTeam>) -> impl IntoResponse {
    (StatusCode::CREATED, Json(s.platform.create_team(req)))
}
async fn list_users(State(s): State<AppState>) -> impl IntoResponse {
    Json(s.platform.list_users())
}
async fn list_roles(State(s): State<AppState>) -> impl IntoResponse {
    Json(s.platform.list_roles())
}

#[derive(Deserialize)]
struct IssueTokenReq {
    email: String,
    #[serde(default = "default_roles")]
    roles: Vec<String>,
}
fn default_roles() -> Vec<String> {
    vec!["viewer".into()]
}
async fn issue_token(
    State(s): State<AppState>,
    Json(req): Json<IssueTokenReq>,
) -> impl IntoResponse {
    (
        StatusCode::CREATED,
        Json(s.platform.issue_token(&req.email, req.roles)),
    )
}
async fn auth_me(State(s): State<AppState>, headers: HeaderMap) -> impl IntoResponse {
    match bearer(&headers).and_then(|t| s.platform.authenticate(t)) {
        Some(tok) => Json(tok).into_response(),
        None => (
            StatusCode::UNAUTHORIZED,
            Json(json!({ "error": "missing or invalid bearer token" })),
        )
            .into_response(),
    }
}

async fn list_incidents(State(s): State<AppState>) -> impl IntoResponse {
    Json(s.platform.list_incidents())
}
async fn create_incident(
    State(s): State<AppState>,
    Json(req): Json<CreateIncident>,
) -> impl IntoResponse {
    (StatusCode::CREATED, Json(s.platform.create_incident(req)))
}
async fn list_work(State(s): State<AppState>) -> impl IntoResponse {
    Json(s.platform.list_work())
}
async fn list_workflows(State(s): State<AppState>) -> impl IntoResponse {
    Json(s.platform.list_workflows())
}
async fn create_workflow(
    State(s): State<AppState>,
    Json(req): Json<CreateWorkflow>,
) -> impl IntoResponse {
    (StatusCode::CREATED, Json(s.platform.create_workflow(req)))
}
async fn list_workflow_runs(State(s): State<AppState>) -> impl IntoResponse {
    Json(s.platform.list_workflow_runs(50))
}

async fn list_slos(State(s): State<AppState>) -> impl IntoResponse {
    let platform = s.platform.clone();
    match tokio::task::spawn_blocking(move || platform.list_slos()).await {
        Ok(v) => Json(v).into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({ "error": e.to_string() })),
        )
            .into_response(),
    }
}
async fn create_slo(State(s): State<AppState>, Json(req): Json<CreateSlo>) -> impl IntoResponse {
    (StatusCode::CREATED, Json(s.platform.create_slo(req)))
}

async fn list_catalog(State(s): State<AppState>) -> impl IntoResponse {
    Json(s.platform.list_catalog())
}
async fn list_integrations(State(s): State<AppState>) -> impl IntoResponse {
    Json(s.platform.list_integrations())
}

#[derive(Deserialize)]
struct PatchIntegration {
    enabled: bool,
}
async fn patch_integration(
    State(s): State<AppState>,
    Path(id): Path<String>,
    Json(body): Json<PatchIntegration>,
) -> impl IntoResponse {
    match s.platform.set_integration(&id, body.enabled) {
        Some(i) => Json(i).into_response(),
        None => (
            StatusCode::NOT_FOUND,
            Json(json!({ "error": "integration not found" })),
        )
            .into_response(),
    }
}

async fn list_hosts(State(s): State<AppState>) -> impl IntoResponse {
    Json(s.platform.list_hosts())
}
async fn list_containers(State(s): State<AppState>) -> impl IntoResponse {
    Json(s.platform.list_containers())
}
async fn upsert_container(
    State(s): State<AppState>,
    Json(c): Json<ContainerInfo>,
) -> impl IntoResponse {
    (StatusCode::CREATED, Json(s.platform.upsert_container(c)))
}
async fn container_stats(State(s): State<AppState>) -> impl IntoResponse {
    Json(s.platform.container_stats())
}

#[derive(Deserialize)]
struct ContainerExplorerQ {
    env: Option<String>,
    namespace: Option<String>,
    host: Option<String>,
    service: Option<String>,
    q: Option<String>,
}
async fn containers_explorer(
    State(s): State<AppState>,
    Query(q): Query<ContainerExplorerQ>,
) -> impl IntoResponse {
    Json(s.platform.containers_explorer(
        q.env.as_deref(),
        q.namespace.as_deref(),
        q.host.as_deref(),
        q.service.as_deref(),
        q.q.as_deref(),
    ))
}

async fn infra_envs(State(s): State<AppState>) -> impl IntoResponse {
    Json(s.platform.infra_envs())
}

#[derive(Deserialize)]
struct HostmapQ {
    group_by: Option<String>,
    resource: Option<String>,
}
async fn infra_hostmap(State(s): State<AppState>, Query(q): Query<HostmapQ>) -> impl IntoResponse {
    Json(
        s.platform
            .infra_hostmap(q.group_by.as_deref(), q.resource.as_deref()),
    )
}

async fn k8s_utilization(State(s): State<AppState>) -> impl IntoResponse {
    Json(s.platform.k8s_resource_utilization())
}
async fn list_gpus(State(s): State<AppState>) -> impl IntoResponse {
    Json(s.platform.list_gpus())
}
async fn upsert_gpu(State(s): State<AppState>, Json(g): Json<GpuDevice>) -> impl IntoResponse {
    (StatusCode::CREATED, Json(s.platform.upsert_gpu(g)))
}
async fn gpu_summary(State(s): State<AppState>) -> impl IntoResponse {
    Json(s.platform.gpu_summary())
}
async fn gpu_fleet(State(s): State<AppState>) -> impl IntoResponse {
    Json(s.platform.gpu_fleet())
}
async fn gpu_health(State(s): State<AppState>) -> impl IntoResponse {
    Json(s.platform.gpu_health())
}
#[derive(Deserialize)]
struct GpuQuery {
    gpu_id: Option<String>,
    limit: Option<usize>,
}
async fn gpu_processes(
    State(s): State<AppState>,
    Query(q): Query<GpuQuery>,
) -> impl IntoResponse {
    Json(s.platform.list_gpu_processes(q.gpu_id.as_deref()))
}
async fn gpu_samples(State(s): State<AppState>, Query(q): Query<GpuQuery>) -> impl IntoResponse {
    Json(
        s.platform
            .list_gpu_samples(q.gpu_id.as_deref(), q.limit.unwrap_or(100)),
    )
}
async fn gpu_metrics_catalog(State(s): State<AppState>) -> impl IntoResponse {
    Json(s.platform.gpu_metrics_catalog_json())
}
async fn gpu_emit_metrics(State(s): State<AppState>) -> impl IntoResponse {
    let n = s.platform.emit_gpu_metrics(None);
    Json(serde_json::json!({ "points_emitted": n }))
}
async fn list_volumes(State(s): State<AppState>) -> impl IntoResponse {
    Json(s.platform.list_volumes())
}
async fn upsert_volume(State(s): State<AppState>, Json(v): Json<VolumeInfo>) -> impl IntoResponse {
    (StatusCode::CREATED, Json(s.platform.upsert_volume(v)))
}
async fn list_functions(State(s): State<AppState>) -> impl IntoResponse {
    Json(s.platform.list_serverless_detailed())
}
async fn upsert_function(
    State(s): State<AppState>,
    Json(f): Json<ServerlessFunction>,
) -> impl IntoResponse {
    (StatusCode::CREATED, Json(s.platform.upsert_function(f)))
}
async fn cost_summary(State(s): State<AppState>) -> impl IntoResponse {
    Json(s.platform.cost_summary())
}
async fn cost_detail(State(s): State<AppState>) -> impl IntoResponse {
    Json(s.platform.cost_detail())
}
async fn network_flows(State(s): State<AppState>) -> impl IntoResponse {
    Json(s.platform.network_flows())
}
async fn ingest_flows(
    State(s): State<AppState>,
    Json(flows): Json<Vec<NetworkFlow>>,
) -> impl IntoResponse {
    Json(json!({ "accepted": s.platform.ingest_flows(flows) }))
}
async fn autoscalers(State(s): State<AppState>) -> impl IntoResponse {
    Json(s.platform.list_autoscalers())
}
async fn upsert_autoscaler(
    State(s): State<AppState>,
    Json(a): Json<Autoscaler>,
) -> impl IntoResponse {
    (StatusCode::CREATED, Json(s.platform.upsert_autoscaler(a)))
}

#[derive(Deserialize)]
struct LimitQuery {
    limit: Option<usize>,
}
async fn list_traces(State(s): State<AppState>, Query(q): Query<LimitQuery>) -> impl IntoResponse {
    Json(s.platform.list_traces(q.limit.unwrap_or(50)))
}
async fn ingest_traces(
    State(s): State<AppState>,
    Json(spans): Json<Vec<SpanRecord>>,
) -> impl IntoResponse {
    // Native APM path → same intake hub as OTLP /v1/traces (normalize + APM stats fan-out).
    let intake: Vec<thine_common::IntakeSpan> = spans
        .into_iter()
        .map(|sp| thine_common::IntakeSpan {
            trace_id: sp.trace_id,
            span_id: sp.span_id,
            parent_span_id: sp.parent_span_id,
            service: sp.service,
            name: sp.name,
            duration_ms: sp.duration_ms,
            timestamp_ms: sp.timestamp_ms,
            status: sp.status,
            resource: sp.resource,
            tags: Default::default(),
        })
        .collect();
    let n = s.ingest.ingest_native_spans(intake);
    Json(json!({ "accepted": n, "path": "native", "signal": "traces" }))
}
async fn apm_services(State(s): State<AppState>) -> impl IntoResponse {
    Json(s.platform.list_apm_services())
}
async fn usm_services(State(s): State<AppState>) -> impl IntoResponse {
    Json(s.platform.usm_services())
}

async fn ingest_logs(
    State(s): State<AppState>,
    Json(events): Json<Vec<LogEvent>>,
) -> impl IntoResponse {
    // Native logs path → same intake hub as OTLP /v1/logs.
    let intake: Vec<thine_common::IntakeLog> = events
        .into_iter()
        .map(|e| thine_common::IntakeLog {
            timestamp_ms: e.timestamp_ms,
            level: e.level,
            service: e.service,
            message: e.message,
            attrs: e.attrs,
        })
        .collect();
    let n = s.ingest.ingest_native_logs(intake);
    Json(json!({ "accepted": n, "path": "native", "signal": "logs" }))
}

#[derive(Deserialize)]
struct LogSearch {
    service: Option<String>,
    level: Option<String>,
    limit: Option<usize>,
    /// Datadog-style query: `service:api @http.status_code:500 -env:dev`
    q: Option<String>,
    /// Live Tail mode — samples under load
    #[serde(default)]
    live: Option<bool>,
}
async fn search_logs(State(s): State<AppState>, Query(q): Query<LogSearch>) -> impl IntoResponse {
    let live = q.live.unwrap_or(false);
    let (logs, sample_rate) = s.platform.search_logs_query(
        q.q.as_deref(),
        q.service.as_deref(),
        q.level.as_deref(),
        q.limit.unwrap_or(if live { 80 } else { 100 }),
        live,
    );
    Json(json!({
        "logs": logs,
        "sample_rate": sample_rate,
        "live": live,
        "query": q.q,
    }))
}
async fn list_audit(State(s): State<AppState>, Query(q): Query<LimitQuery>) -> impl IntoResponse {
    Json(s.platform.list_audit(q.limit.unwrap_or(100)))
}
async fn list_errors(State(s): State<AppState>) -> impl IntoResponse {
    Json(s.platform.list_errors())
}
async fn list_agents(State(s): State<AppState>) -> impl IntoResponse {
    Json(s.platform.list_agents())
}
async fn upsert_agent(State(s): State<AppState>, Json(a): Json<AgentInfo>) -> impl IntoResponse {
    (StatusCode::CREATED, Json(s.platform.upsert_agent(a)))
}
async fn delete_agent(State(s): State<AppState>, Path(id): Path<String>) -> impl IntoResponse {
    if s.platform.delete_agent(&id) {
        StatusCode::NO_CONTENT.into_response()
    } else {
        (
            StatusCode::NOT_FOUND,
            Json(json!({ "error": "agent not found" })),
        )
            .into_response()
    }
}
async fn list_marketplace(State(s): State<AppState>) -> impl IntoResponse {
    Json(s.platform.list_marketplace())
}
async fn install_app(State(s): State<AppState>, Path(id): Path<String>) -> impl IntoResponse {
    match s.platform.install_app(&id) {
        Some(a) => Json(a).into_response(),
        None => (
            StatusCode::NOT_FOUND,
            Json(json!({ "error": "app not found" })),
        )
            .into_response(),
    }
}
async fn uninstall_app(State(s): State<AppState>, Path(id): Path<String>) -> impl IntoResponse {
    match s.platform.uninstall_app(&id) {
        Some(a) => Json(a).into_response(),
        None => (
            StatusCode::NOT_FOUND,
            Json(json!({ "error": "app not found" })),
        )
            .into_response(),
    }
}
async fn list_pipelines(State(s): State<AppState>) -> impl IntoResponse {
    Json(s.platform.list_pipelines())
}
async fn upsert_pipeline(State(s): State<AppState>, Json(p): Json<Pipeline>) -> impl IntoResponse {
    (StatusCode::CREATED, Json(s.platform.upsert_pipeline(p)))
}
async fn run_pipeline(
    State(s): State<AppState>,
    Path(id): Path<String>,
    Json(events): Json<Vec<LogEvent>>,
) -> impl IntoResponse {
    Json(s.platform.run_pipeline(&id, events))
}
async fn list_profiles(State(s): State<AppState>) -> impl IntoResponse {
    Json(s.platform.list_profiles())
}

#[derive(Deserialize)]
struct UploadProfile {
    #[serde(flatten)]
    meta: ProfileMeta,
    blob_b64: Option<String>,
}
async fn upload_profile(
    State(s): State<AppState>,
    Json(body): Json<UploadProfile>,
) -> impl IntoResponse {
    (
        StatusCode::CREATED,
        Json(s.platform.upload_profile(body.meta, body.blob_b64)),
    )
}
async fn profile_blob(State(s): State<AppState>, Path(id): Path<String>) -> impl IntoResponse {
    match s.platform.get_profile_blob(&id) {
        Some(b) => Json(json!({ "id": id, "blob_b64": b })).into_response(),
        None => (
            StatusCode::NOT_FOUND,
            Json(json!({ "error": "blob not found" })),
        )
            .into_response(),
    }
}
async fn list_streams(State(s): State<AppState>) -> impl IntoResponse {
    Json(s.platform.list_streams())
}
async fn list_db(State(s): State<AppState>) -> impl IntoResponse {
    Json(s.platform.list_db_instances())
}
async fn top_queries(State(s): State<AppState>, Query(q): Query<LimitQuery>) -> impl IntoResponse {
    Json(s.platform.top_queries(q.limit.unwrap_or(20)))
}
async fn list_data_assets(State(s): State<AppState>) -> impl IntoResponse {
    Json(s.platform.list_data_assets())
}
async fn list_sds(State(s): State<AppState>) -> impl IntoResponse {
    Json(s.platform.list_sds_rules())
}
async fn upsert_sds(State(s): State<AppState>, Json(r): Json<SdsRule>) -> impl IntoResponse {
    (StatusCode::CREATED, Json(s.platform.upsert_sds_rule(r)))
}
async fn sds_scan(State(s): State<AppState>) -> impl IntoResponse {
    Json(s.platform.scan_logs_for_sensitive())
}
async fn list_fleet(State(s): State<AppState>) -> impl IntoResponse {
    Json(s.platform.list_fleet())
}

#[derive(Deserialize)]
struct HeartbeatReq {
    id: String,
    version: String,
    host: String,
    #[serde(default)]
    platform: Option<String>,
}
async fn fleet_heartbeat(
    State(s): State<AppState>,
    Json(req): Json<HeartbeatReq>,
) -> impl IntoResponse {
    Json(s.platform.fleet_heartbeat(
        &req.id,
        &req.version,
        &req.host,
        req.platform.as_deref(),
    ))
}

#[derive(Deserialize)]
struct ConfigureReq {
    #[serde(default = "default_profile")]
    profile: String,
    #[serde(default)]
    agent_ids: Option<Vec<String>>,
}
fn default_profile() -> String {
    "standard".into()
}
async fn fleet_configure(
    State(s): State<AppState>,
    Json(req): Json<ConfigureReq>,
) -> impl IntoResponse {
    Json(
        s.platform
            .fleet_configure(&req.profile, req.agent_ids.as_deref()),
    )
}

async fn fleet_summary(State(s): State<AppState>) -> impl IntoResponse {
    Json(s.platform.fleet_summary())
}

#[derive(Deserialize)]
struct RolloutReq {
    target_version: String,
}
async fn fleet_rollout(
    State(s): State<AppState>,
    Json(req): Json<RolloutReq>,
) -> impl IntoResponse {
    Json(s.platform.fleet_rollout(&req.target_version))
}
async fn list_policies(State(s): State<AppState>) -> impl IntoResponse {
    Json(s.platform.list_policies())
}
async fn dora(State(s): State<AppState>) -> impl IntoResponse {
    Json(s.platform.dora())
}
async fn ux_overview(State(s): State<AppState>) -> impl IntoResponse {
    Json(s.platform.ux_overview())
}
async fn ux_sessions(State(s): State<AppState>) -> impl IntoResponse {
    Json(s.platform.ux_sessions())
}
async fn ux_vitals(State(s): State<AppState>) -> impl IntoResponse {
    Json(s.platform.ux_vitals())
}
async fn ux_synthetics(State(s): State<AppState>) -> impl IntoResponse {
    Json(s.platform.ux_synthetics())
}
async fn ux_timeseries(State(s): State<AppState>) -> impl IntoResponse {
    Json(s.platform.ux_timeseries())
}
async fn ux_pages(State(s): State<AppState>) -> impl IntoResponse {
    Json(s.platform.ux_pages())
}
async fn ux_funnel(State(s): State<AppState>) -> impl IntoResponse {
    Json(s.platform.ux_funnel())
}

#[derive(Deserialize)]
struct ChatReq {
    message: String,
}
async fn bits_chat(State(s): State<AppState>, Json(req): Json<ChatReq>) -> impl IntoResponse {
    Json(s.platform.bits_chat(&req.message))
}
async fn mcp_tools(State(s): State<AppState>) -> impl IntoResponse {
    Json(s.platform.mcp_tools())
}

#[derive(Deserialize)]
struct McpInvoke {
    tool: String,
    #[serde(default)]
    args: serde_json::Value,
}
async fn mcp_invoke(State(s): State<AppState>, Json(req): Json<McpInvoke>) -> impl IntoResponse {
    Json(s.platform.mcp_invoke(&req.tool, &req.args))
}
async fn cli_info(State(s): State<AppState>) -> impl IntoResponse {
    Json(s.platform.cli_info())
}

async fn list_probes(State(s): State<AppState>) -> impl IntoResponse {
    Json(s.platform.list_probes())
}
async fn upsert_probe(State(s): State<AppState>, Json(p): Json<DynProbe>) -> impl IntoResponse {
    (StatusCode::CREATED, Json(s.platform.upsert_probe(p)))
}
async fn delete_probe(State(s): State<AppState>, Path(id): Path<String>) -> impl IntoResponse {
    if s.platform.delete_probe(&id) {
        StatusCode::NO_CONTENT.into_response()
    } else {
        (
            StatusCode::NOT_FOUND,
            Json(json!({ "error": "probe not found" })),
        )
            .into_response()
    }
}

async fn list_byoc(State(s): State<AppState>) -> impl IntoResponse {
    Json(s.platform.list_byoc())
}
async fn upsert_byoc(State(s): State<AppState>, Json(b): Json<ByocSink>) -> impl IntoResponse {
    (StatusCode::CREATED, Json(s.platform.upsert_byoc(b)))
}

#[derive(Deserialize)]
struct ForwardQuery {
    limit: Option<usize>,
}
async fn forward_byoc(
    State(s): State<AppState>,
    Path(id): Path<String>,
    Query(q): Query<ForwardQuery>,
) -> impl IntoResponse {
    Json(s.platform.forward_logs_byoc(&id, q.limit.unwrap_or(100)))
}

async fn mobile_config(State(s): State<AppState>) -> impl IntoResponse {
    Json(s.platform.mobile_config())
}
async fn set_mobile(State(s): State<AppState>, Json(c): Json<MobileConfig>) -> impl IntoResponse {
    Json(s.platform.set_mobile_config(c))
}
async fn list_ide(State(s): State<AppState>) -> impl IntoResponse {
    Json(s.platform.list_ide_plugins())
}

async fn escalate_incident(
    State(s): State<AppState>,
    Path(id): Path<uuid::Uuid>,
    Json(body): Json<serde_json::Value>,
) -> impl IntoResponse {
    let note = body
        .get("note")
        .and_then(|v| v.as_str())
        .unwrap_or("escalated");
    match s.platform.escalate_incident(id, note) {
        Some(v) => Json(v).into_response(),
        None => (
            StatusCode::NOT_FOUND,
            Json(json!({ "error": "incident not found" })),
        )
            .into_response(),
    }
}

async fn run_workflow(State(s): State<AppState>, Path(id): Path<uuid::Uuid>) -> impl IntoResponse {
    match s.platform.run_workflow(id) {
        Some(v) => Json(v).into_response(),
        None => (
            StatusCode::NOT_FOUND,
            Json(json!({ "error": "workflow not found" })),
        )
            .into_response(),
    }
}

async fn slo_budgets(State(s): State<AppState>) -> impl IntoResponse {
    Json(s.platform.slo_budgets())
}

async fn host_live(State(s): State<AppState>) -> impl IntoResponse {
    Json(s.platform.host_live())
}

async fn apm_stats(State(s): State<AppState>) -> impl IntoResponse {
    Json(s.platform.apm_service_stats())
}

async fn trace_tree(State(s): State<AppState>, Path(id): Path<String>) -> impl IntoResponse {
    match s.platform.trace_tree(&id) {
        Some(t) => Json(t).into_response(),
        None => (
            StatusCode::NOT_FOUND,
            Json(json!({ "error": "trace not found" })),
        )
            .into_response(),
    }
}

async fn service_map(State(s): State<AppState>) -> impl IntoResponse {
    Json(s.platform.service_map())
}

async fn log_facets(State(s): State<AppState>) -> impl IntoResponse {
    Json(s.platform.log_facets())
}

#[derive(Deserialize)]
struct AggQuery {
    group_by: Option<String>,
}
async fn log_aggregate(State(s): State<AppState>, Query(q): Query<AggQuery>) -> impl IntoResponse {
    Json(s.platform.log_aggregate(q.group_by.as_deref().unwrap_or("service")))
}

#[derive(Deserialize)]
struct FormulaQuery {
    expr: String,
}
async fn query_formula(State(s): State<AppState>, Query(q): Query<FormulaQuery>) -> impl IntoResponse {
    match s.platform.query_formula(&q.expr) {
        Ok(v) => Json(v).into_response(),
        Err(e) => (StatusCode::BAD_REQUEST, Json(json!({ "error": e }))).into_response(),
    }
}
async fn query_formula_post(
    State(s): State<AppState>,
    Json(q): Json<FormulaQuery>,
) -> impl IntoResponse {
    match s.platform.query_formula(&q.expr) {
        Ok(v) => Json(v).into_response(),
        Err(e) => (StatusCode::BAD_REQUEST, Json(json!({ "error": e }))).into_response(),
    }
}

async fn list_notif(State(s): State<AppState>) -> impl IntoResponse {
    Json(s.platform.list_notif_channels())
}
async fn upsert_notif(State(s): State<AppState>, Json(c): Json<NotifChannel>) -> impl IntoResponse {
    (StatusCode::CREATED, Json(s.platform.upsert_notif_channel(c)))
}
async fn list_deliveries(
    State(s): State<AppState>,
    Query(q): Query<LimitQuery>,
) -> impl IntoResponse {
    Json(s.platform.list_notif_deliveries(q.limit.unwrap_or(50)))
}
async fn sync_notify(State(s): State<AppState>) -> impl IntoResponse {
    Json(s.platform.sync_alert_notifications())
}

async fn compare_datadog(State(s): State<AppState>) -> impl IntoResponse {
    Json(s.platform.datadog_comparison())
}

async fn compare_competitors(State(s): State<AppState>) -> impl IntoResponse {
    Json(s.platform.competitors_comparison())
}

async fn usm_ebpf(State(s): State<AppState>) -> impl IntoResponse {
    Json(s.platform.usm_ebpf_status())
}
async fn usm_discover(State(s): State<AppState>) -> impl IntoResponse {
    Json(s.platform.usm_discover())
}
async fn usm_map(State(s): State<AppState>) -> impl IntoResponse {
    Json(s.platform.usm_full_map())
}

async fn watchdog_scan(State(s): State<AppState>) -> impl IntoResponse {
    Json(s.platform.watchdog_scan())
}
async fn watchdog_anomalies(
    State(s): State<AppState>,
    Query(q): Query<LimitQuery>,
) -> impl IntoResponse {
    Json(s.platform.list_watchdog_anomalies(q.limit.unwrap_or(50)))
}
async fn watchdog_summary(State(s): State<AppState>) -> impl IntoResponse {
    Json(s.platform.watchdog_summary())
}

async fn ha_status(State(s): State<AppState>) -> impl IntoResponse {
    Json(s.platform.ha_status())
}
async fn ha_regions(State(s): State<AppState>) -> impl IntoResponse {
    Json(s.platform.list_regions())
}

#[derive(Deserialize)]
struct FailoverReq {
    to_region: String,
}
async fn ha_failover(
    State(s): State<AppState>,
    Json(req): Json<FailoverReq>,
) -> impl IntoResponse {
    match s.platform.ha_failover(&req.to_region) {
        Ok(v) => Json(v).into_response(),
        Err(e) => (StatusCode::BAD_REQUEST, Json(json!({ "error": e }))).into_response(),
    }
}
async fn ha_replicate(State(s): State<AppState>) -> impl IntoResponse {
    Json(s.platform.ha_replicate_tick())
}

#[derive(Deserialize)]
struct IntegSearch {
    q: Option<String>,
    category: Option<String>,
    enabled: Option<bool>,
    limit: Option<usize>,
    offset: Option<usize>,
}
async fn integrations_search(
    State(s): State<AppState>,
    Query(q): Query<IntegSearch>,
) -> impl IntoResponse {
    Json(s.platform.search_integrations(
        q.q.as_deref(),
        q.category.as_deref(),
        q.enabled.unwrap_or(false),
        q.limit.unwrap_or(50),
        q.offset.unwrap_or(0),
    ))
}
async fn integration_health(
    State(s): State<AppState>,
    Path(id): Path<String>,
) -> impl IntoResponse {
    match s.platform.integration_health(&id) {
        Some(v) => Json(v).into_response(),
        None => (
            StatusCode::NOT_FOUND,
            Json(json!({ "error": "integration not found" })),
        )
            .into_response(),
    }
}

#[derive(Deserialize)]
struct CloudcraftQuery {
    provider: Option<String>,
    overlay: Option<String>,
    group_by: Option<String>,
    q: Option<String>,
}
async fn cloudcraft_diagram(
    State(s): State<AppState>,
    Query(q): Query<CloudcraftQuery>,
) -> impl IntoResponse {
    let groups: Vec<String> = q
        .group_by
        .as_deref()
        .unwrap_or("region,vpc,service")
        .split(',')
        .map(|x| x.trim().to_string())
        .filter(|x| !x.is_empty())
        .collect();
    Json(s.platform.cloudcraft_diagram(
        q.provider.as_deref(),
        q.overlay.as_deref(),
        &groups,
        q.q.as_deref(),
    ))
}
async fn cloudcraft_resources(State(s): State<AppState>) -> impl IntoResponse {
    Json(s.platform.list_cloud_resources())
}
async fn cloudcraft_views(State(s): State<AppState>) -> impl IntoResponse {
    Json(s.platform.list_cloudcraft_views())
}
async fn save_cloudcraft_view(
    State(s): State<AppState>,
    Json(v): Json<CloudcraftView>,
) -> impl IntoResponse {
    (StatusCode::CREATED, Json(s.platform.save_cloudcraft_view(v)))
}

#[derive(Deserialize)]
struct ProcQuery {
    host: Option<String>,
    limit: Option<usize>,
}
async fn live_processes(State(s): State<AppState>, Query(q): Query<ProcQuery>) -> impl IntoResponse {
    Json(
        s.platform
            .list_live_processes(q.host.as_deref(), q.limit.unwrap_or(50)),
    )
}

#[derive(Deserialize)]
struct ProcExplorerQ {
    host: Option<String>,
    user: Option<String>,
    command: Option<String>,
    service: Option<String>,
    env: Option<String>,
    q: Option<String>,
    limit: Option<usize>,
}
async fn processes_explorer(
    State(s): State<AppState>,
    Query(q): Query<ProcExplorerQ>,
) -> impl IntoResponse {
    Json(s.platform.processes_explorer(
        q.host.as_deref(),
        q.user.as_deref(),
        q.command.as_deref(),
        q.service.as_deref(),
        q.env.as_deref(),
        q.q.as_deref(),
        q.limit.unwrap_or(200),
    ))
}

async fn serverless_overview(State(s): State<AppState>) -> impl IntoResponse {
    Json(s.platform.serverless_overview())
}

async fn usm_red(State(s): State<AppState>) -> impl IntoResponse {
    Json(s.platform.usm_red_metrics())
}
async fn dbm_explain(State(s): State<AppState>) -> impl IntoResponse {
    Json(s.platform.list_explain_plans())
}
#[derive(Deserialize)]
struct SchemaQuery {
    db: Option<String>,
}
async fn dbm_schema(State(s): State<AppState>, Query(q): Query<SchemaQuery>) -> impl IntoResponse {
    Json(s.platform.list_schemas(q.db.as_deref()))
}
async fn dbm_apm(State(s): State<AppState>) -> impl IntoResponse {
    Json(s.platform.dbm_apm_correlation())
}
async fn dbm_summary(State(s): State<AppState>) -> impl IntoResponse {
    Json(s.platform.dbm_summary())
}
async fn dbm_health(State(s): State<AppState>) -> impl IntoResponse {
    Json(s.platform.dbm_health())
}
async fn dbm_metrics_catalog(State(s): State<AppState>) -> impl IntoResponse {
    Json(s.platform.dbm_metric_catalog_json())
}
#[derive(Deserialize)]
struct DbmSampleQuery {
    db: Option<String>,
    limit: Option<usize>,
}
async fn dbm_samples(
    State(s): State<AppState>,
    Query(q): Query<DbmSampleQuery>,
) -> impl IntoResponse {
    Json(s.platform.list_db_samples(q.db.as_deref(), q.limit.unwrap_or(180)))
}
async fn dbm_query_metrics(
    State(s): State<AppState>,
    Query(q): Query<LimitQuery>,
) -> impl IntoResponse {
    Json(s.platform.list_db_query_metrics(q.limit.unwrap_or(50)))
}
async fn dbm_waits(State(s): State<AppState>) -> impl IntoResponse {
    Json(s.platform.list_db_wait_events())
}
async fn dbm_blocking(State(s): State<AppState>) -> impl IntoResponse {
    Json(s.platform.list_db_blocking())
}
async fn dbm_activity(
    State(s): State<AppState>,
    Query(q): Query<LimitQuery>,
) -> impl IntoResponse {
    Json(s.platform.list_db_activity(q.limit.unwrap_or(50)))
}
async fn dbm_emit_metrics(State(s): State<AppState>) -> impl IntoResponse {
    let n = s.platform.emit_db_metrics(None);
    Json(json!({ "points_emitted": n }))
}

#[derive(Deserialize)]
struct AiRunQuery {
    project: Option<String>,
    run_type: Option<String>,
    limit: Option<usize>,
}
#[derive(Deserialize)]
struct AiSampleQuery {
    project: Option<String>,
    limit: Option<usize>,
}
#[derive(Deserialize)]
struct AiDatasetQuery {
    dataset_id: Option<String>,
}
#[derive(Deserialize)]
struct AiExpQuery {
    experiment_id: Option<String>,
}
#[derive(Deserialize)]
struct AiRunExpBody {
    dataset_id: String,
    #[serde(default = "default_trials")]
    trials: u32,
}
fn default_trials() -> u32 {
    3
}

async fn ai_summary(State(s): State<AppState>) -> impl IntoResponse {
    Json(s.platform.ai_summary())
}
async fn ai_projects(State(s): State<AppState>) -> impl IntoResponse {
    Json(s.platform.list_ai_projects())
}
async fn ai_runs(State(s): State<AppState>, Query(q): Query<AiRunQuery>) -> impl IntoResponse {
    Json(s.platform.list_ai_runs(
        q.project.as_deref(),
        q.run_type.as_deref(),
        q.limit.unwrap_or(100),
    ))
}
async fn ai_create_run(
    State(s): State<AppState>,
    Json(body): Json<AiCreateRun>,
) -> impl IntoResponse {
    Json(s.platform.ingest_ai_run(body))
}
async fn ai_runs_batch(
    State(s): State<AppState>,
    Json(body): Json<Vec<AiCreateRun>>,
) -> impl IntoResponse {
    Json(s.platform.ingest_ai_runs_batch(body))
}
async fn ai_feedback(State(s): State<AppState>, Query(q): Query<LimitQuery>) -> impl IntoResponse {
    Json(s.platform.list_ai_feedback(q.limit.unwrap_or(100)))
}
async fn ai_datasets(State(s): State<AppState>) -> impl IntoResponse {
    Json(s.platform.list_ai_datasets())
}
async fn ai_examples(
    State(s): State<AppState>,
    Query(q): Query<AiDatasetQuery>,
) -> impl IntoResponse {
    Json(s.platform.list_ai_examples(q.dataset_id.as_deref()))
}
async fn ai_experiments(State(s): State<AppState>) -> impl IntoResponse {
    Json(s.platform.list_ai_experiments())
}
async fn ai_run_experiment(
    State(s): State<AppState>,
    Json(body): Json<AiRunExpBody>,
) -> impl IntoResponse {
    Json(s.platform.run_ai_experiment(&body.dataset_id, body.trials))
}
async fn ai_evals(State(s): State<AppState>, Query(q): Query<AiExpQuery>) -> impl IntoResponse {
    Json(s.platform.list_ai_eval_results(q.experiment_id.as_deref()))
}
async fn ai_graders(State(s): State<AppState>) -> impl IntoResponse {
    Json(s.platform.list_ai_graders())
}
async fn ai_samples(
    State(s): State<AppState>,
    Query(q): Query<AiSampleQuery>,
) -> impl IntoResponse {
    Json(
        s.platform
            .list_ai_samples(q.project.as_deref(), q.limit.unwrap_or(180)),
    )
}
async fn ai_metrics_catalog(State(s): State<AppState>) -> impl IntoResponse {
    Json(s.platform.ai_metric_catalog_json())
}
async fn ai_health(State(s): State<AppState>) -> impl IntoResponse {
    Json(s.platform.ai_health())
}
async fn ai_emit_metrics(State(s): State<AppState>) -> impl IntoResponse {
    let n = s.platform.emit_ai_metrics(None);
    Json(json!({ "points_emitted": n }))
}

async fn data_lineage(State(s): State<AppState>) -> impl IntoResponse {
    Json(s.platform.data_lineage_graph())
}
async fn cost_recs(State(s): State<AppState>) -> impl IntoResponse {
    Json(s.platform.cost_recommendations())
}
async fn profiler_flame(State(s): State<AppState>, Path(id): Path<String>) -> impl IntoResponse {
    match s.platform.profiler_flame_summary(&id) {
        Some(v) => Json(v).into_response(),
        None => (
            StatusCode::NOT_FOUND,
            Json(json!({ "error": "profile not found" })),
        )
            .into_response(),
    }
}
async fn pipeline_workers(State(s): State<AppState>) -> impl IntoResponse {
    Json(s.platform.pipeline_workers())
}

async fn list_log_patterns(
    State(s): State<AppState>,
    Query(q): Query<LimitQuery>,
) -> impl IntoResponse {
    Json(s.platform.log_patterns(q.limit.unwrap_or(40)))
}

async fn kube_container_map(State(s): State<AppState>) -> impl IntoResponse {
    Json(s.platform.kube_container_map())
}

async fn list_scorecards(State(s): State<AppState>) -> impl IntoResponse {
    Json(s.platform.list_catalog_scorecards())
}

async fn get_service_scorecard(
    State(s): State<AppState>,
    Path(name): Path<String>,
) -> impl IntoResponse {
    match s.platform.catalog_scorecard(&name) {
        Some(sc) => Json(sc).into_response(),
        None => (StatusCode::NOT_FOUND, Json(json!({ "error": "service not found" }))).into_response(),
    }
}

async fn get_service_definition(
    State(s): State<AppState>,
    Path(name): Path<String>,
) -> impl IntoResponse {
    match s.platform.get_service_definition_yaml(&name) {
        Some(yaml) => Json(json!({ "service": name, "yaml": yaml })).into_response(),
        None => (StatusCode::NOT_FOUND, Json(json!({ "error": "service not found" }))).into_response(),
    }
}

#[derive(Deserialize)]
struct PutDefinitionBody {
    yaml: String,
}

async fn put_service_definition(
    State(s): State<AppState>,
    Path(name): Path<String>,
    Json(body): Json<PutDefinitionBody>,
) -> impl IntoResponse {
    match s.platform.put_service_definition_yaml(&name, body.yaml) {
        Some(c) => Json(c).into_response(),
        None => (
            StatusCode::BAD_REQUEST,
            Json(json!({ "error": "invalid service definition" })),
        )
            .into_response(),
    }
}

async fn share_notebook(
    State(s): State<AppState>,
    Path(id): Path<uuid::Uuid>,
) -> impl IntoResponse {
    match s.platform.share_notebook(id) {
        Some(nb) => Json(nb).into_response(),
        None => (StatusCode::NOT_FOUND, Json(json!({ "error": "notebook not found" }))).into_response(),
    }
}

async fn get_shared_notebook(
    State(s): State<AppState>,
    Path(token): Path<String>,
) -> impl IntoResponse {
    match s.platform.get_notebook_by_share(&token) {
        Some(nb) => Json(nb).into_response(),
        None => (StatusCode::NOT_FOUND, Json(json!({ "error": "share not found" }))).into_response(),
    }
}

#[derive(Deserialize)]
struct LinkErrorBody {
    issue: String,
    trace_id: Option<String>,
}

async fn link_error_issue(
    State(s): State<AppState>,
    Path(id): Path<String>,
    Json(body): Json<LinkErrorBody>,
) -> impl IntoResponse {
    let id = urlencoding_decode(&id);
    match s.platform.link_error_issue(&id, body.issue, body.trace_id) {
        Some(e) => Json(e).into_response(),
        None => (StatusCode::NOT_FOUND, Json(json!({ "error": "error group not found" }))).into_response(),
    }
}

/// Datadog Live Tail — WebSocket stream of matching log lines.
async fn logs_tail_ws(
    ws: WebSocketUpgrade,
    State(s): State<AppState>,
    Query(q): Query<LogSearch>,
) -> impl IntoResponse {
    ws.on_upgrade(move |socket| run_logs_tail(socket, s, q))
}

async fn run_logs_tail(socket: WebSocket, s: AppState, q: LogSearch) {
    let (mut sink, mut stream) = socket.split();
    let _ = sink
        .send(Message::Text(
            json!({
                "type": "hello",
                "query": q.q,
                "live": true,
                "docs_ref": "https://docs.datadoghq.com/logs/explorer/live_tail/"
            })
            .to_string()
            .into(),
        ))
        .await;
    let mut last_ts: i64 = 0;
    let mut ticks: u32 = 0;
    loop {
        tokio::select! {
            msg = stream.next() => {
                match msg {
                    Some(Ok(Message::Close(_))) | None => break,
                    Some(Ok(Message::Ping(p))) => {
                        let _ = sink.send(Message::Pong(p)).await;
                    }
                    Some(Ok(Message::Text(t))) => {
                        // Client can send {"q":"..."} to update filter — ignore parse errors.
                        if let Ok(v) = serde_json::from_str::<serde_json::Value>(&t) {
                            if v.get("type").and_then(|x| x.as_str()) == Some("ping") {
                                let _ = sink.send(Message::Text(r#"{"type":"pong"}"#.into())).await;
                            }
                        }
                    }
                    _ => {}
                }
            }
            _ = tokio::time::sleep(std::time::Duration::from_millis(750)) => {
                ticks += 1;
                let (logs, sample_rate) = s.platform.search_logs_query(
                    q.q.as_deref(),
                    q.service.as_deref(),
                    q.level.as_deref(),
                    q.limit.unwrap_or(40),
                    true,
                );
                let mut fresh: Vec<_> = logs.into_iter().filter(|l| l.timestamp_ms > last_ts).collect();
                if let Some(max_ts) = fresh.iter().map(|l| l.timestamp_ms).max() {
                    last_ts = max_ts;
                }
                // Keep payload bounded
                if fresh.len() > 30 {
                    fresh.truncate(30);
                }
                if !fresh.is_empty() || ticks % 8 == 0 {
                    let payload = json!({
                        "type": "batch",
                        "sample_rate": sample_rate,
                        "logs": fresh,
                    });
                    if sink.send(Message::Text(payload.to_string().into())).await.is_err() {
                        break;
                    }
                }
                if ticks > 800 {
                    // ~10 minutes max session for demo safety
                    let _ = sink.send(Message::Close(None)).await;
                    break;
                }
            }
        }
    }
}
