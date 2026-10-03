//! Concurrent in-memory time-series store with retention and aggregation.

use chrono::Utc;
use dashmap::DashMap;
use parking_lot::RwLock;
use serde::{Deserialize, Serialize};
use std::collections::VecDeque;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use thine_common::{
    series_key, Aggregation, AlertEvent, AlertRule, AlertStatus, Comparator, CreateAlertRequest,
    CreateBoardRequest, DashboardBoard, DashboardSummary, IngestStats, MetricMeta, MetricPoint,
    MetricSeries, MetricType, QueryRequest, QueryResult, Sample, Tags, ThineError,
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

        {
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

    pub fn delete_board(&self, id: Uuid) -> bool {
        self.boards.remove(&id).is_some()
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
