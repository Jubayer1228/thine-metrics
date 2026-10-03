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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AlertRule {
    pub id: Uuid,
    pub name: String,
    pub metric: String,
    #[serde(default)]
    pub tags: Tags,
    pub threshold: f64,
    pub comparator: Comparator,
    pub window_ms: i64,
    pub enabled: bool,
    pub created_at: DateTime<Utc>,
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
    #[serde(default)]
    pub tags: Tags,
    pub threshold: f64,
    pub comparator: Comparator,
    #[serde(default = "default_window")]
    pub window_ms: i64,
    #[serde(default = "default_true")]
    pub enabled: bool,
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

/// Datadog-aligned widget types (subset of screenboard/timeboard widgets).
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub enum WidgetType {
    #[default]
    Timeseries,
    QueryValue,
    Toplist,
    Note,
    Group,
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
    pub widgets: Vec<DashboardWidget>,
    pub created_at: DateTime<Utc>,
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
    pub widgets: Vec<DashboardWidget>,
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
    /// timeseries
    #[serde(default)]
    pub series: Vec<QueryResult>,
    /// toplist
    #[serde(default)]
    pub toplist: Vec<ToplistItem>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RenderedBoard {
    pub id: Uuid,
    pub name: String,
    pub description: Option<String>,
    pub range_ms: i64,
    pub start_ms: i64,
    pub end_ms: i64,
    pub widgets: Vec<RenderedWidget>,
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
}
