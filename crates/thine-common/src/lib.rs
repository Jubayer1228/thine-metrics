//! Shared metric types used across ingest, storage, and query.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use thiserror::Error;
use uuid::Uuid;

pub type Tags = BTreeMap<String, String>;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MetricType {
    Gauge,
    Counter,
    Histogram,
    Summary,
    Untyped,
}

impl Default for MetricType {
    fn default() -> Self {
        Self::Gauge
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
pub struct Sample {
    pub timestamp_ms: i64,
    pub value: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MetricPoint {
    pub name: String,
    pub metric_type: MetricType,
    pub tags: Tags,
    pub sample: Sample,
    #[serde(default)]
    pub unit: Option<String>,
    #[serde(default)]
    pub description: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MetricSeries {
    pub name: String,
    pub metric_type: MetricType,
    pub tags: Tags,
    pub samples: Vec<Sample>,
    #[serde(default)]
    pub unit: Option<String>,
    #[serde(default)]
    pub description: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MetricMeta {
    pub name: String,
    pub metric_type: MetricType,
    pub tags: Tags,
    pub unit: Option<String>,
    pub description: Option<String>,
    pub last_value: Option<f64>,
    pub last_seen_ms: Option<i64>,
    pub sample_count: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QueryRequest {
    pub metric: String,
    #[serde(default)]
    pub tags: Tags,
    pub start_ms: Option<i64>,
    pub end_ms: Option<i64>,
    #[serde(default = "default_step")]
    pub step_ms: i64,
    #[serde(default)]
    pub aggregation: Aggregation,
}

fn default_step() -> i64 {
    15_000
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub enum Aggregation {
    #[default]
    Avg,
    Sum,
    Min,
    Max,
    Count,
    Last,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QueryResult {
    pub metric: String,
    pub tags: Tags,
    pub aggregation: Aggregation,
    pub points: Vec<Sample>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IngestStats {
    pub accepted: u64,
    pub rejected: u64,
    pub series_count: u64,
    pub sample_count: u64,
}

/// Datadog-style monitor options (not used as metric tag filters).
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct AlertOptions {
    /// threshold | change | anomaly | forecast
    #[serde(default = "default_detection_method")]
    pub detection_method: String,
    #[serde(default)]
    pub warning_threshold: Option<f64>,
    #[serde(default)]
    pub recovery_threshold: Option<f64>,
    #[serde(default)]
    pub recipients: String,
    /// avg | max | min | sum
    #[serde(default = "default_evaluate")]
    pub evaluate: String,
    /// change | pct_change
    #[serde(default)]
    pub change_type: String,
    #[serde(default)]
    pub comparison_window_ms: i64,
    /// above_or_below | above | below
    #[serde(default)]
    pub anomaly_direction: String,
    #[serde(default)]
    pub forecast_horizon_ms: i64,
    /// Optional multi-alert group-by tag key (e.g. host, service)
    #[serde(default)]
    pub group_by: Option<String>,
    #[serde(default)]
    pub message: String,
    #[serde(default = "default_severity")]
    pub severity: String,
}

fn default_detection_method() -> String {
    "threshold".into()
}
fn default_evaluate() -> String {
    "avg".into()
}
fn default_severity() -> String {
    "critical".into()
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AlertRule {
    pub id: Uuid,
    pub name: String,
    pub metric: String,
    /// Metric filter tags only (env:prod). Monitor metadata lives in `options`.
    #[serde(default)]
    pub tags: Tags,
    pub threshold: f64,
    pub comparator: Comparator,
    pub window_ms: i64,
    pub enabled: bool,
    pub created_at: DateTime<Utc>,
    #[serde(default)]
    pub options: AlertOptions,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Comparator {
    Gt,
    Gte,
    Lt,
    Lte,
    Eq,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AlertEvent {
    pub id: Uuid,
    pub rule_id: Uuid,
    pub rule_name: String,
    pub metric: String,
    pub value: f64,
    pub threshold: f64,
    pub fired_at: DateTime<Utc>,
    pub status: AlertStatus,
    /// Multi-alert group key (e.g. host:web-1) — empty for simple alerts.
    #[serde(default)]
    pub group_key: String,
    #[serde(default)]
    pub group_tags: Tags,
    #[serde(default)]
    pub detection_method: String,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum AlertStatus {
    Firing,
    Resolved,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateAlertRequest {
    pub name: String,
    pub metric: String,
    /// Metric filter tags only.
    #[serde(default)]
    pub tags: Tags,
    pub threshold: f64,
    pub comparator: Comparator,
    #[serde(default = "default_window")]
    pub window_ms: i64,
    #[serde(default = "default_true")]
    pub enabled: bool,
    #[serde(default)]
    pub options: AlertOptions,
}

fn default_window() -> i64 {
    60_000
}

fn default_true() -> bool {
    true
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DashboardSummary {
    pub series_count: u64,
    pub sample_count: u64,
    pub ingest_rate_per_sec: f64,
    pub active_alerts: u64,
    pub uptime_secs: u64,
    pub top_metrics: Vec<MetricMeta>,
}

/// Datadog-aligned widget types (screenboard / timeboard / dashboard widgets).
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub enum WidgetType {
    #[default]
    Timeseries,
    QueryValue,
    Toplist,
    Note,
    Group,
    Heatmap,
    Distribution,
    PieChart,
    Table,
    Hostmap,
    Slo,
    EventStream,
    AlertGraph,
    Change,
    ScatterPlot,
    Funnel,
    ListStream,
    CheckStatus,
}

/// Layout family from Datadog docs: Dashboard (grid), Timeboard, Screenboard.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub enum BoardLayoutKind {
    #[default]
    Dashboard,
    Timeboard,
    Screenboard,
}

/// Template variable — dynamically filter/group widgets (`$env`, `$service`).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TemplateVariable {
    pub name: String,
    /// Tag / attribute key (e.g. `env`, `service`, `gpu_id`).
    pub tag: String,
    #[serde(default = "default_star")]
    pub default: String,
    #[serde(default)]
    pub available_values: Vec<String>,
    /// `filter` or `groupby`
    #[serde(default = "default_filter")]
    pub prefix: String,
}

fn default_star() -> String {
    "*".into()
}
fn default_filter() -> String {
    "filter".into()
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SavedView {
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub description: Option<String>,
    /// Variable name → selected value
    pub selections: BTreeMap<String, String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EventOverlay {
    #[serde(default)]
    pub query: String,
    #[serde(default)]
    pub enabled: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BoardAnnotation {
    pub id: String,
    pub timestamp_ms: i64,
    pub label: String,
    #[serde(default)]
    pub color: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ShareConfig {
    #[serde(default)]
    pub public: bool,
    #[serde(default)]
    pub token: Option<String>,
    /// Public boards refresh every 30s per Datadog docs.
    #[serde(default = "default_public_refresh")]
    pub refresh_secs: u64,
}

fn default_public_refresh() -> u64 {
    30
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DashboardList {
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub board_ids: Vec<Uuid>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WidgetLayout {
    #[serde(default)]
    pub x: u32,
    #[serde(default)]
    pub y: u32,
    #[serde(default = "default_w")]
    pub w: u32,
    #[serde(default = "default_h")]
    pub h: u32,
}

fn default_w() -> u32 {
    4
}
fn default_h() -> u32 {
    3
}

impl Default for WidgetLayout {
    fn default() -> Self {
        Self {
            x: 0,
            y: 0,
            w: default_w(),
            h: default_h(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DashboardBoard {
    pub id: Uuid,
    pub name: String,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub layout_type: BoardLayoutKind,
    #[serde(default)]
    pub widgets: Vec<DashboardWidget>,
    #[serde(default)]
    pub template_variables: Vec<TemplateVariable>,
    #[serde(default)]
    pub saved_views: Vec<SavedView>,
    #[serde(default)]
    pub event_overlay: Option<EventOverlay>,
    #[serde(default)]
    pub annotations: Vec<BoardAnnotation>,
    #[serde(default)]
    pub share: Option<ShareConfig>,
    #[serde(default)]
    pub tags: Vec<String>,
    #[serde(default)]
    pub author: Option<String>,
    pub created_at: DateTime<Utc>,
    #[serde(default)]
    pub updated_at: Option<DateTime<Utc>>,
    /// Soft-delete timestamp (Recently Deleted list).
    #[serde(default)]
    pub deleted_at: Option<DateTime<Utc>>,
    /// Permanently deleted after this time (Datadog: 30 days).
    #[serde(default)]
    pub recoverable_until: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DashboardWidget {
    #[serde(default = "new_widget_id")]
    pub id: String,
    #[serde(default, rename = "type")]
    pub widget_type: WidgetType,
    pub title: String,
    #[serde(default)]
    pub metric: String,
    #[serde(default)]
    pub tags: Tags,
    #[serde(default)]
    pub aggregation: Aggregation,
    /// Space aggregation / split key, e.g. `service` (Datadog "by {tag}").
    #[serde(default)]
    pub group_by: Option<String>,
    #[serde(default)]
    pub layout: WidgetLayout,
    #[serde(default)]
    pub unit: Option<String>,
    /// line | area | bars
    #[serde(default)]
    pub display: Option<String>,
    /// For note/group widgets.
    #[serde(default)]
    pub text: Option<String>,
    /// Datadog query functions applied before graphing (e.g. `anomalies`, `rollup`, `rate`).
    #[serde(default)]
    pub functions: Vec<String>,
    /// Template variables this widget listens to (names without `$`).
    #[serde(default)]
    pub template_vars: Vec<String>,
    /// Optional tab / section name for large boards.
    #[serde(default)]
    pub tab: Option<String>,
    /// Event overlay query for this widget (`$env` supported).
    #[serde(default)]
    pub events_query: Option<String>,
    /// Hide this widget from dashboard anomaly detection (Datadog Graph Insights).
    #[serde(default)]
    pub hide_anomaly_detection: bool,
    /// Screenboard accent for query_value tiles: green | red | orange | purple | yellow.
    #[serde(default)]
    pub accent: Option<String>,
}

fn new_widget_id() -> String {
    Uuid::new_v4().to_string()
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateBoardRequest {
    pub name: String,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub layout_type: BoardLayoutKind,
    #[serde(default)]
    pub widgets: Vec<DashboardWidget>,
    #[serde(default)]
    pub template_variables: Vec<TemplateVariable>,
    #[serde(default)]
    pub tags: Vec<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct UpdateBoardRequest {
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub layout_type: Option<BoardLayoutKind>,
    #[serde(default)]
    pub widgets: Option<Vec<DashboardWidget>>,
    #[serde(default)]
    pub template_variables: Option<Vec<TemplateVariable>>,
    #[serde(default)]
    pub saved_views: Option<Vec<SavedView>>,
    #[serde(default)]
    pub event_overlay: Option<EventOverlay>,
    #[serde(default)]
    pub annotations: Option<Vec<BoardAnnotation>>,
    #[serde(default)]
    pub share: Option<ShareConfig>,
    #[serde(default)]
    pub tags: Option<Vec<String>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToplistItem {
    pub label: String,
    pub value: f64,
    pub tags: Tags,
}

/// Resolved widget payload for dashboard rendering (Datadog-style).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RenderedWidget {
    pub id: String,
    #[serde(rename = "type")]
    pub widget_type: WidgetType,
    pub title: String,
    pub layout: WidgetLayout,
    #[serde(default)]
    pub unit: Option<String>,
    #[serde(default)]
    pub display: Option<String>,
    #[serde(default)]
    pub text: Option<String>,
    /// query_value
    #[serde(default)]
    pub value: Option<f64>,
    #[serde(default)]
    pub previous_value: Option<f64>,
    #[serde(default)]
    pub change_pct: Option<f64>,
    #[serde(default)]
    pub sparkline: Vec<Sample>,
    /// timeseries / heatmap / distribution
    #[serde(default)]
    pub series: Vec<QueryResult>,
    /// toplist / pie / table
    #[serde(default)]
    pub toplist: Vec<ToplistItem>,
    #[serde(default)]
    pub overlays: Vec<BoardAnnotation>,
    #[serde(default)]
    pub tab: Option<String>,
    #[serde(default)]
    pub functions: Vec<String>,
    /// Graph Insights anomaly regions (pink highlights).
    #[serde(default)]
    pub anomalies: Vec<GraphAnomalyRegion>,
    /// Screenboard tile accent color.
    #[serde(default)]
    pub accent: Option<String>,
}

/// Highlighted anomaly window on a timeseries (Watchdog Explains / dashboard anomalies).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GraphAnomalyRegion {
    pub start_ms: i64,
    pub end_ms: i64,
    pub severity: String,
    /// e.g. "The spike is 70% higher than the expected range"
    pub summary: String,
    pub deviation_pct: f64,
    pub z_score: f64,
    #[serde(default)]
    pub influential_tags: Vec<TagContribution>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TagContribution {
    pub key: String,
    pub value: String,
    /// Relative contribution 0..1
    pub contribution: f64,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RenderedBoard {
    pub id: Uuid,
    pub name: String,
    pub description: Option<String>,
    pub layout_type: BoardLayoutKind,
    pub range_ms: i64,
    pub start_ms: i64,
    pub end_ms: i64,
    /// Recommended refresh seconds for this timeframe (Datadog refresh table).
    pub refresh_secs: u64,
    pub template_variables: Vec<TemplateVariable>,
    /// Resolved template variable selections used for this render.
    pub template_selections: BTreeMap<String, String>,
    pub saved_views: Vec<SavedView>,
    pub annotations: Vec<BoardAnnotation>,
    pub share: Option<ShareConfig>,
    pub widgets: Vec<RenderedWidget>,
    pub tabs: Vec<String>,
}

/// Granular Metrics Summary row (Datadog Metrics Summary parity).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MetricSummaryRow {
    pub name: String,
    pub metric_type: MetricType,
    pub series_count: u64,
    pub tag_keys: Vec<String>,
    pub last_value: Option<f64>,
    pub avg: Option<f64>,
    pub min: Option<f64>,
    pub max: Option<f64>,
    pub unit: Option<String>,
    pub sparkline: Vec<Sample>,
}

#[derive(Debug, Error)]
pub enum ThineError {
    #[error("invalid metric name: {0}")]
    InvalidMetricName(String),
    #[error("metric not found: {0}")]
    NotFound(String),
    #[error("bad request: {0}")]
    BadRequest(String),
    #[error("storage error: {0}")]
    Storage(String),
}

pub fn now_ms() -> i64 {
    Utc::now().timestamp_millis()
}

pub fn tags_fingerprint(tags: &Tags) -> String {
    tags.iter()
        .map(|(k, v)| format!("{k}={v}"))
        .collect::<Vec<_>>()
        .join(",")
}

pub fn series_key(name: &str, tags: &Tags) -> String {
    format!("{name}|{}", tags_fingerprint(tags))
}

/// Which on-ramp produced the payload (Datadog: native Agent/SDK vs OTel).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum IngestPath {
    /// DogStatsD / DD SDK / check-style native JSON
    Native,
    /// OTLP metrics / traces / logs
    Otel,
}

/// Signal fan-out target after intake normalization (Datadog: RTDB / APM / Husky).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SignalKind {
    Metrics,
    Traces,
    Logs,
    Events,
}

/// Normalized span after OTel→Thine mapping (same model for native APM ingest).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IntakeSpan {
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
    #[serde(default)]
    pub tags: Tags,
}

/// Normalized log event after intake mapping.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IntakeLog {
    pub timestamp_ms: i64,
    pub level: String,
    pub service: String,
    pub message: String,
    #[serde(default)]
    pub attrs: Tags,
}

/// Lightweight event (DogStatsD `_e` / check transitions) → event store + overlays.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IntakeEvent {
    pub timestamp_ms: i64,
    pub title: String,
    pub text: String,
    #[serde(default)]
    pub alert_type: String,
    #[serde(default)]
    pub tags: Tags,
    #[serde(default)]
    pub source: String,
}

/// Map OpenTelemetry resource/attribute keys → Datadog-style tags.
/// After this step, native vs OTel payloads share one tag model.
pub fn normalize_tags(mut tags: Tags) -> Tags {
    const REMAP: &[(&str, &str)] = &[
        ("service.name", "service"),
        ("deployment.environment", "env"),
        ("service.namespace", "kube_namespace"),
        ("k8s.namespace.name", "kube_namespace"),
        ("k8s.pod.name", "pod_name"),
        ("k8s.node.name", "kube_node"),
        ("k8s.cluster.name", "kube_cluster_name"),
        ("host.name", "host"),
        ("container.id", "container_id"),
        ("container.name", "container_name"),
        ("process.pid", "pid"),
        ("telemetry.sdk.language", "language"),
        ("telemetry.sdk.name", "telemetry_sdk"),
        ("cloud.provider", "cloud_provider"),
        ("cloud.region", "region"),
        ("faas.name", "functionname"),
    ];
    for (from, to) in REMAP {
        if let Some(v) = tags.remove(*from) {
            tags.entry((*to).into()).or_insert(v);
        }
    }
    tags
}

pub fn tag_or<'a>(tags: &'a Tags, key: &str, default: &'a str) -> &'a str {
    tags.get(key).map(|s| s.as_str()).unwrap_or(default)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn series_key_is_stable() {
        let mut tags = Tags::new();
        tags.insert("service".into(), "api".into());
        tags.insert("env".into(), "prod".into());
        assert_eq!(series_key("http.requests", &tags), "http.requests|env=prod,service=api");
    }

    #[test]
    fn otel_tags_normalize_to_dd_style() {
        let mut tags = Tags::new();
        tags.insert("service.name".into(), "checkout".into());
        tags.insert("deployment.environment".into(), "prod".into());
        tags.insert("host.name".into(), "i-1".into());
        let n = normalize_tags(tags);
        assert_eq!(n.get("service").unwrap(), "checkout");
        assert_eq!(n.get("env").unwrap(), "prod");
        assert_eq!(n.get("host").unwrap(), "i-1");
        assert!(!n.contains_key("service.name"));
    }
}
