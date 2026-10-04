use crate::auth::resolve_org;
use crate::state::AppState;
use axum::extract::{Path, Query, State};
use axum::http::{HeaderMap, StatusCode};
use axum::response::IntoResponse;
use axum::routing::{get, post};
use axum::{Json, Router};
use serde::Deserialize;
use serde_json::json;
use thine_common::{
    CreateAlertRequest, CreateBoardRequest, DashboardWidget, QueryRequest, UpdateBoardRequest,
};
use thine_storage::health;
use uuid::Uuid;

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/health", get(health_handler))
        .route("/api/v1/health", get(health_handler))
        .route("/api/v1/dashboard", get(dashboard))
        .route("/api/v1/metrics", get(list_metrics))
        .route("/api/v1/query", get(query_get).post(query_post))
        .route("/api/v1/series", post(ingest_simple))
        .route("/api/v1/ingest", post(ingest_simple))
        .route("/api/v1/ingest/statsd", post(ingest_statsd))
        .route("/api/v1/stats", get(stats))
        .route("/api/v1/alerts", get(list_alerts).post(create_alert))
        .route("/api/v1/alerts/{id}", axum::routing::delete(delete_alert))
        .route("/api/v1/alerts/events", get(alert_events))
        // Static board paths before `{id}` so they are not captured as UUIDs.
        .route(
            "/api/v1/boards/lists",
            get(list_board_lists).post(create_board_list),
        )
        .route(
            "/api/v1/boards/clipboard",
            get(clipboard_get).post(clipboard_set),
        )
        .route("/api/v1/boards/deleted", get(list_deleted_boards))
        .route("/api/v1/boards", get(list_boards).post(create_board))
        .route(
            "/api/v1/boards/{id}",
            get(get_board)
                .put(update_board)
                .delete(delete_board),
        )
        .route("/api/v1/boards/{id}/render", get(render_board))
        .route("/api/v1/boards/{id}/share", post(share_board))
        .route("/api/v1/boards/{id}/restore", post(restore_board))
        .route("/api/v1/boards/{id}/anomalies", get(board_anomalies))
        .route("/api/v1/dashboards/guide", get(dashboards_guide))
        .route(
            "/api/v1/graph_insights/correlations",
            post(graph_correlations),
        )
        .route("/api/v1/graph_insights/explain", post(graph_explain))
        .route("/api/v1/graph_insights/guide", get(graph_insights_guide))
        .route("/api/v1/metrics/summary", get(metrics_summary))
        // Dual-path intake architecture (native + OTel converge → fan-out)
        .route("/api/v1/ingest/architecture", get(ingest_architecture))
        .route("/api/v1/events", get(list_events).post(ingest_events))
        // OpenTelemetry OTLP/HTTP (JSON) — metrics / traces / logs
        .route("/v1/metrics", post(otlp_metrics))
        .route("/otlp/v1/metrics", post(otlp_metrics))
        .route("/v1/traces", post(otlp_traces))
        .route("/otlp/v1/traces", post(otlp_traces))
        .route("/v1/logs", post(otlp_logs))
        .route("/otlp/v1/logs", post(otlp_logs))
}

async fn health_handler() -> impl IntoResponse {
    Json(health())
}

async fn dashboard(State(state): State<AppState>) -> impl IntoResponse {
    Json(state.store.dashboard())
}

#[derive(Debug, Deserialize)]
struct ListQuery {
    prefix: Option<String>,
}

fn with_org_tags(ctx: &crate::auth::OrgCtx, mut tags: thine_common::Tags) -> thine_common::Tags {
    tags.insert("org_id".into(), ctx.org_id.clone());
    tags
}

async fn list_metrics(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(q): Query<ListQuery>,
) -> impl IntoResponse {
    let ctx = resolve_org(&state, &headers);
    let all = state.store.list_metrics(q.prefix.as_deref());
    let filtered: Vec<_> = all
        .into_iter()
        .filter(|m| m.tags.get("org_id").map(String::as_str) == Some(ctx.org_id.as_str()))
        .collect();
    Json(filtered)
}

async fn query_post(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(mut req): Json<QueryRequest>,
) -> impl IntoResponse {
    let ctx = resolve_org(&state, &headers);
    req.tags = with_org_tags(&ctx, req.tags);
    match state.store.query(req) {
        Ok(r) => (StatusCode::OK, Json(json!({ "results": r }))).into_response(),
        Err(e) => (
            StatusCode::BAD_REQUEST,
            Json(json!({ "error": e.to_string() })),
        )
            .into_response(),
    }
}

#[derive(Debug, Deserialize)]
struct QueryParams {
    metric: String,
    start_ms: Option<i64>,
    end_ms: Option<i64>,
    step_ms: Option<i64>,
    aggregation: Option<String>,
    /// Comma-separated tag filters: `service:api,env:prod`
    #[serde(default)]
    tags: Option<String>,
    #[serde(default)]
    service: Option<String>,
    #[serde(default)]
    env: Option<String>,
    /// When set, only return series that have this tag key (for split/group).
    #[serde(default)]
    group_by: Option<String>,
}

fn parse_tag_filters(raw: Option<&str>) -> thine_common::Tags {
    let mut tags = thine_common::Tags::new();
    if let Some(s) = raw {
        for part in s.split(',') {
            let part = part.trim();
            if part.is_empty() || part == "*" {
                continue;
            }
            if let Some((k, v)) = part.split_once(':') {
                tags.insert(k.trim().to_string(), v.trim().to_string());
            }
        }
    }
    tags
}

async fn query_get(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(q): Query<QueryParams>,
) -> impl IntoResponse {
    let ctx = resolve_org(&state, &headers);
    let mut tags = with_org_tags(&ctx, parse_tag_filters(q.tags.as_deref()));
    if let Some(s) = q.service {
        tags.insert("service".into(), s);
    }
    if let Some(e) = q.env {
        tags.insert("env".into(), e);
    }
    let aggregation = match q.aggregation.as_deref() {
        Some("sum") => thine_common::Aggregation::Sum,
        Some("min") => thine_common::Aggregation::Min,
        Some("max") => thine_common::Aggregation::Max,
        Some("count") => thine_common::Aggregation::Count,
        Some("last") => thine_common::Aggregation::Last,
        _ => thine_common::Aggregation::Avg,
    };
    let req = QueryRequest {
        metric: q.metric,
        tags,
        start_ms: q.start_ms,
        end_ms: q.end_ms,
        step_ms: q.step_ms.unwrap_or(15_000),
        aggregation,
    };
    match state.store.query(req) {
        Ok(mut r) => {
            if let Some(key) = q.group_by.as_deref().filter(|k| !k.is_empty()) {
                r.retain(|s| s.tags.contains_key(key) && !s.points.is_empty());
            } else {
                let nonempty: Vec<_> = r.iter().filter(|s| !s.points.is_empty()).cloned().collect();
                if !nonempty.is_empty() {
                    r = nonempty;
                }
            }
            (StatusCode::OK, Json(json!({ "results": r }))).into_response()
        }
        Err(e) => (
            StatusCode::BAD_REQUEST,
            Json(json!({ "error": e.to_string() })),
        )
            .into_response(),
    }
}

async fn ingest_simple(
    State(state): State<AppState>,
    headers: HeaderMap,
    body: axum::body::Bytes,
) -> impl IntoResponse {
    let ctx = resolve_org(&state, &headers);
    match state
        .ingest
        .ingest_simple_json_org(&body, Some(&ctx.org_id))
    {
        Ok(stats) => (StatusCode::OK, Json(stats)).into_response(),
        Err(e) => (
            StatusCode::BAD_REQUEST,
            Json(json!({ "error": e.to_string() })),
        )
            .into_response(),
    }
}

async fn ingest_statsd(
    State(state): State<AppState>,
    headers: HeaderMap,
    body: String,
) -> impl IntoResponse {
    let ctx = resolve_org(&state, &headers);
    Json(
        state
            .ingest
            .ingest_statsd_lines_org(&body, Some(&ctx.org_id)),
    )
}

fn reject_protobuf(headers: &axum::http::HeaderMap) -> Option<axum::response::Response> {
    let ct = headers
        .get(axum::http::header::CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");
    if ct.contains("protobuf") || ct.contains("application/x-protobuf") {
        return Some(
            (
                StatusCode::UNSUPPORTED_MEDIA_TYPE,
                Json(json!({
                    "error": "OTLP protobuf framing accepted at edge — decode via OTLP/HTTP JSON on this build. Set Content-Type: application/json (OTel HTTP JSON exporter) or terminate protobuf at an OTel Collector → Thine JSON exporter.",
                    "supported": ["application/json", "application/json; charset=utf-8"],
                    "paths": ["/v1/metrics", "/v1/traces", "/v1/logs"]
                })),
            )
                .into_response(),
        );
    }
    None
}

async fn otlp_metrics(
    State(state): State<AppState>,
    headers: axum::http::HeaderMap,
    body: axum::body::Bytes,
) -> impl IntoResponse {
    if let Some(r) = reject_protobuf(&headers) {
        return r;
    }
    let ctx = resolve_org(&state, &headers);
    match state
        .ingest
        .ingest_otlp_json_org(&body, Some(&ctx.org_id))
    {
        Ok(stats) => (
            StatusCode::OK,
            Json(json!({
                "partialSuccess": {
                    "rejectedDataPoints": stats.rejected,
                    "errorMessage": ""
                },
                "thine": stats
            })),
        )
            .into_response(),
        Err(e) => (
            StatusCode::BAD_REQUEST,
            Json(json!({ "error": e.to_string() })),
        )
            .into_response(),
    }
}

async fn otlp_traces(
    State(state): State<AppState>,
    headers: axum::http::HeaderMap,
    body: axum::body::Bytes,
) -> impl IntoResponse {
    if let Some(r) = reject_protobuf(&headers) {
        return r;
    }
    let ctx = resolve_org(&state, &headers);
    match state
        .ingest
        .ingest_otlp_traces_json_org(&body, Some(&ctx.org_id))
    {
        Ok(v) => (StatusCode::OK, Json(v)).into_response(),
        Err(e) => (
            StatusCode::BAD_REQUEST,
            Json(json!({ "error": e.to_string() })),
        )
            .into_response(),
    }
}

async fn otlp_logs(
    State(state): State<AppState>,
    headers: axum::http::HeaderMap,
    body: axum::body::Bytes,
) -> impl IntoResponse {
    if let Some(r) = reject_protobuf(&headers) {
        return r;
    }
    match state.ingest.ingest_otlp_logs_json(&body) {
        Ok(v) => (StatusCode::OK, Json(v)).into_response(),
        Err(e) => (
            StatusCode::BAD_REQUEST,
            Json(json!({ "error": e.to_string() })),
        )
            .into_response(),
    }
}

async fn ingest_architecture(State(state): State<AppState>) -> impl IntoResponse {
    state.refresh_durable_stats();
    Json(state.ingest.architecture())
}

async fn list_events(
    State(state): State<AppState>,
    Query(q): Query<ListEventsQuery>,
) -> impl IntoResponse {
    Json(state.platform.list_events(q.limit.unwrap_or(50)))
}

#[derive(Debug, Deserialize)]
struct ListEventsQuery {
    limit: Option<usize>,
}

async fn ingest_events(
    State(state): State<AppState>,
    Json(events): Json<Vec<thine_common::IntakeEvent>>,
) -> impl IntoResponse {
    let n = state.ingest.ingest_native_events(events);
    Json(json!({ "accepted": n, "path": "native", "signal": "events" }))
}

async fn stats(State(state): State<AppState>) -> impl IntoResponse {
    Json(state.store.stats())
}

async fn list_alerts(State(state): State<AppState>) -> impl IntoResponse {
    Json(state.store.list_alerts())
}

async fn create_alert(
    State(state): State<AppState>,
    Json(req): Json<CreateAlertRequest>,
) -> impl IntoResponse {
    let rule = state.store.create_alert(req);
    (StatusCode::CREATED, Json(rule))
}

async fn delete_alert(State(state): State<AppState>, Path(id): Path<Uuid>) -> impl IntoResponse {
    if state.store.delete_alert(id) {
        StatusCode::NO_CONTENT.into_response()
    } else {
        (
            StatusCode::NOT_FOUND,
            Json(json!({ "error": "alert not found" })),
        )
            .into_response()
    }
}

#[derive(Debug, Deserialize)]
struct EventsQuery {
    limit: Option<usize>,
}

async fn alert_events(
    State(state): State<AppState>,
    Query(q): Query<EventsQuery>,
) -> impl IntoResponse {
    Json(state.store.list_alert_events(q.limit.unwrap_or(50)))
}

async fn list_boards(State(state): State<AppState>) -> impl IntoResponse {
    Json(state.store.list_boards())
}

async fn get_board(State(state): State<AppState>, Path(id): Path<Uuid>) -> impl IntoResponse {
    match state.store.get_board(id) {
        Some(b) => Json(b).into_response(),
        None => (
            StatusCode::NOT_FOUND,
            Json(json!({ "error": "board not found" })),
        )
            .into_response(),
    }
}

async fn create_board(
    State(state): State<AppState>,
    Json(req): Json<CreateBoardRequest>,
) -> impl IntoResponse {
    (StatusCode::CREATED, Json(state.store.create_board(req)))
}

async fn update_board(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    Json(req): Json<UpdateBoardRequest>,
) -> impl IntoResponse {
    match state.store.update_board(id, req) {
        Some(b) => Json(b).into_response(),
        None => (
            StatusCode::NOT_FOUND,
            Json(json!({ "error": "board not found" })),
        )
            .into_response(),
    }
}

async fn delete_board(State(state): State<AppState>, Path(id): Path<Uuid>) -> impl IntoResponse {
    if state.store.delete_board(id) {
        StatusCode::NO_CONTENT.into_response()
    } else {
        (
            StatusCode::NOT_FOUND,
            Json(json!({ "error": "board not found" })),
        )
            .into_response()
    }
}

#[derive(Debug, Deserialize)]
struct ShareBody {
    public: bool,
}

async fn share_board(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    Json(body): Json<ShareBody>,
) -> impl IntoResponse {
    match state.store.share_board(id, body.public) {
        Some(b) => Json(b).into_response(),
        None => (
            StatusCode::NOT_FOUND,
            Json(json!({ "error": "board not found" })),
        )
            .into_response(),
    }
}

#[derive(Debug, Deserialize)]
struct CreateListBody {
    name: String,
    #[serde(default)]
    board_ids: Vec<Uuid>,
}

async fn list_board_lists(State(state): State<AppState>) -> impl IntoResponse {
    Json(state.store.list_board_lists())
}

async fn create_board_list(
    State(state): State<AppState>,
    Json(body): Json<CreateListBody>,
) -> impl IntoResponse {
    (
        StatusCode::CREATED,
        Json(state.store.create_board_list(body.name, body.board_ids)),
    )
}

async fn clipboard_get(State(state): State<AppState>) -> impl IntoResponse {
    Json(state.store.clipboard_get())
}

async fn clipboard_set(
    State(state): State<AppState>,
    Json(widgets): Json<Vec<DashboardWidget>>,
) -> impl IntoResponse {
    state.store.clipboard_set(widgets);
    Json(json!({ "ok": true, "count": state.store.clipboard_get().len() }))
}

async fn list_deleted_boards(State(state): State<AppState>) -> impl IntoResponse {
    Json(state.store.list_deleted_boards())
}

#[derive(Debug, Deserialize)]
struct RestoreBody {
    /// Optional dashboard list id to restore into ("Restore to").
    #[serde(default)]
    list_id: Option<String>,
}

async fn restore_board(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    Json(body): Json<RestoreBody>,
) -> impl IntoResponse {
    match state
        .store
        .restore_board(id, body.list_id.as_deref())
    {
        Some(b) => Json(b).into_response(),
        None => (
            StatusCode::NOT_FOUND,
            Json(json!({ "error": "board not found" })),
        )
            .into_response(),
    }
}

#[derive(Debug, Deserialize)]
struct AnomaliesQuery {
    range_ms: Option<i64>,
    /// Auto-detect issues (browser preference); default true.
    auto_detect: Option<bool>,
}

async fn board_anomalies(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    Query(q): Query<AnomaliesQuery>,
) -> impl IntoResponse {
    let range_ms = q.range_ms.unwrap_or(3_600_000);
    let auto = q.auto_detect.unwrap_or(true);
    match state.platform.board_anomalies(id, range_ms, auto) {
        Some(r) => Json(r).into_response(),
        None => (
            StatusCode::NOT_FOUND,
            Json(json!({ "error": "board not found" })),
        )
            .into_response(),
    }
}

async fn graph_correlations(
    State(state): State<AppState>,
    Json(req): Json<thine_platform::CorrelationSearchRequest>,
) -> impl IntoResponse {
    Json(state.platform.graph_correlations(req))
}

async fn graph_explain(
    State(state): State<AppState>,
    Json(req): Json<thine_platform::WatchdogExplainRequest>,
) -> impl IntoResponse {
    Json(state.platform.graph_watchdog_explain(req))
}

async fn graph_insights_guide(State(state): State<AppState>) -> impl IntoResponse {
    Json(state.platform.graph_insights_guide())
}

async fn dashboards_guide() -> impl IntoResponse {
    Json(json!({
        "title": "Dashboards",
        "docs": "https://docs.datadoghq.com/dashboards/",
        "overview": "Real-time insights into system health, KPIs, trends, and anomalies.",
        "layouts": [
            {"id": "dashboard", "label": "Dashboard", "desc": "12-column grid for status boards and storytelling"},
            {"id": "timeboard", "label": "Timeboard", "desc": "Shared time cursor across widgets for troubleshooting"},
            {"id": "screenboard", "label": "Screenboard", "desc": "Free-form layout for NOC / executive views"}
        ],
        "features": [
            {"id": "configure", "path": "/api/v1/boards/{id}", "desc": "General configuration options"},
            {"id": "dashboard_list", "path": "/api/v1/boards/lists", "desc": "Search, view, or create dashboard lists"},
            {"id": "template_variables", "path": "/api/v1/boards/{id}/render?vars=", "desc": "Dynamically filter widgets"},
            {"id": "clipboard", "path": "/api/v1/boards/clipboard", "desc": "Copy and move dashboard widgets"},
            {"id": "api", "path": "/api/v1/boards", "desc": "Manage dashboards programmatically"},
            {"id": "widgets", "path": "/api/v1/boards/{id}/render", "desc": "Timeseries, query value, toplist, pie, table, heatmap, …"},
            {"id": "querying", "path": "/api/v1/query", "desc": "Metric query formatting"},
            {"id": "functions", "path": "widget.functions", "desc": "rate, cumsum, abs, top, exclude_null, anomalies"},
            {"id": "overlays", "path": "board.annotations", "desc": "Event overlays and annotations on graphs"},
            {"id": "sharing", "path": "/api/v1/boards/{id}/share", "desc": "Public share tokens (30s refresh)"},
            {"id": "graph_insights", "path": "/api/v1/graph_insights/guide", "desc": "Metric Correlations, Watchdog Explains, dashboard anomalies"},
            {"id": "recently_deleted", "path": "/api/v1/boards/deleted", "desc": "Soft-deleted boards recoverable for 30 days"}
        ],
        "refresh_rates": [
            {"timeframe": "≤10m", "secs": 10},
            {"timeframe": "≤1h", "secs": 20},
            {"timeframe": "≤4h", "secs": 60},
            {"timeframe": "≤1d", "secs": 180},
            {"timeframe": "≤2d", "secs": 600},
            {"timeframe": ">2d", "secs": 3600},
            {"timeframe": "public", "secs": 30}
        ],
        "widget_types": [
            "timeseries", "query_value", "toplist", "note", "group",
            "heatmap", "distribution", "pie_chart", "table", "hostmap",
            "slo", "event_stream", "alert_graph", "change", "scatter_plot",
            "funnel", "list_stream", "check_status"
        ]
    }))
}

#[derive(Debug, Deserialize)]
struct RenderQuery {
    range_ms: Option<i64>,
    /// Comma-separated tag filters applied to every widget, e.g. `env:prod`
    tags: Option<String>,
    /// Template variable selections, e.g. `env:prod,gpu_id:0`
    vars: Option<String>,
}

async fn render_board(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    Query(q): Query<RenderQuery>,
) -> impl IntoResponse {
    let tags = parse_tag_filters(q.tags.as_deref());
    let vars = parse_tag_filters(q.vars.as_deref());
    let range_ms = q.range_ms.unwrap_or(3_600_000);
    let store = state.store.clone();
    let result = tokio::task::spawn_blocking(move || {
        store.render_board(id, range_ms, &tags, &vars)
    })
    .await;
    match result {
        Ok(Ok(board)) => Json(board).into_response(),
        Ok(Err(e)) => (
            StatusCode::NOT_FOUND,
            Json(json!({ "error": e.to_string() })),
        )
            .into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({ "error": e.to_string() })),
        )
            .into_response(),
    }
}

#[derive(Debug, Deserialize)]
struct SummaryQuery {
    range_ms: Option<i64>,
}

async fn metrics_summary(
    State(state): State<AppState>,
    Query(q): Query<SummaryQuery>,
) -> impl IntoResponse {
    let range_ms = q.range_ms.unwrap_or(3_600_000);
    let store = state.store.clone();
    match tokio::task::spawn_blocking(move || store.metrics_summary(range_ms)).await {
        Ok(rows) => Json(rows).into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({ "error": e.to_string() })),
        )
            .into_response(),
    }
}
