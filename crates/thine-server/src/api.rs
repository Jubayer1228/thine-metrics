use crate::state::AppState;
use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::routing::{get, post};
use axum::{Json, Router};
use serde::Deserialize;
use serde_json::json;
use thine_common::{CreateAlertRequest, CreateBoardRequest, QueryRequest};
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
        .route("/api/v1/boards", get(list_boards).post(create_board))
        .route("/api/v1/boards/{id}", get(get_board).delete(delete_board))
        .route("/api/v1/boards/{id}/render", get(render_board))
        .route("/api/v1/metrics/summary", get(metrics_summary))
        // OpenTelemetry OTLP/HTTP (JSON) — drop-in for OTEL collectors/SDKs
        .route("/v1/metrics", post(otlp_metrics))
        .route("/otlp/v1/metrics", post(otlp_metrics))
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

async fn list_metrics(
    State(state): State<AppState>,
    Query(q): Query<ListQuery>,
) -> impl IntoResponse {
    Json(state.store.list_metrics(q.prefix.as_deref()))
}

async fn query_post(
    State(state): State<AppState>,
    Json(req): Json<QueryRequest>,
) -> impl IntoResponse {
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
    Query(q): Query<QueryParams>,
) -> impl IntoResponse {
    let mut tags = parse_tag_filters(q.tags.as_deref());
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

async fn ingest_simple(State(state): State<AppState>, body: axum::body::Bytes) -> impl IntoResponse {
    match state.ingest.ingest_simple_json(&body) {
        Ok(stats) => (StatusCode::OK, Json(stats)).into_response(),
        Err(e) => (
            StatusCode::BAD_REQUEST,
            Json(json!({ "error": e.to_string() })),
        )
            .into_response(),
    }
}

async fn ingest_statsd(State(state): State<AppState>, body: String) -> impl IntoResponse {
    Json(state.ingest.ingest_statsd_lines(&body))
}

async fn otlp_metrics(State(state): State<AppState>, body: axum::body::Bytes) -> impl IntoResponse {
    match state.ingest.ingest_otlp_json(&body) {
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
struct RenderQuery {
    range_ms: Option<i64>,
    /// Comma-separated tag filters applied to every widget, e.g. `env:prod`
    tags: Option<String>,
}

async fn render_board(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    Query(q): Query<RenderQuery>,
) -> impl IntoResponse {
    let tags = parse_tag_filters(q.tags.as_deref());
    let range_ms = q.range_ms.unwrap_or(3_600_000);
    let store = state.store.clone();
    let result = tokio::task::spawn_blocking(move || store.render_board(id, range_ms, &tags)).await;
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
