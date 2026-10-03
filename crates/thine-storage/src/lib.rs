//! Concurrent in-memory time-series store with retention and aggregation.

use chrono::Utc;
use dashmap::DashMap;
use parking_lot::RwLock;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet, VecDeque};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use thine_common::{
    series_key, tags_fingerprint, Aggregation, AlertEvent, AlertRule, AlertStatus, Comparator,
    CreateAlertRequest, CreateBoardRequest, DashboardBoard, DashboardSummary, IngestStats,
    MetricMeta, MetricPoint, MetricSeries, MetricSummaryRow, MetricType, QueryRequest, QueryResult,
    RenderedBoard, RenderedWidget, Sample, Tags, ThineError, ToplistItem, WidgetType,
};
use tracing::debug;
use uuid::Uuid;

const DEFAULT_RETENTION_MS: i64 = 24 * 60 * 60 * 1000; // 24h
const DEFAULT_MAX_POINTS: usize = 10_000;

#[derive(Debug)]
struct SeriesState {
    name: String,
    metric_type: MetricType,
    tags: Tags,
    unit: Option<String>,
    description: Option<String>,
    samples: VecDeque<Sample>,
}

impl SeriesState {
    fn push(&mut self, sample: Sample, max_points: usize, retention_ms: i64) {
        if let Some(last) = self.samples.back() {
            // Keep counters monotonic-friendly; allow out-of-order within small skew.
            if sample.timestamp_ms < last.timestamp_ms.saturating_sub(5_000) {
                return;
            }
        }
        self.samples.push_back(sample);
        let cutoff = Utc::now().timestamp_millis() - retention_ms;
        while self
            .samples
            .front()
            .map(|s| s.timestamp_ms < cutoff)
            .unwrap_or(false)
        {
            self.samples.pop_front();
        }
        while self.samples.len() > max_points {
            self.samples.pop_front();
        }
    }
}

#[derive(Debug, Clone)]
pub struct StorageConfig {
    pub retention_ms: i64,
    pub max_points_per_series: usize,
}

impl Default for StorageConfig {
    fn default() -> Self {
        Self {
            retention_ms: DEFAULT_RETENTION_MS,
            max_points_per_series: DEFAULT_MAX_POINTS,
        }
    }
}

#[derive(Debug)]
pub struct MetricStore {
    series: DashMap<String, RwLock<SeriesState>>,
    alerts: DashMap<Uuid, AlertRule>,
    boards: DashMap<Uuid, DashboardBoard>,
    alert_events: RwLock<VecDeque<AlertEvent>>,
    accepted: AtomicU64,
    rejected: AtomicU64,
    sample_count: AtomicU64,
    started_at_ms: i64,
    ingest_window: RwLock<VecDeque<i64>>,
    config: StorageConfig,
}

impl MetricStore {
    pub fn new(config: StorageConfig) -> Arc<Self> {
        Arc::new(Self {
            series: DashMap::new(),
            alerts: DashMap::new(),
            boards: DashMap::new(),
            alert_events: RwLock::new(VecDeque::with_capacity(500)),
            accepted: AtomicU64::new(0),
            rejected: AtomicU64::new(0),
            sample_count: AtomicU64::new(0),
            started_at_ms: Utc::now().timestamp_millis(),
            ingest_window: RwLock::new(VecDeque::new()),
            config,
        })
    }

    pub fn ingest_point(&self, point: MetricPoint) -> Result<(), ThineError> {
        if point.name.trim().is_empty() || point.name.len() > 256 {
            self.rejected.fetch_add(1, Ordering::Relaxed);
            return Err(ThineError::InvalidMetricName(point.name));
        }
        if !point.value_is_finite() {
            self.rejected.fetch_add(1, Ordering::Relaxed);
            return Err(ThineError::BadRequest("non-finite sample value".into()));
        }

        let key = series_key(&point.name, &point.tags);
        {
            // Scope DashMap guard so it is dropped before evaluate_alerts
            // (holding a map Ref across another get/iter deadlocks DashMap).
            let entry = self.series.entry(key).or_insert_with(|| {
                RwLock::new(SeriesState {
                    name: point.name.clone(),
                    metric_type: point.metric_type,
                    tags: point.tags.clone(),
                    unit: point.unit.clone(),
                    description: point.description.clone(),
                    samples: VecDeque::with_capacity(256),
                })
            });
            let mut state = entry.write();
            if state.unit.is_none() {
                state.unit = point.unit.clone();
            }
            if state.description.is_none() {
                state.description = point.description.clone();
            }
            state.push(
                point.sample,
                self.config.max_points_per_series,
                self.config.retention_ms,
            );
        }

        self.accepted.fetch_add(1, Ordering::Relaxed);
        self.sample_count.fetch_add(1, Ordering::Relaxed);
        self.record_ingest_ts(point.sample.timestamp_ms.max(Utc::now().timestamp_millis()));
        self.evaluate_alerts(&point.name, &point.tags);
        Ok(())
    }

    pub fn ingest_batch(&self, points: Vec<MetricPoint>) -> IngestStats {
        let mut accepted = 0u64;
        let mut rejected = 0u64;
        for p in points {
            match self.ingest_point(p) {
                Ok(()) => accepted += 1,
                Err(_) => rejected += 1,
            }
        }
        IngestStats {
            accepted,
            rejected,
            series_count: self.series.len() as u64,
            sample_count: self.sample_count.load(Ordering::Relaxed),
        }
    }

    fn record_ingest_ts(&self, ts: i64) {
        let mut window = self.ingest_window.write();
        window.push_back(ts);
        let cutoff = Utc::now().timestamp_millis() - 60_000;
        while window.front().map(|t| *t < cutoff).unwrap_or(false) {
            window.pop_front();
        }
    }

    pub fn list_metrics(&self, prefix: Option<&str>) -> Vec<MetricMeta> {
        let mut out = Vec::new();
        for entry in self.series.iter() {
            let state = entry.value().read();
            if let Some(p) = prefix {
                if !state.name.starts_with(p) {
                    continue;
                }
            }
            out.push(MetricMeta {
                name: state.name.clone(),
                metric_type: state.metric_type,
                tags: state.tags.clone(),
                unit: state.unit.clone(),
                description: state.description.clone(),
                last_value: state.samples.back().map(|s| s.value),
                last_seen_ms: state.samples.back().map(|s| s.timestamp_ms),
                sample_count: state.samples.len() as u64,
            });
        }
        out.sort_by(|a, b| a.name.cmp(&b.name));
        out
    }

    pub fn query(&self, req: QueryRequest) -> Result<Vec<QueryResult>, ThineError> {
        if req.metric.trim().is_empty() {
            return Err(ThineError::BadRequest("metric is required".into()));
        }
        let end = req.end_ms.unwrap_or_else(|| Utc::now().timestamp_millis());
        let start = req.start_ms.unwrap_or(end - 3_600_000);
        if start >= end {
            return Err(ThineError::BadRequest("start_ms must be < end_ms".into()));
        }
        let step = req.step_ms.max(1_000);

        let mut results = Vec::new();
        for entry in self.series.iter() {
            let state = entry.value().read();
            if state.name != req.metric {
                continue;
            }
            if !tags_match(&req.tags, &state.tags) {
                continue;
            }

            let samples: Vec<Sample> = state
                .samples
                .iter()
                .filter(|s| s.timestamp_ms >= start && s.timestamp_ms <= end)
                .cloned()
                .collect();

            let points = downsample(&samples, start, end, step, req.aggregation);
            results.push(QueryResult {
                metric: state.name.clone(),
                tags: state.tags.clone(),
                aggregation: req.aggregation,
                points,
            });
        }

        if results.is_empty() {
            // Still return empty series so UI can render "no data"
            results.push(QueryResult {
                metric: req.metric,
                tags: req.tags,
                aggregation: req.aggregation,
                points: Vec::new(),
            });
        }
        Ok(results)
    }

    pub fn get_series(&self, name: &str, tags: &Tags) -> Option<MetricSeries> {
        let key = series_key(name, tags);
        self.series.get(&key).map(|entry| {
            let state = entry.read();
            MetricSeries {
                name: state.name.clone(),
                metric_type: state.metric_type,
                tags: state.tags.clone(),
                samples: state.samples.iter().cloned().collect(),
                unit: state.unit.clone(),
                description: state.description.clone(),
            }
        })
    }

    pub fn stats(&self) -> IngestStats {
        IngestStats {
            accepted: self.accepted.load(Ordering::Relaxed),
            rejected: self.rejected.load(Ordering::Relaxed),
            series_count: self.series.len() as u64,
            sample_count: self.sample_count.load(Ordering::Relaxed),
        }
    }

    pub fn dashboard(&self) -> DashboardSummary {
        let mut top = self.list_metrics(None);
        top.sort_by(|a, b| b.sample_count.cmp(&a.sample_count));
        top.truncate(8);

        let window = self.ingest_window.read();
        let ingest_rate = window.len() as f64 / 60.0;
        let active_alerts = self
            .alert_events
            .read()
            .iter()
            .filter(|e| e.status == AlertStatus::Firing)
            .count() as u64;

        DashboardSummary {
            series_count: self.series.len() as u64,
            sample_count: self.sample_count.load(Ordering::Relaxed),
            ingest_rate_per_sec: ingest_rate,
            active_alerts,
            uptime_secs: ((Utc::now().timestamp_millis() - self.started_at_ms).max(0) / 1000)
                as u64,
            top_metrics: top,
        }
    }

    pub fn create_alert(&self, req: CreateAlertRequest) -> AlertRule {
        let rule = AlertRule {
            id: Uuid::new_v4(),
            name: req.name,
            metric: req.metric,
            tags: req.tags,
            threshold: req.threshold,
            comparator: req.comparator,
            window_ms: req.window_ms,
            enabled: req.enabled,
            created_at: Utc::now(),
        };
        self.alerts.insert(rule.id, rule.clone());
        rule
    }

    pub fn list_alerts(&self) -> Vec<AlertRule> {
        let mut rules: Vec<_> = self.alerts.iter().map(|e| e.value().clone()).collect();
        rules.sort_by(|a, b| a.name.cmp(&b.name));
        rules
    }

    pub fn delete_alert(&self, id: Uuid) -> bool {
        self.alerts.remove(&id).is_some()
    }

    pub fn list_alert_events(&self, limit: usize) -> Vec<AlertEvent> {
        self.alert_events
            .read()
            .iter()
            .rev()
            .take(limit)
            .cloned()
            .collect()
    }

    pub fn create_board(&self, req: CreateBoardRequest) -> DashboardBoard {
        let board = DashboardBoard {
            id: Uuid::new_v4(),
            name: req.name,
            description: req.description,
            widgets: req.widgets,
            created_at: Utc::now(),
        };
        self.boards.insert(board.id, board.clone());
        board
    }

    pub fn list_boards(&self) -> Vec<DashboardBoard> {
        let mut boards: Vec<_> = self.boards.iter().map(|e| e.value().clone()).collect();
        boards.sort_by(|a, b| a.name.cmp(&b.name));
        boards
    }

    pub fn get_board(&self, id: Uuid) -> Option<DashboardBoard> {
        self.boards.get(&id).map(|e| e.value().clone())
    }

    pub fn delete_board(&self, id: Uuid) -> bool {
        self.boards.remove(&id).is_some()
    }

    /// Granular Metrics Summary — Datadog Metrics Summary style stats + sparkline.
    pub fn metrics_summary(&self, range_ms: i64) -> Vec<MetricSummaryRow> {
        let end = Utc::now().timestamp_millis();
        let start = end - range_ms.max(60_000);
        let step = (range_ms / 40).max(5_000);

        let mut by_name: BTreeMap<String, Vec<(MetricType, Option<String>, Tags, Vec<Sample>)>> =
            BTreeMap::new();

        for entry in self.series.iter() {
            let state = entry.value().read();
            let samples: Vec<Sample> = state
                .samples
                .iter()
                .filter(|s| s.timestamp_ms >= start && s.timestamp_ms <= end)
                .cloned()
                .collect();
            by_name.entry(state.name.clone()).or_default().push((
                state.metric_type,
                state.unit.clone(),
                state.tags.clone(),
                samples,
            ));
        }

        let mut rows = Vec::new();
        for (name, series_list) in by_name {
            let metric_type = series_list.first().map(|s| s.0).unwrap_or(MetricType::Gauge);
            let unit = series_list.iter().find_map(|s| s.1.clone());
            let mut tag_keys = BTreeSet::new();
            let mut all_values = Vec::new();
            let mut merged: BTreeMap<i64, Vec<f64>> = BTreeMap::new();

            for (_ty, _unit, tags, samples) in &series_list {
                for k in tags.keys() {
                    tag_keys.insert(k.clone());
                }
                for s in samples {
                    all_values.push(s.value);
                    let bucket = (s.timestamp_ms / step) * step;
                    merged.entry(bucket).or_default().push(s.value);
                }
            }

            let sparkline: Vec<Sample> = merged
                .into_iter()
                .map(|(ts, vals)| Sample {
                    timestamp_ms: ts,
                    value: vals.iter().sum::<f64>() / vals.len() as f64,
                })
                .collect();

            let last_value = sparkline.last().map(|s| s.value);
            let (avg, min, max) = if all_values.is_empty() {
                (None, None, None)
            } else {
                let sum: f64 = all_values.iter().sum();
                (
                    Some(sum / all_values.len() as f64),
                    Some(all_values.iter().cloned().fold(f64::INFINITY, f64::min)),
                    Some(all_values.iter().cloned().fold(f64::NEG_INFINITY, f64::max)),
                )
            };

            rows.push(MetricSummaryRow {
                name,
                metric_type,
                series_count: series_list.len() as u64,
                tag_keys: tag_keys.into_iter().collect(),
                last_value,
                avg,
                min,
                max,
                unit,
                sparkline,
            });
        }
        rows
    }

    /// Render a board with resolved widget data for a live time window.
    pub fn render_board(
        &self,
        id: Uuid,
        range_ms: i64,
        filter_tags: &Tags,
    ) -> Result<RenderedBoard, ThineError> {
        let board = self
            .get_board(id)
            .ok_or_else(|| ThineError::NotFound(id.to_string()))?;
        let end = Utc::now().timestamp_millis();
        let start = end - range_ms.max(60_000);
        let step = ((range_ms / 60).max(5_000)).min(60_000);
        let prev_end = start;
        let prev_start = start - range_ms;

        let mut widgets = Vec::new();
        for w in &board.widgets {
            let mut tags = w.tags.clone();
            for (k, v) in filter_tags {
                tags.insert(k.clone(), v.clone());
            }

            match w.widget_type {
                WidgetType::Group | WidgetType::Note => {
                    widgets.push(RenderedWidget {
                        id: w.id.clone(),
                        widget_type: w.widget_type,
                        title: w.title.clone(),
                        layout: w.layout.clone(),
                        unit: w.unit.clone(),
                        display: w.display.clone(),
                        text: w.text.clone().or_else(|| Some(w.title.clone())),
                        value: None,
                        previous_value: None,
                        change_pct: None,
                        sparkline: Vec::new(),
                        series: Vec::new(),
                        toplist: Vec::new(),
                    });
                }
                WidgetType::QueryValue => {
                    let current = self
                        .query(QueryRequest {
                            metric: w.metric.clone(),
                            tags: tags.clone(),
                            start_ms: Some(start),
                            end_ms: Some(end),
                            step_ms: step,
                            aggregation: w.aggregation,
                        })
                        .unwrap_or_default();
                    let previous = self
                        .query(QueryRequest {
                            metric: w.metric.clone(),
                            tags: tags.clone(),
                            start_ms: Some(prev_start),
                            end_ms: Some(prev_end),
                            step_ms: step,
                            aggregation: w.aggregation,
                        })
                        .unwrap_or_default();

                    let sparkline = merge_series_avg(&current);
                    let value = sparkline.last().map(|s| s.value).or_else(|| {
                        current
                            .iter()
                            .flat_map(|s| s.points.iter().map(|p| p.value))
                            .last()
                    });
                    let prev_spark = merge_series_avg(&previous);
                    let previous_value = prev_spark.last().map(|s| s.value);
                    let change_pct = match (value, previous_value) {
                        (Some(v), Some(p)) if p.abs() > f64::EPSILON => {
                            Some(((v - p) / p.abs()) * 100.0)
                        }
                        _ => None,
                    };

                    widgets.push(RenderedWidget {
                        id: w.id.clone(),
                        widget_type: WidgetType::QueryValue,
                        title: w.title.clone(),
                        layout: w.layout.clone(),
                        unit: w.unit.clone(),
                        display: w.display.clone(),
                        text: None,
                        value,
                        previous_value,
                        change_pct,
                        sparkline,
                        series: Vec::new(),
                        toplist: Vec::new(),
                    });
                }
                WidgetType::Timeseries => {
                    let mut series = self
                        .query(QueryRequest {
                            metric: w.metric.clone(),
                            tags: tags.clone(),
                            start_ms: Some(start),
                            end_ms: Some(end),
                            step_ms: step,
                            aggregation: w.aggregation,
                        })
                        .unwrap_or_default();
                    series.retain(|s| !s.points.is_empty());
                    if let Some(key) = w.group_by.as_deref() {
                        series.retain(|s| s.tags.contains_key(key));
                    }
                    widgets.push(RenderedWidget {
                        id: w.id.clone(),
                        widget_type: WidgetType::Timeseries,
                        title: w.title.clone(),
                        layout: w.layout.clone(),
                        unit: w.unit.clone(),
                        display: w.display.clone(),
                        text: None,
                        value: None,
                        previous_value: None,
                        change_pct: None,
                        sparkline: Vec::new(),
                        series,
                        toplist: Vec::new(),
                    });
                }
                WidgetType::Toplist => {
                    let series = self
                        .query(QueryRequest {
                            metric: w.metric.clone(),
                            tags: tags.clone(),
                            start_ms: Some(start),
                            end_ms: Some(end),
                            step_ms: step,
                            aggregation: w.aggregation,
                        })
                        .unwrap_or_default();
                    let mut items: Vec<ToplistItem> = series
                        .into_iter()
                        .filter(|s| !s.points.is_empty())
                        .map(|s| {
                            let value = match w.aggregation {
                                Aggregation::Sum => s.points.iter().map(|p| p.value).sum(),
                                Aggregation::Max => s
                                    .points
                                    .iter()
                                    .map(|p| p.value)
                                    .fold(f64::NEG_INFINITY, f64::max),
                                Aggregation::Min => {
                                    s.points.iter().map(|p| p.value).fold(f64::INFINITY, f64::min)
                                }
                                Aggregation::Last => s.points.last().map(|p| p.value).unwrap_or(0.0),
                                Aggregation::Count => s.points.len() as f64,
                                Aggregation::Avg => {
                                    s.points.iter().map(|p| p.value).sum::<f64>()
                                        / s.points.len() as f64
                                }
                            };
                            let label = if let Some(key) = w.group_by.as_deref() {
                                s.tags
                                    .get(key)
                                    .cloned()
                                    .unwrap_or_else(|| tags_fingerprint(&s.tags))
                            } else {
                                tags_fingerprint(&s.tags)
                            };
                            ToplistItem {
                                label,
                                value,
                                tags: s.tags,
                            }
                        })
                        .collect();
                    items.sort_by(|a, b| {
                        b.value
                            .partial_cmp(&a.value)
                            .unwrap_or(std::cmp::Ordering::Equal)
                    });
                    items.truncate(10);
                    widgets.push(RenderedWidget {
                        id: w.id.clone(),
                        widget_type: WidgetType::Toplist,
                        title: w.title.clone(),
                        layout: w.layout.clone(),
                        unit: w.unit.clone(),
                        display: w.display.clone(),
                        text: None,
                        value: None,
                        previous_value: None,
                        change_pct: None,
                        sparkline: Vec::new(),
                        series: Vec::new(),
                        toplist: items,
                    });
                }
            }
        }

        Ok(RenderedBoard {
            id: board.id,
            name: board.name,
            description: board.description,
            range_ms,
            start_ms: start,
            end_ms: end,
            widgets,
        })
    }

    fn evaluate_alerts(&self, metric: &str, tags: &Tags) {
        let now = Utc::now().timestamp_millis();
        for entry in self.alerts.iter() {
            let rule = entry.value();
            if !rule.enabled || rule.metric != metric || !tags_match(&rule.tags, tags) {
                continue;
            }
            let Some(series) = self.get_series(metric, tags) else {
                continue;
            };
            let window_start = now - rule.window_ms;
            let values: Vec<f64> = series
                .samples
                .iter()
                .filter(|s| s.timestamp_ms >= window_start)
                .map(|s| s.value)
                .collect();
            if values.is_empty() {
                continue;
            }
            let avg = values.iter().sum::<f64>() / values.len() as f64;
            if compare(avg, rule.threshold, rule.comparator) {
                debug!(rule = %rule.name, value = avg, "alert firing");
                let event = AlertEvent {
                    id: Uuid::new_v4(),
                    rule_id: rule.id,
                    rule_name: rule.name.clone(),
                    metric: metric.to_string(),
                    value: avg,
                    threshold: rule.threshold,
                    fired_at: Utc::now(),
                    status: AlertStatus::Firing,
                };
                let mut events = self.alert_events.write();
                events.push_back(event);
                while events.len() > 500 {
                    events.pop_front();
                }
            }
        }
    }
}

trait FiniteCheck {
    fn value_is_finite(&self) -> bool;
}

impl FiniteCheck for MetricPoint {
    fn value_is_finite(&self) -> bool {
        self.sample.value.is_finite()
    }
}

fn merge_series_avg(series: &[QueryResult]) -> Vec<Sample> {
    let mut buckets: BTreeMap<i64, Vec<f64>> = BTreeMap::new();
    for s in series {
        for p in &s.points {
            buckets.entry(p.timestamp_ms).or_default().push(p.value);
        }
    }
    buckets
        .into_iter()
        .map(|(ts, vals)| Sample {
            timestamp_ms: ts,
            value: vals.iter().sum::<f64>() / vals.len() as f64,
        })
        .collect()
}

fn tags_match(filter: &Tags, actual: &Tags) -> bool {
    filter.iter().all(|(k, v)| actual.get(k).map(|av| av == v).unwrap_or(false))
}

fn compare(value: f64, threshold: f64, op: Comparator) -> bool {
    match op {
        Comparator::Gt => value > threshold,
        Comparator::Gte => value >= threshold,
        Comparator::Lt => value < threshold,
        Comparator::Lte => value <= threshold,
        Comparator::Eq => (value - threshold).abs() < f64::EPSILON,
    }
}

fn downsample(
    samples: &[Sample],
    start: i64,
    end: i64,
    step: i64,
    agg: Aggregation,
) -> Vec<Sample> {
    if samples.is_empty() {
        return Vec::new();
    }
    let mut out = Vec::new();
    let mut bucket_start = start;
    while bucket_start < end {
        let bucket_end = (bucket_start + step).min(end);
        let bucket: Vec<f64> = samples
            .iter()
            .filter(|s| s.timestamp_ms >= bucket_start && s.timestamp_ms < bucket_end)
            .map(|s| s.value)
            .collect();
        if !bucket.is_empty() {
            let value = match agg {
                Aggregation::Avg => bucket.iter().sum::<f64>() / bucket.len() as f64,
                Aggregation::Sum => bucket.iter().sum(),
                Aggregation::Min => bucket.iter().cloned().fold(f64::INFINITY, f64::min),
                Aggregation::Max => bucket.iter().cloned().fold(f64::NEG_INFINITY, f64::max),
                Aggregation::Count => bucket.len() as f64,
                Aggregation::Last => *bucket.last().unwrap(),
            };
            out.push(Sample {
                timestamp_ms: bucket_start,
                value,
            });
        }
        bucket_start = bucket_end;
    }
    out
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HealthStatus {
    pub status: &'static str,
    pub version: &'static str,
}

pub fn health() -> HealthStatus {
    HealthStatus {
        status: "ok",
        version: env!("CARGO_PKG_VERSION"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use thine_common::{MetricType, Sample};

    fn point(name: &str, value: f64, ts: i64) -> MetricPoint {
        MetricPoint {
            name: name.into(),
            metric_type: MetricType::Gauge,
            tags: Tags::from([("service".into(), "demo".into())]),
            sample: Sample {
                timestamp_ms: ts,
                value,
            },
            unit: Some("1".into()),
            description: None,
        }
    }

    #[test]
    fn ingest_and_query_avg() {
        let store = MetricStore::new(StorageConfig::default());
        let now = Utc::now().timestamp_millis();
        for i in 0..10 {
            store
                .ingest_point(point("cpu.usage", i as f64, now - 9_000 + i * 1_000))
                .unwrap();
        }
        let results = store
            .query(QueryRequest {
                metric: "cpu.usage".into(),
                tags: Tags::new(),
                start_ms: Some(now - 10_000),
                end_ms: Some(now + 1),
                step_ms: 10_000,
                aggregation: Aggregation::Avg,
            })
            .unwrap();
        assert_eq!(results.len(), 1);
        assert!(!results[0].points.is_empty());
    }

    #[test]
    fn rejects_empty_name() {
        let store = MetricStore::new(StorageConfig::default());
        let err = store
            .ingest_point(point("", 1.0, Utc::now().timestamp_millis()))
            .unwrap_err();
        assert!(matches!(err, ThineError::InvalidMetricName(_)));
    }
}
