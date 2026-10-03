//! Routes for Datadog-parity platform modules.

use crate::state::AppState;
use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::routing::{get, post};
use axum::{Json, Router};
use serde::Deserialize;
use serde_json::json;
use thine_platform::{
    all_features, feature_stats, CreateIncident, CreateNotebook, CreateSlo, CreateTeam,
    CreateWorkflow, LogEvent, SpanRecord,
};

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/api/v1/features", get(features))
        .route("/api/v1/features/stats", get(features_stats))
        .route("/api/v1/notebooks", get(list_notebooks).post(create_notebook))
        .route("/api/v1/teams", get(list_teams).post(create_team))
        .route("/api/v1/rbac/users", get(list_users))
        .route("/api/v1/rbac/roles", get(list_roles))
        .route("/api/v1/incidents", get(list_incidents).post(create_incident))
        .route("/api/v1/work", get(list_work))
        .route("/api/v1/workflows", get(list_workflows).post(create_workflow))
        .route("/api/v1/workflows/runs", get(list_workflow_runs))
        .route("/api/v1/slos", get(list_slos).post(create_slo))
        .route("/api/v1/catalog/services", get(list_catalog))
        .route("/api/v1/integrations", get(list_integrations))
        .route("/api/v1/integrations/{id}", axum::routing::patch(patch_integration))
        .route("/api/v1/infra/hosts", get(list_hosts))
        .route("/api/v1/containers", get(list_containers))
        .route("/api/v1/gpu/devices", get(list_gpus))
        .route("/api/v1/storage/volumes", get(list_volumes))
        .route("/api/v1/serverless/functions", get(list_functions))
        .route("/api/v1/cost/summary", get(cost_summary))
        .route("/api/v1/network/flows", get(network_flows))
        .route("/api/v1/k8s/autoscalers", get(autoscalers))
        .route("/api/v1/apm/traces", get(list_traces).post(ingest_traces))
        .route("/api/v1/apm/services", get(apm_services))
        .route("/api/v1/usm/services", get(usm_services))
        .route("/api/v1/logs/ingest", post(ingest_logs))
        .route("/api/v1/logs/search", get(search_logs))
        .route("/api/v1/audit", get(list_audit))
        .route("/api/v1/errors", get(list_errors))
        .route("/api/v1/agents", get(list_agents))
        .route("/api/v1/marketplace", get(list_marketplace))
        .route("/api/v1/pipelines", get(list_pipelines))
        .route("/api/v1/profiler/profiles", get(list_profiles))
        .route("/api/v1/streams", get(list_streams))
        .route("/api/v1/dbm/instances", get(list_db))
        .route("/api/v1/data/assets", get(list_data_assets))
        .route("/api/v1/sds/rules", get(list_sds))
        .route("/api/v1/fleet/agents", get(list_fleet))
        .route("/api/v1/governance/policies", get(list_policies))
        .route("/api/v1/dora", get(dora))
        .route("/api/v1/bits/chat", post(bits_chat))
        .route("/api/v1/mcp/tools", get(mcp_tools))
        .route("/api/v1/cli/info", get(cli_info))
}

async fn features() -> impl IntoResponse {
    Json(all_features())
}
async fn features_stats() -> impl IntoResponse {
    Json(feature_stats())
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
async fn list_gpus(State(s): State<AppState>) -> impl IntoResponse {
    Json(s.platform.list_gpus())
}
async fn list_volumes(State(s): State<AppState>) -> impl IntoResponse {
    Json(s.platform.list_volumes())
}
async fn list_functions(State(s): State<AppState>) -> impl IntoResponse {
    Json(s.platform.list_functions())
}
async fn cost_summary(State(s): State<AppState>) -> impl IntoResponse {
    Json(s.platform.cost_summary())
}
async fn network_flows(State(s): State<AppState>) -> impl IntoResponse {
    Json(s.platform.network_flows())
}
async fn autoscalers(State(s): State<AppState>) -> impl IntoResponse {
    Json(s.platform.list_autoscalers())
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
    let n = s.platform.ingest_spans(spans);
    Json(json!({ "accepted": n }))
}
async fn apm_services(State(s): State<AppState>) -> impl IntoResponse {
    Json(s.platform.list_apm_services())
}
async fn usm_services(State(s): State<AppState>) -> impl IntoResponse {
    Json(s.platform.usm_services())
}

async fn ingest_logs(State(s): State<AppState>, Json(events): Json<Vec<LogEvent>>) -> impl IntoResponse {
    let n = events.len();
    for e in events {
        s.platform.ingest_log(e);
    }
    Json(json!({ "accepted": n }))
}

#[derive(Deserialize)]
struct LogSearch {
    service: Option<String>,
    level: Option<String>,
    limit: Option<usize>,
}
async fn search_logs(State(s): State<AppState>, Query(q): Query<LogSearch>) -> impl IntoResponse {
    Json(s.platform.search_logs(
        q.service.as_deref(),
        q.level.as_deref(),
        q.limit.unwrap_or(100),
    ))
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
async fn list_marketplace(State(s): State<AppState>) -> impl IntoResponse {
    Json(s.platform.list_marketplace())
}
async fn list_pipelines(State(s): State<AppState>) -> impl IntoResponse {
    Json(s.platform.list_pipelines())
}
async fn list_profiles(State(s): State<AppState>) -> impl IntoResponse {
    Json(s.platform.list_profiles())
}
async fn list_streams(State(s): State<AppState>) -> impl IntoResponse {
    Json(s.platform.list_streams())
}
async fn list_db(State(s): State<AppState>) -> impl IntoResponse {
    Json(s.platform.list_db_instances())
}
async fn list_data_assets(State(s): State<AppState>) -> impl IntoResponse {
    Json(s.platform.list_data_assets())
}
async fn list_sds(State(s): State<AppState>) -> impl IntoResponse {
    Json(s.platform.list_sds_rules())
}
async fn list_fleet(State(s): State<AppState>) -> impl IntoResponse {
    Json(s.platform.list_fleet())
}
async fn list_policies(State(s): State<AppState>) -> impl IntoResponse {
    Json(s.platform.list_policies())
}
async fn dora(State(s): State<AppState>) -> impl IntoResponse {
    Json(s.platform.dora())
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
async fn cli_info(State(s): State<AppState>) -> impl IntoResponse {
    Json(s.platform.cli_info())
}
