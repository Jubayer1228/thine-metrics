//! Signal-specific storage engines (Datadog-style):
//! - MetricStore + RtdbEngine (hot TS + tag index + WAL)
//! - HuskyStore (log/event fragments + catalog)
//! - TraceStore (sampled indexed traces)

mod husky;
mod rtdb;
mod trace_store;

pub use husky::{HuskyEvent, HuskyLog, HuskyStore};
pub use rtdb::RtdbEngine;
pub use trace_store::{StoredSpan, TraceStore};

use chrono::Utc;
use dashmap::DashMap;
use parking_lot::RwLock;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet, VecDeque};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use thine_common::{
    series_key, tags_fingerprint, Aggregation, AlertEvent, AlertRule, AlertStatus, Comparator,
    AlertOptions, CreateAlertRequest, CreateBoardRequest, DashboardBoard, DashboardList,
    DashboardSummary,
    DashboardWidget, IngestStats, MetricMeta, MetricPoint, MetricSeries, MetricSummaryRow,
    MetricType, QueryRequest, QueryResult, RenderedBoard, RenderedWidget, Sample, ShareConfig,
    Tags, ThineError, ToplistItem, UpdateBoardRequest, WidgetType,
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
    board_lists: DashMap<String, DashboardList>,
    /// Datadog Clipboard — copy/paste widgets across boards.
    clipboard: RwLock<Vec<DashboardWidget>>,
    alert_events: RwLock<VecDeque<AlertEvent>>,
    /// Last known alert status per `rule_id|group_key` for fire/resolve transitions.
    alert_state: DashMap<String, AlertStatus>,
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
            board_lists: DashMap::new(),
            clipboard: RwLock::new(Vec::new()),
            alert_events: RwLock::new(VecDeque::with_capacity(500)),
            alert_state: DashMap::new(),
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
        let options = normalize_alert_options(req.options, &req.tags);
        // Keep only metric filter tags — strip monitor metadata keys if clients still send them in tags.
        let tags = metric_filter_tags(&req.tags);
        let rule = AlertRule {
            id: Uuid::new_v4(),
            name: req.name,
            metric: req.metric,
            tags,
            threshold: req.threshold,
            comparator: req.comparator,
            window_ms: req.window_ms,
            enabled: req.enabled,
            created_at: Utc::now(),
            options,
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
        let now = Utc::now();
        let board = DashboardBoard {
            id: Uuid::new_v4(),
            name: req.name,
            description: req.description,
            layout_type: req.layout_type,
            widgets: req.widgets,
            template_variables: req.template_variables,
            saved_views: Vec::new(),
            event_overlay: None,
            annotations: Vec::new(),
            share: None,
            tags: req.tags,
            author: Some("you".into()),
            created_at: now,
            updated_at: Some(now),
            deleted_at: None,
            recoverable_until: None,
        };
        self.boards.insert(board.id, board.clone());
        board
    }

    pub fn update_board(&self, id: Uuid, req: UpdateBoardRequest) -> Option<DashboardBoard> {
        let mut entry = self.boards.get_mut(&id)?;
        let board = entry.value_mut();
        if let Some(name) = req.name {
            board.name = name;
        }
        if let Some(description) = req.description {
            board.description = Some(description);
        }
        if let Some(layout_type) = req.layout_type {
            board.layout_type = layout_type;
        }
        if let Some(widgets) = req.widgets {
            board.widgets = widgets;
        }
        if let Some(template_variables) = req.template_variables {
            board.template_variables = template_variables;
        }
        if let Some(saved_views) = req.saved_views {
            board.saved_views = saved_views;
        }
        if let Some(event_overlay) = req.event_overlay {
            board.event_overlay = Some(event_overlay);
        }
        if let Some(annotations) = req.annotations {
            board.annotations = annotations;
        }
        if let Some(share) = req.share {
            board.share = Some(share);
        }
        if let Some(tags) = req.tags {
            board.tags = tags;
        }
        board.updated_at = Some(Utc::now());
        Some(board.clone())
    }

    pub fn list_boards(&self) -> Vec<DashboardBoard> {
        self.purge_expired_deleted();
        let mut boards: Vec<_> = self
            .boards
            .iter()
            .filter(|e| e.value().deleted_at.is_none())
            .map(|e| e.value().clone())
            .collect();
        boards.sort_by(|a, b| a.name.cmp(&b.name));
        boards
    }

    /// Recently Deleted — recoverable for 30 days (Datadog Dashboard List).
    pub fn list_deleted_boards(&self) -> Vec<DashboardBoard> {
        self.purge_expired_deleted();
        let mut boards: Vec<_> = self
            .boards
            .iter()
            .filter(|e| e.value().deleted_at.is_some())
            .map(|e| e.value().clone())
            .collect();
        boards.sort_by(|a, b| {
            b.recoverable_until
                .cmp(&a.recoverable_until)
                .then_with(|| a.name.cmp(&b.name))
        });
        boards
    }

    pub fn get_board(&self, id: Uuid) -> Option<DashboardBoard> {
        self.boards.get(&id).map(|e| e.value().clone())
    }

    /// Soft-delete; board appears in Recently Deleted for 30 days.
    pub fn delete_board(&self, id: Uuid) -> bool {
        let mut entry = match self.boards.get_mut(&id) {
            Some(e) => e,
            None => return false,
        };
        let board = entry.value_mut();
        if board.deleted_at.is_some() {
            return true;
        }
        let now = Utc::now();
        board.deleted_at = Some(now);
        board.recoverable_until = Some(now + chrono::Duration::days(30));
        board.updated_at = Some(now);
        true
    }

    /// Restore from Recently Deleted into a list (or active boards).
    pub fn restore_board(&self, id: Uuid, list_id: Option<&str>) -> Option<DashboardBoard> {
        let mut entry = self.boards.get_mut(&id)?;
        let board = entry.value_mut();
        board.deleted_at = None;
        board.recoverable_until = None;
        board.updated_at = Some(Utc::now());
        let restored = board.clone();
        drop(entry);
        if let Some(lid) = list_id {
            if let Some(mut list) = self.board_lists.get_mut(lid) {
                if !list.board_ids.contains(&id) {
                    list.board_ids.push(id);
                }
            }
        }
        Some(restored)
    }

    fn purge_expired_deleted(&self) {
        let now = Utc::now();
        let expired: Vec<Uuid> = self
            .boards
            .iter()
            .filter(|e| {
                e.value()
                    .recoverable_until
                    .map(|u| u < now)
                    .unwrap_or(false)
                    && e.value().deleted_at.is_some()
            })
            .map(|e| *e.key())
            .collect();
        for id in expired {
            self.boards.remove(&id);
        }
    }

    pub fn create_board_list(&self, name: String, board_ids: Vec<Uuid>) -> DashboardList {
        let list = DashboardList {
            id: Uuid::new_v4().to_string(),
            name,
            board_ids,
        };
        self.board_lists.insert(list.id.clone(), list.clone());
        list
    }

    pub fn list_board_lists(&self) -> Vec<DashboardList> {
        let mut lists: Vec<_> = self.board_lists.iter().map(|e| e.value().clone()).collect();
        lists.sort_by(|a, b| a.name.cmp(&b.name));
        lists
    }

    pub fn clipboard_set(&self, widgets: Vec<DashboardWidget>) {
        *self.clipboard.write() = widgets;
    }

    pub fn clipboard_get(&self) -> Vec<DashboardWidget> {
        self.clipboard.read().clone()
    }

    pub fn share_board(&self, id: Uuid, public: bool) -> Option<DashboardBoard> {
        let mut entry = self.boards.get_mut(&id)?;
        let board = entry.value_mut();
        let token = if public {
            Some(format!("share-{}", &Uuid::new_v4().to_string()[..8]))
        } else {
            None
        };
        board.share = Some(ShareConfig {
            public,
            token,
            refresh_secs: 30,
        });
        board.updated_at = Some(Utc::now());
        Some(board.clone())
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
    /// `template_selections` maps template variable names (no `$`) → values.
    pub fn render_board(
        &self,
        id: Uuid,
        range_ms: i64,
        filter_tags: &Tags,
        template_selections: &BTreeMap<String, String>,
    ) -> Result<RenderedBoard, ThineError> {
        let board = self
            .get_board(id)
            .ok_or_else(|| ThineError::NotFound(id.to_string()))?;
        let end = Utc::now().timestamp_millis();
        let start = end - range_ms.max(60_000);
        let step = ((range_ms / 60).max(5_000)).min(60_000);
        let prev_end = start;
        let prev_start = start - range_ms;

        let mut resolved_vars = BTreeMap::new();
        for tv in &board.template_variables {
            let val = template_selections
                .get(&tv.name)
                .cloned()
                .unwrap_or_else(|| tv.default.clone());
            resolved_vars.insert(tv.name.clone(), val);
        }
        // Also honor ad-hoc filter tags as template overrides.
        for (k, v) in filter_tags {
            resolved_vars.entry(k.clone()).or_insert_with(|| v.clone());
        }

        let public = board.share.as_ref().map(|s| s.public).unwrap_or(false);
        let refresh_secs = refresh_secs_for_range(range_ms, public);

        let mut tabs = BTreeSet::new();
        let mut widgets = Vec::new();
        for w in &board.widgets {
            if let Some(tab) = &w.tab {
                tabs.insert(tab.clone());
            }
            let mut tags = w.tags.clone();
            for (k, v) in filter_tags {
                tags.insert(k.clone(), v.clone());
            }
            // Apply template variables that this widget listens to (or all if empty).
            for tv in &board.template_variables {
                let listens = w.template_vars.is_empty() || w.template_vars.contains(&tv.name);
                if !listens {
                    continue;
                }
                let Some(val) = resolved_vars.get(&tv.name) else {
                    continue;
                };
                if val == "*" || val.is_empty() {
                    continue;
                }
                if tv.prefix == "groupby" {
                    // groupby vars don't filter; group_by on widget may already be set
                    continue;
                }
                tags.insert(tv.tag.clone(), val.clone());
            }

            let overlays = board.annotations.clone();

            match w.widget_type {
                WidgetType::Group | WidgetType::Note | WidgetType::EventStream => {
                    let mut text = w.text.clone().or_else(|| Some(w.title.clone()));
                    if let Some(t) = text.as_mut() {
                        *t = substitute_template_vars(t, &resolved_vars);
                    }
                    widgets.push(RenderedWidget {
                        id: w.id.clone(),
                        widget_type: w.widget_type,
                        title: substitute_template_vars(&w.title, &resolved_vars),
                        layout: w.layout.clone(),
                        unit: w.unit.clone(),
                        display: w.display.clone(),
                        text,
                        value: None,
                        previous_value: None,
                        change_pct: None,
                        sparkline: Vec::new(),
                        series: Vec::new(),
                        toplist: Vec::new(),
                        overlays: overlays.clone(),
                        tab: w.tab.clone(),
                        functions: w.functions.clone(),
                        anomalies: Vec::new(),
                        accent: w.accent.clone(),
                    });
                }
                WidgetType::QueryValue
                | WidgetType::Change
                | WidgetType::CheckStatus
                | WidgetType::Slo
                | WidgetType::AlertGraph => {
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

                    let mut sparkline = merge_series_avg(&current);
                    apply_functions_samples(&mut sparkline, &w.functions);
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
                        widget_type: if matches!(w.widget_type, WidgetType::Change) {
                            WidgetType::Change
                        } else {
                            WidgetType::QueryValue
                        },
                        title: substitute_template_vars(&w.title, &resolved_vars),
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
                        overlays: overlays.clone(),
                        tab: w.tab.clone(),
                        functions: w.functions.clone(),
                        anomalies: Vec::new(),
                        accent: w.accent.clone(),
                    });
                }
                WidgetType::Timeseries
                | WidgetType::Heatmap
                | WidgetType::Distribution
                | WidgetType::ScatterPlot
                | WidgetType::Hostmap => {
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
                    apply_functions_series(&mut series, &w.functions);
                    widgets.push(RenderedWidget {
                        id: w.id.clone(),
                        widget_type: w.widget_type,
                        title: substitute_template_vars(&w.title, &resolved_vars),
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
                        overlays: overlays.clone(),
                        tab: w.tab.clone(),
                        functions: w.functions.clone(),
                        anomalies: Vec::new(),
                        accent: w.accent.clone(),
                    });
                }
                WidgetType::Toplist
                | WidgetType::PieChart
                | WidgetType::Table
                | WidgetType::Funnel
                | WidgetType::ListStream => {
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
                                Aggregation::Last => {
                                    s.points.last().map(|p| p.value).unwrap_or(0.0)
                                }
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
                        widget_type: w.widget_type,
                        title: substitute_template_vars(&w.title, &resolved_vars),
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
                        overlays: overlays.clone(),
                        tab: w.tab.clone(),
                        functions: w.functions.clone(),
                        anomalies: Vec::new(),
                        accent: w.accent.clone(),
                    });
                }
            }
        }

        Ok(RenderedBoard {
            id: board.id,
            name: board.name,
            description: board.description,
            layout_type: board.layout_type,
            range_ms,
            start_ms: start,
            end_ms: end,
            refresh_secs,
            template_variables: board.template_variables,
            template_selections: resolved_vars,
            saved_views: board.saved_views,
            annotations: board.annotations,
            share: board.share,
            widgets,
            tabs: tabs.into_iter().collect(),
        })
    }

    fn evaluate_alerts(&self, metric: &str, tags: &Tags) {
        let now = Utc::now().timestamp_millis();
        for entry in self.alerts.iter() {
            let rule = entry.value();
            if !rule.enabled || rule.metric != metric {
                continue;
            }
            // Filter tags only — options never participate in series matching.
            if !tags_match(&rule.tags, tags) {
                continue;
            }
            // Multi-alert: evaluate the series that just ingested (group context).
            // Also evaluate sibling groups matching the same filter when group_by is set.
            let candidates = self.alert_candidate_series(metric, &rule.tags, rule.options.group_by.as_deref(), tags);
            for (group_key, series_tags, samples) in candidates {
                let Some((value, breached)) = evaluate_detection(rule, &samples, now) else {
                    continue;
                };
                let state_key = format!("{}|{}", rule.id, group_key);
                let prev = self
                    .alert_state
                    .get(&state_key)
                    .map(|s| *s)
                    .unwrap_or(AlertStatus::Resolved);

                let recovery = rule
                    .options
                    .recovery_threshold
                    .unwrap_or(rule.threshold);
                let recovered = if prev == AlertStatus::Firing {
                    !compare(value, recovery, rule.comparator)
                        || (!breached
                            && rule.options.detection_method != "threshold"
                            && rule.options.detection_method != "change")
                } else {
                    false
                };

                if breached && prev != AlertStatus::Firing {
                    debug!(rule = %rule.name, value, group = %group_key, "alert firing");
                    self.alert_state.insert(state_key.clone(), AlertStatus::Firing);
                    self.push_alert_event(AlertEvent {
                        id: Uuid::new_v4(),
                        rule_id: rule.id,
                        rule_name: substitute_group_template(&rule.name, &series_tags),
                        metric: metric.to_string(),
                        value,
                        threshold: rule.threshold,
                        fired_at: Utc::now(),
                        status: AlertStatus::Firing,
                        group_key: group_key.clone(),
                        group_tags: series_tags.clone(),
                        detection_method: rule.options.detection_method.clone(),
                    });
                } else if recovered {
                    debug!(rule = %rule.name, value, group = %group_key, "alert resolved");
                    self.alert_state.insert(state_key, AlertStatus::Resolved);
                    self.push_alert_event(AlertEvent {
                        id: Uuid::new_v4(),
                        rule_id: rule.id,
                        rule_name: substitute_group_template(&rule.name, &series_tags),
                        metric: metric.to_string(),
                        value,
                        threshold: recovery,
                        fired_at: Utc::now(),
                        status: AlertStatus::Resolved,
                        group_key,
                        group_tags: series_tags,
                        detection_method: rule.options.detection_method.clone(),
                    });
                }
            }
        }
    }

    fn push_alert_event(&self, event: AlertEvent) {
        let mut events = self.alert_events.write();
        events.push_back(event);
        while events.len() > 500 {
            events.pop_front();
        }
    }

    /// Series candidates for alert evaluation.
    fn alert_candidate_series(
        &self,
        metric: &str,
        filter: &Tags,
        group_by: Option<&str>,
        ingest_tags: &Tags,
    ) -> Vec<(String, Tags, Vec<Sample>)> {
        let mut out = Vec::new();
        for entry in self.series.iter() {
            let state = entry.value().read();
            if state.name != metric || !tags_match(filter, &state.tags) {
                continue;
            }
            if let Some(gb) = group_by {
                // Only evaluate the ingested group on this path for efficiency;
                // other groups are evaluated when their series ingest.
                if ingest_tags.get(gb) != state.tags.get(gb) {
                    continue;
                }
            } else if &state.tags != ingest_tags && !filter.is_empty() {
                // Simple alert: prefer exact series that just ingested.
                if state.tags != *ingest_tags {
                    continue;
                }
            } else if filter.is_empty() && state.tags != *ingest_tags {
                continue;
            }
            let group_key = group_by
                .and_then(|gb| state.tags.get(gb).map(|v| format!("{gb}:{v}")))
                .unwrap_or_default();
            out.push((group_key, state.tags.clone(), state.samples.iter().cloned().collect()));
        }
        if out.is_empty() {
            if let Some(series) = self.get_series(metric, ingest_tags) {
                out.push((String::new(), series.tags, series.samples));
            }
        }
        out
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

/// Datadog refresh-rate table (private dashboards); public always 30s.
fn refresh_secs_for_range(range_ms: i64, public: bool) -> u64 {
    if public {
        return 30;
    }
    match range_ms {
        r if r <= 10 * 60_000 => 10,
        r if r <= 60 * 60_000 => 20,
        r if r <= 4 * 60 * 60_000 => 60,
        r if r <= 24 * 60 * 60_000 => 180,
        r if r <= 2 * 24 * 60 * 60_000 => 600,
        _ => 3600,
    }
}

fn substitute_template_vars(text: &str, vars: &BTreeMap<String, String>) -> String {
    let mut out = text.to_string();
    for (name, val) in vars {
        out = out.replace(&format!("${name}"), val);
        out = out.replace(&format!("${{{name}}}"), val);
    }
    out
}

fn apply_functions_samples(samples: &mut Vec<Sample>, functions: &[String]) {
    for f in functions {
        let name = f.split('(').next().unwrap_or(f).trim().to_ascii_lowercase();
        match name.as_str() {
            "rate" | "diff" => {
                let mut prev = None;
                for s in samples.iter_mut() {
                    let cur = s.value;
                    s.value = prev.map(|p: f64| cur - p).unwrap_or(0.0);
                    prev = Some(cur);
                }
            }
            "cumsum" => {
                let mut acc = 0.0;
                for s in samples.iter_mut() {
                    acc += s.value;
                    s.value = acc;
                }
            }
            "abs" => {
                for s in samples.iter_mut() {
                    s.value = s.value.abs();
                }
            }
            "exclude_null" => {
                samples.retain(|s| s.value.is_finite());
            }
            _ => {}
        }
    }
}

fn apply_functions_series(series: &mut Vec<QueryResult>, functions: &[String]) {
    for s in series.iter_mut() {
        apply_functions_samples(&mut s.points, functions);
    }
    for f in functions {
        let name = f.split('(').next().unwrap_or(f).trim().to_ascii_lowercase();
        if name == "top" || name == "bottom" {
            let n = f
                .split(['(', ',', ')'])
                .nth(1)
                .and_then(|x| x.trim().parse::<usize>().ok())
                .unwrap_or(5);
            series.sort_by(|a, b| {
                let av = a.points.last().map(|p| p.value).unwrap_or(0.0);
                let bv = b.points.last().map(|p| p.value).unwrap_or(0.0);
                if name == "bottom" {
                    av.partial_cmp(&bv).unwrap_or(std::cmp::Ordering::Equal)
                } else {
                    bv.partial_cmp(&av).unwrap_or(std::cmp::Ordering::Equal)
                }
            });
            series.truncate(n);
        }
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

const MONITOR_META_KEYS: &[&str] = &[
    "severity",
    "warning_threshold",
    "recovery_threshold",
    "evaluate",
    "recipients",
    "detection_method",
    "change_type",
    "comparison_window_ms",
    "anomaly_direction",
    "forecast_horizon_ms",
    "message",
    "group_by",
];

fn metric_filter_tags(tags: &Tags) -> Tags {
    tags.iter()
        .filter(|(k, _)| !MONITOR_META_KEYS.contains(&k.as_str()))
        .map(|(k, v)| (k.clone(), v.clone()))
        .collect()
}

fn normalize_alert_options(mut opts: AlertOptions, legacy_tags: &Tags) -> AlertOptions {
    // Migrate legacy UI that stuffed options into tags.
    if opts.detection_method.is_empty() || opts.detection_method == "threshold" {
        if let Some(m) = legacy_tags.get("detection_method") {
            opts.detection_method = m.clone();
        }
    }
    if opts.recipients.is_empty() {
        if let Some(r) = legacy_tags.get("recipients") {
            opts.recipients = r.clone();
        }
    }
    if opts.severity.is_empty() || opts.severity == "critical" {
        if let Some(s) = legacy_tags.get("severity") {
            opts.severity = s.clone();
        }
    }
    if opts.evaluate.is_empty() || opts.evaluate == "avg" {
        if let Some(e) = legacy_tags.get("evaluate") {
            opts.evaluate = e.clone();
        }
    }
    if opts.change_type.is_empty() {
        if let Some(c) = legacy_tags.get("change_type") {
            opts.change_type = c.clone();
        }
    }
    if opts.comparison_window_ms == 0 {
        if let Some(w) = legacy_tags.get("comparison_window_ms").and_then(|s| s.parse().ok()) {
            opts.comparison_window_ms = w;
        }
    }
    if opts.anomaly_direction.is_empty() {
        if let Some(d) = legacy_tags.get("anomaly_direction") {
            opts.anomaly_direction = d.clone();
        }
    }
    if opts.forecast_horizon_ms == 0 {
        if let Some(h) = legacy_tags.get("forecast_horizon_ms").and_then(|s| s.parse().ok()) {
            opts.forecast_horizon_ms = h;
        }
    }
    if opts.warning_threshold.is_none() {
        opts.warning_threshold = legacy_tags
            .get("warning_threshold")
            .and_then(|s| s.parse().ok());
    }
    if opts.recovery_threshold.is_none() {
        opts.recovery_threshold = legacy_tags
            .get("recovery_threshold")
            .and_then(|s| s.parse().ok());
    }
    if opts.message.is_empty() {
        if let Some(m) = legacy_tags.get("message") {
            opts.message = m.clone();
        }
    }
    if opts.group_by.is_none() {
        opts.group_by = legacy_tags.get("group_by").cloned();
    }
    if opts.detection_method.is_empty() {
        opts.detection_method = "threshold".into();
    }
    if opts.evaluate.is_empty() {
        opts.evaluate = "avg".into();
    }
    if opts.comparison_window_ms <= 0 {
        opts.comparison_window_ms = 30 * 60_000;
    }
    if opts.forecast_horizon_ms <= 0 {
        opts.forecast_horizon_ms = 24 * 60 * 60_000;
    }
    if opts.anomaly_direction.is_empty() {
        opts.anomaly_direction = "above_or_below".into();
    }
    if opts.change_type.is_empty() {
        opts.change_type = "pct_change".into();
    }
    opts
}

fn substitute_group_template(name: &str, tags: &Tags) -> String {
    let mut out = name.to_string();
    for (k, v) in tags {
        out = out.replace(&format!("{{{{{k}.name}}}}"), v);
        out = out.replace(&format!("{{{{{k}}}}}"), v);
    }
    out
}

fn aggregate_values(values: &[f64], how: &str) -> f64 {
    if values.is_empty() {
        return 0.0;
    }
    match how {
        "max" => values.iter().cloned().fold(f64::NEG_INFINITY, f64::max),
        "min" => values.iter().cloned().fold(f64::INFINITY, f64::min),
        "sum" => values.iter().sum(),
        _ => values.iter().sum::<f64>() / values.len() as f64,
    }
}

/// Median + median absolute deviation — robust seasonal baseline for anomaly monitors.
fn median_mad(values: &[f64]) -> (f64, f64) {
    if values.is_empty() {
        return (0.0, 1e-9);
    }
    let mut sorted = values.to_vec();
    sorted.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    let mid = sorted.len() / 2;
    let med = if sorted.len() % 2 == 0 {
        (sorted[mid - 1] + sorted[mid]) / 2.0
    } else {
        sorted[mid]
    };
    let mut devs: Vec<f64> = sorted.iter().map(|v| (v - med).abs()).collect();
    devs.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    let mad = if devs.len() % 2 == 0 {
        (devs[devs.len() / 2 - 1] + devs[devs.len() / 2]) / 2.0
    } else {
        devs[devs.len() / 2]
    }
    .max(1e-9);
    (med, mad)
}

/// Returns (evaluated_value, breached_critical).
fn evaluate_detection(rule: &AlertRule, samples: &[Sample], now: i64) -> Option<(f64, bool)> {
    let method = rule.options.detection_method.as_str();
    let window = rule.window_ms.max(1_000);
    let eval = rule.options.evaluate.as_str();

    match method {
        "change" => {
            let cur_start = now - window;
            let shift = rule.options.comparison_window_ms.max(1_000);
            let prev_start = cur_start - shift;
            let prev_end = now - shift;
            let cur: Vec<f64> = samples
                .iter()
                .filter(|s| s.timestamp_ms >= cur_start && s.timestamp_ms <= now)
                .map(|s| s.value)
                .collect();
            let prev: Vec<f64> = samples
                .iter()
                .filter(|s| s.timestamp_ms >= prev_start && s.timestamp_ms <= prev_end)
                .map(|s| s.value)
                .collect();
            if cur.is_empty() || prev.is_empty() {
                return None;
            }
            let a = aggregate_values(&cur, eval);
            let b = aggregate_values(&prev, eval);
            let value = if rule.options.change_type == "change" {
                a - b
            } else if b.abs() < f64::EPSILON {
                0.0
            } else {
                ((a - b) / b) * 100.0
            };
            Some((value, compare(value, rule.threshold, rule.comparator)))
        }
        "anomaly" => {
            // Datadog-shaped seasonal anomaly: hour-of-day baselines + robust MAD bounds.
            // Falls back to global MAD when a bucket is thin.
            let hist_start = now - window.max(3_600_000) * 24; // up to ~1 day×24 of history
            let alert_start = now - window;
            let hist: Vec<&Sample> = samples
                .iter()
                .filter(|s| s.timestamp_ms >= hist_start && s.timestamp_ms < alert_start)
                .collect();
            let cur: Vec<&Sample> = samples
                .iter()
                .filter(|s| s.timestamp_ms >= alert_start && s.timestamp_ms <= now)
                .collect();
            if hist.len() < 8 || cur.is_empty() {
                return None;
            }
            let mut by_hour: [Vec<f64>; 24] = std::array::from_fn(|_| Vec::new());
            for s in &hist {
                let hour = ((s.timestamp_ms / 3_600_000) % 24) as usize;
                by_hour[hour].push(s.value);
            }
            let global: Vec<f64> = hist.iter().map(|s| s.value).collect();
            let (g_med, g_mad) = median_mad(&global);
            let mut anomalous = 0usize;
            for s in &cur {
                let hour = ((s.timestamp_ms / 3_600_000) % 24) as usize;
                let (med, mad) = if by_hour[hour].len() >= 3 {
                    median_mad(&by_hour[hour])
                } else {
                    (g_med, g_mad)
                };
                let scale = (mad * 1.4826).max(1e-9); // MAD→σ
                let z = (s.value - med) / scale;
                let is_anom = match rule.options.anomaly_direction.as_str() {
                    "above" => z > 3.0,
                    "below" => z < -3.0,
                    _ => z.abs() > 3.0,
                };
                if is_anom {
                    anomalous += 1;
                }
            }
            let fraction = anomalous as f64 / cur.len() as f64;
            Some((fraction, fraction >= rule.threshold))
        }
        "forecast" => {
            // Holt linear (double exponential) forecast — Datadog-like trend projection
            // with residual-based confidence bands (not plain OLS).
            let hist_start = now - window.max(3_600_000) * 24;
            let mut hist: Vec<&Sample> = samples
                .iter()
                .filter(|s| s.timestamp_ms >= hist_start && s.timestamp_ms <= now)
                .collect();
            hist.sort_by_key(|s| s.timestamp_ms);
            if hist.len() < 6 {
                return None;
            }
            let alpha = 0.35_f64;
            let beta = 0.15_f64;
            let mut level = hist[0].value;
            let mut trend = hist[1].value - hist[0].value;
            let mut fitted = Vec::with_capacity(hist.len());
            fitted.push(level);
            for s in hist.iter().skip(1) {
                let prev_level = level;
                level = alpha * s.value + (1.0 - alpha) * (level + trend);
                trend = beta * (level - prev_level) + (1.0 - beta) * trend;
                fitted.push(level + trend);
            }
            let residuals: Vec<f64> = hist
                .iter()
                .zip(fitted.iter())
                .map(|(s, f)| s.value - f)
                .collect();
            let std = (residuals.iter().map(|r| r.powi(2)).sum::<f64>() / residuals.len() as f64)
                .sqrt()
                .max(1e-9);
            // Project steps = horizon / median sample interval (fallback 1m).
            let mut gaps = Vec::new();
            for w in hist.windows(2) {
                gaps.push((w[1].timestamp_ms - w[0].timestamp_ms).max(1) as f64);
            }
            gaps.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
            let step_ms = gaps.get(gaps.len() / 2).copied().unwrap_or(60_000.0).max(1_000.0);
            let steps = (rule.options.forecast_horizon_ms as f64 / step_ms).max(1.0);
            let forecast = level + trend * steps;
            // Widen band with √steps (random-walk-ish uncertainty growth).
            let band = 1.96 * std * steps.sqrt();
            let upper = forecast + band;
            let lower = forecast - band;
            let breached = match rule.comparator {
                Comparator::Gt | Comparator::Gte => upper >= rule.threshold,
                Comparator::Lt | Comparator::Lte => lower <= rule.threshold,
                Comparator::Eq => (forecast - rule.threshold).abs() < band,
            };
            Some((forecast, breached))
        }
        _ => {
            // threshold (default)
            let window_start = now - window;
            let values: Vec<f64> = samples
                .iter()
                .filter(|s| s.timestamp_ms >= window_start)
                .map(|s| s.value)
                .collect();
            if values.is_empty() {
                return None;
            }
            let value = aggregate_values(&values, eval);
            let mut breached = compare(value, rule.threshold, rule.comparator);
            if let Some(warn) = rule.options.warning_threshold {
                // Warning still counts as a breach for state machine (severity in options).
                if !breached && compare(value, warn, rule.comparator) {
                    breached = true;
                }
            }
            Some((value, breached))
        }
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
