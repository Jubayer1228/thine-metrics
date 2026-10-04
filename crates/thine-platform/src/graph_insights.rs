//! Datadog Graph Insights: Metric Correlations, Watchdog Explains, dashboard anomalies.
//! Docs: https://docs.datadoghq.com/dashboards/graph_insights/

use chrono::Utc;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::collections::{BTreeMap, BTreeSet, HashMap};
use uuid::Uuid;

use crate::state::PlatformState;
use thine_common::{
    Aggregation, GraphAnomalyRegion, QueryRequest, Sample, TagContribution, Tags, WidgetType,
};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CorrelationSearchRequest {
    pub metric: String,
    #[serde(default)]
    pub tags: Tags,
    pub start_ms: i64,
    pub end_ms: i64,
    /// Area of interest (pink box); defaults to last 15% of the window.
    #[serde(default)]
    pub interest_start_ms: Option<i64>,
    #[serde(default)]
    pub interest_end_ms: Option<i64>,
    /// apm | integrations | dashboards | custom
    #[serde(default = "default_sources")]
    pub sources: Vec<String>,
    #[serde(default)]
    pub namespaces: Vec<String>,
    #[serde(default)]
    pub tag_filter: Tags,
    #[serde(default = "default_limit")]
    pub limit: usize,
}

fn default_sources() -> Vec<String> {
    vec![
        "apm".into(),
        "integrations".into(),
        "dashboards".into(),
    ]
}
fn default_limit() -> usize {
    25
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CorrelationHit {
    pub source_type: String,
    pub source: String,
    pub metric: String,
    pub tags: Tags,
    pub score: f64,
    pub z_score: f64,
    pub correlations: usize,
    pub preview: Vec<Sample>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CorrelationSearchResponse {
    pub metric: String,
    pub interest_start_ms: i64,
    pub interest_end_ms: i64,
    pub results: Vec<CorrelationGroup>,
    pub hits: Vec<CorrelationHit>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CorrelationGroup {
    pub source_type: String,
    pub source: String,
    pub correlations: usize,
    pub preview: Vec<Sample>,
    pub metrics: Vec<CorrelationHit>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WatchdogExplainRequest {
    pub metric: String,
    #[serde(default)]
    pub tags: Tags,
    pub start_ms: i64,
    pub end_ms: i64,
    #[serde(default)]
    pub group_keys: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WatchdogExplainResult {
    pub metric: String,
    pub anomaly: Option<GraphAnomalyRegion>,
    pub affects_everyone: bool,
    pub influential_tags: Vec<TagContribution>,
    pub findings: Vec<ExplainFinding>,
    pub summary: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExplainFinding {
    pub tag_key: String,
    pub tag_value: String,
    pub contribution: f64,
    pub message: String,
    pub with_tag_peak: f64,
    pub without_tag_peak: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DashboardAnomalyIssue {
    pub id: String,
    pub title: String,
    pub metric: String,
    pub widget_ids: Vec<String>,
    pub widget_titles: Vec<String>,
    pub detected_at_ms: i64,
    pub anomaly: GraphAnomalyRegion,
    pub influential_tags: Vec<TagContribution>,
    pub co_occurring_metrics: Vec<String>,
    pub next_steps: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DashboardAnomaliesResponse {
    pub board_id: String,
    pub board_name: String,
    pub auto_detect: bool,
    pub issues: Vec<DashboardAnomalyIssue>,
    pub scanned_widgets: usize,
    pub elapsed_hint_ms: u64,
}

impl PlatformState {
    /// Metric Correlations — find metrics with irregular behavior in the same window.
    pub fn graph_correlations(&self, req: CorrelationSearchRequest) -> CorrelationSearchResponse {
        let end = req.end_ms;
        let start = req.start_ms.min(end - 60_000);
        let span = (end - start).max(60_000);
        let interest_end = req.interest_end_ms.unwrap_or(end);
        let interest_start = req
            .interest_start_ms
            .unwrap_or(end - (span as f64 * 0.15) as i64)
            .max(start);

        let primary = self
            .metrics
            .query(QueryRequest {
                metric: req.metric.clone(),
                tags: req.tags.clone(),
                start_ms: Some(start),
                end_ms: Some(end),
                step_ms: ((span / 60).max(5_000)).min(60_000),
                aggregation: Aggregation::Avg,
            })
            .unwrap_or_default();
        let primary_pts = primary
            .first()
            .map(|s| s.points.as_slice())
            .unwrap_or(&[]);

        let board_metrics: BTreeSet<String> = self
            .metrics
            .list_boards()
            .into_iter()
            .flat_map(|b| b.widgets.into_iter().map(|w| w.metric))
            .filter(|m| !m.is_empty())
            .collect();

        let mut hits = Vec::new();
        let metas = self.metrics.list_metrics(None);
        let mut seen = BTreeSet::new();

        for meta in metas {
            if meta.name == req.metric {
                continue;
            }
            if !req.tag_filter.is_empty() && !tags_subset(&req.tag_filter, &meta.tags) {
                continue;
            }
            let source_type = classify_source(&meta.name, board_metrics.contains(&meta.name));
            if !req.sources.is_empty() && !req.sources.iter().any(|s| s == &source_type) {
                continue;
            }
            if source_type == "custom" {
                if req.namespaces.is_empty() {
                    continue;
                }
                if !req.namespaces.iter().any(|ns| {
                    meta.name == *ns || meta.name.starts_with(&format!("{ns}."))
                }) {
                    continue;
                }
            }

            let key = format!("{}|{:?}", meta.name, meta.tags);
            if !seen.insert(key) {
                continue;
            }

            let series = self
                .metrics
                .query(QueryRequest {
                    metric: meta.name.clone(),
                    tags: meta.tags.clone(),
                    start_ms: Some(start),
                    end_ms: Some(end),
                    step_ms: ((span / 60).max(5_000)).min(60_000),
                    aggregation: Aggregation::Avg,
                })
                .unwrap_or_default();
            let Some(s) = series.first() else {
                continue;
            };
            if s.points.len() < 4 {
                continue;
            }

            let (z, interest_mean, baseline) =
                window_z_score(&s.points, interest_start, interest_end);
            if z.abs() < 2.0 {
                continue;
            }
            let corr = pearson_aligned(primary_pts, &s.points);
            let score = (z.abs() / 5.0).min(1.0) * 0.65 + corr.abs() * 0.35;
            if score < 0.25 {
                continue;
            }

            hits.push(CorrelationHit {
                source_type: source_type.clone(),
                source: source_label(&meta.name, &source_type, &board_metrics),
                metric: meta.name.clone(),
                tags: s.tags.clone(),
                score,
                z_score: z,
                correlations: 1,
                preview: s.points.clone(),
            });
            let _ = (interest_mean, baseline);
        }

        hits.sort_by(|a, b| {
            b.score
                .partial_cmp(&a.score)
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        hits.truncate(req.limit);

        // Group by source for Datadog-style results list
        let mut groups: BTreeMap<(String, String), Vec<CorrelationHit>> = BTreeMap::new();
        for h in &hits {
            groups
                .entry((h.source_type.clone(), h.source.clone()))
                .or_default()
                .push(h.clone());
        }
        let mut results: Vec<CorrelationGroup> = groups
            .into_iter()
            .map(|((source_type, source), metrics)| {
                let preview = metrics
                    .first()
                    .map(|m| m.preview.clone())
                    .unwrap_or_default();
                CorrelationGroup {
                    source_type,
                    source,
                    correlations: metrics.len(),
                    preview,
                    metrics,
                }
            })
            .collect();
        results.sort_by(|a, b| b.correlations.cmp(&a.correlations));

        CorrelationSearchResponse {
            metric: req.metric,
            interest_start_ms: interest_start,
            interest_end_ms: interest_end,
            results,
            hits,
        }
    }

    /// Watchdog Explains — dimensional analysis of a spike.
    pub fn graph_watchdog_explain(&self, req: WatchdogExplainRequest) -> WatchdogExplainResult {
        let end = req.end_ms;
        let start = req.start_ms.min(end - 60_000);
        let span = (end - start).max(60_000);
        let step = ((span / 60).max(5_000)).min(60_000);

        let series = self
            .metrics
            .query(QueryRequest {
                metric: req.metric.clone(),
                tags: req.tags.clone(),
                start_ms: Some(start),
                end_ms: Some(end),
                step_ms: step,
                aggregation: Aggregation::Avg,
            })
            .unwrap_or_default();

        let merged = merge_avg(
            &series
                .iter()
                .flat_map(|s| s.points.iter().cloned())
                .collect::<Vec<_>>(),
        );
        let anomaly = detect_anomaly_region(&merged);

        let group_keys = if req.group_keys.is_empty() {
            vec![
                "service".into(),
                "env".into(),
                "gpu_id".into(),
                "host".into(),
            ]
        } else {
            req.group_keys.clone()
        };

        // Expand by each tag key present in series
        let mut findings = Vec::new();
        let all_series = self
            .metrics
            .query(QueryRequest {
                metric: req.metric.clone(),
                tags: Tags::new(),
                start_ms: Some(start),
                end_ms: Some(end),
                step_ms: step,
                aggregation: Aggregation::Avg,
            })
            .unwrap_or_default();

        let global_peak = peak_in_region(&merged, anomaly.as_ref());

        for key in &group_keys {
            let mut by_val: HashMap<String, Vec<Sample>> = HashMap::new();
            for s in &all_series {
                if let Some(v) = s.tags.get(key) {
                    by_val.entry(v.clone()).or_default().extend(s.points.clone());
                }
            }
            if by_val.len() < 2 {
                continue;
            }
            let mut contribs: Vec<(String, f64, f64, f64)> = Vec::new();
            for (val, pts) in &by_val {
                let m = merge_avg(pts);
                let peak = peak_in_region(&m, anomaly.as_ref());
                // Approximate "without" as global - this series contribution
                let without = (global_peak - peak).max(0.0);
                let contribution = if global_peak > 1e-9 {
                    (peak / global_peak).clamp(0.0, 1.0)
                } else {
                    0.0
                };
                contribs.push((val.clone(), contribution, peak, without));
            }
            contribs.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
            if let Some((val, contribution, with_peak, without_peak)) = contribs.first() {
                if *contribution >= 0.35 {
                    findings.push(ExplainFinding {
                        tag_key: key.clone(),
                        tag_value: val.clone(),
                        contribution: *contribution,
                        message: format!(
                            "Isolating {key}:{val} accounts for {:.0}% of the spike",
                            contribution * 100.0
                        ),
                        with_tag_peak: *with_peak,
                        without_tag_peak: *without_peak,
                    });
                }
            }
        }
        findings.sort_by(|a, b| {
            b.contribution
                .partial_cmp(&a.contribution)
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        findings.truncate(8);

        let influential_tags: Vec<TagContribution> = findings
            .iter()
            .map(|f| TagContribution {
                key: f.tag_key.clone(),
                value: f.tag_value.clone(),
                contribution: f.contribution,
                message: f.message.clone(),
            })
            .collect();

        let affects_everyone = findings.first().map(|f| f.contribution < 0.55).unwrap_or(true);
        let summary = if let Some(a) = &anomaly {
            if let Some(f) = findings.first() {
                format!(
                    "{}. Primary driver: {}:{} ({:.0}% of anomaly).",
                    a.summary,
                    f.tag_key,
                    f.tag_value,
                    f.contribution * 100.0
                )
            } else {
                a.summary.clone()
            }
        } else {
            "No significant anomaly detected in the selected window.".into()
        };

        let mut anomaly = anomaly;
        if let Some(ref mut a) = anomaly {
            a.influential_tags = influential_tags.clone();
        }

        WatchdogExplainResult {
            metric: req.metric,
            anomaly,
            affects_everyone,
            influential_tags,
            findings,
            summary,
        }
    }

    /// Dashboard anomaly detection — group co-occurring widget anomalies into issues.
    pub fn board_anomalies(
        &self,
        board_id: Uuid,
        range_ms: i64,
        auto_detect: bool,
    ) -> Option<DashboardAnomaliesResponse> {
        let board = self.metrics.get_board(board_id)?;
        if board.deleted_at.is_some() {
            return None;
        }
        // Public shared boards skip detection (Datadog docs).
        if board.share.as_ref().map(|s| s.public).unwrap_or(false) {
            return Some(DashboardAnomaliesResponse {
                board_id: board_id.to_string(),
                board_name: board.name,
                auto_detect: false,
                issues: Vec::new(),
                scanned_widgets: 0,
                elapsed_hint_ms: 0,
            });
        }
        if !auto_detect {
            return Some(DashboardAnomaliesResponse {
                board_id: board_id.to_string(),
                board_name: board.name,
                auto_detect: false,
                issues: Vec::new(),
                scanned_widgets: 0,
                elapsed_hint_ms: 0,
            });
        }

        let end = Utc::now().timestamp_millis();
        let start = end - range_ms.max(60_000);
        let step = ((range_ms / 60).max(5_000)).min(60_000);
        let mut per_widget: Vec<(String, String, String, GraphAnomalyRegion)> = Vec::new();

        for w in &board.widgets {
            if w.hide_anomaly_detection {
                continue;
            }
            if !matches!(
                w.widget_type,
                WidgetType::Timeseries
                    | WidgetType::QueryValue
                    | WidgetType::Heatmap
                    | WidgetType::Distribution
            ) {
                continue;
            }
            if w.metric.is_empty() {
                continue;
            }
            // Skip widgets using cumsum/anomalies/outliers functions
            if w.functions.iter().any(|f| {
                let n = f.to_ascii_lowercase();
                n.contains("cumsum") || n.contains("anomal") || n.contains("outlier")
            }) {
                continue;
            }
            let series = self
                .metrics
                .query(QueryRequest {
                    metric: w.metric.clone(),
                    tags: w.tags.clone(),
                    start_ms: Some(start),
                    end_ms: Some(end),
                    step_ms: step,
                    aggregation: w.aggregation,
                })
                .unwrap_or_default();
            let merged = merge_avg(
                &series
                    .iter()
                    .flat_map(|s| s.points.iter().cloned())
                    .collect::<Vec<_>>(),
            );
            if let Some(mut region) = detect_anomaly_region(&merged) {
                // Dimensional tags for the issue chip
                let explain = self.graph_watchdog_explain(WatchdogExplainRequest {
                    metric: w.metric.clone(),
                    tags: w.tags.clone(),
                    start_ms: start,
                    end_ms: end,
                    group_keys: w
                        .group_by
                        .clone()
                        .into_iter()
                        .collect::<Vec<_>>()
                        .into_iter()
                        .chain(["service".into(), "env".into(), "gpu_id".into()])
                        .collect(),
                });
                region.influential_tags = explain.influential_tags;
                per_widget.push((w.id.clone(), w.title.clone(), w.metric.clone(), region));
            }
        }

        // Group co-occurring anomalies (overlapping windows)
        let mut used = vec![false; per_widget.len()];
        let mut issues = Vec::new();
        for i in 0..per_widget.len() {
            if used[i] {
                continue;
            }
            used[i] = true;
            let mut widget_ids = vec![per_widget[i].0.clone()];
            let mut widget_titles = vec![per_widget[i].1.clone()];
            let mut metrics = vec![per_widget[i].2.clone()];
            let mut region = per_widget[i].3.clone();
            for j in (i + 1)..per_widget.len() {
                if used[j] {
                    continue;
                }
                if windows_overlap(
                    region.start_ms,
                    region.end_ms,
                    per_widget[j].3.start_ms,
                    per_widget[j].3.end_ms,
                ) {
                    used[j] = true;
                    widget_ids.push(per_widget[j].0.clone());
                    widget_titles.push(per_widget[j].1.clone());
                    metrics.push(per_widget[j].2.clone());
                    // Prefer higher deviation region
                    if per_widget[j].3.deviation_pct > region.deviation_pct {
                        region = per_widget[j].3.clone();
                    }
                }
            }
            metrics.sort();
            metrics.dedup();
            let title = if widget_ids.len() > 1 {
                format!("Anomalies co-occur on {} widgets", widget_ids.len())
            } else {
                format!(
                    "{} — {}",
                    metrics.first().cloned().unwrap_or_default(),
                    region.summary
                )
            };
            let next_steps = vec![
                "Open Watchdog Explains on the affected graph".into(),
                "Create a monitor for this metric".into(),
                "Investigate with Bits AI".into(),
            ];
            issues.push(DashboardAnomalyIssue {
                id: Uuid::new_v4().to_string(),
                title,
                metric: metrics.first().cloned().unwrap_or_default(),
                widget_ids,
                widget_titles,
                detected_at_ms: Utc::now().timestamp_millis(),
                influential_tags: region.influential_tags.clone(),
                anomaly: region,
                co_occurring_metrics: metrics,
                next_steps,
            });
        }

        issues.sort_by(|a, b| {
            b.anomaly
                .deviation_pct
                .partial_cmp(&a.anomaly.deviation_pct)
                .unwrap_or(std::cmp::Ordering::Equal)
        });

        Some(DashboardAnomaliesResponse {
            board_id: board_id.to_string(),
            board_name: board.name,
            auto_detect: true,
            scanned_widgets: per_widget.len(),
            issues,
            elapsed_hint_ms: 20_000,
        })
    }

    pub fn graph_insights_guide(&self) -> serde_json::Value {
        json!({
            "title": "Graph Insights",
            "docs": "https://docs.datadoghq.com/dashboards/graph_insights/",
            "features": [
                {
                    "id": "metric_correlations",
                    "path": "/api/v1/graph_insights/correlations",
                    "desc": "Find metrics with irregular behavior around the same time"
                },
                {
                    "id": "watchdog_explains",
                    "path": "/api/v1/graph_insights/explain",
                    "desc": "Dimensional analysis — which tags drive the spike?"
                },
                {
                    "id": "dashboard_anomaly_detection",
                    "path": "/api/v1/boards/{id}/anomalies",
                    "desc": "Group co-occurring graph anomalies into Investigate issues"
                }
            ]
        })
    }
}

fn classify_source(metric: &str, on_dashboard: bool) -> String {
    if metric.starts_with("http.")
        || metric.starts_with("rpc.")
        || metric.starts_with("trace.")
        || metric.contains("apm")
    {
        return "apm".into();
    }
    if metric.starts_with("gpu.")
        || metric.starts_with("system.")
        || metric.starts_with("process.")
        || metric.starts_with("container.")
        || metric.starts_with("aws.")
        || metric.starts_with("kubernetes.")
    {
        return "integrations".into();
    }
    if on_dashboard {
        return "dashboards".into();
    }
    "custom".into()
}

fn source_label(metric: &str, source_type: &str, board_metrics: &BTreeSet<String>) -> String {
    match source_type {
        "apm" => metric.split('.').take(2).collect::<Vec<_>>().join("."),
        "integrations" => metric.split('.').next().unwrap_or(metric).to_string(),
        "dashboards" if board_metrics.contains(metric) => "Dashboard widgets".into(),
        _ => metric.to_string(),
    }
}

fn tags_subset(filter: &Tags, actual: &Tags) -> bool {
    filter
        .iter()
        .all(|(k, v)| actual.get(k).map(|av| av == v).unwrap_or(false))
}

fn window_z_score(points: &[Sample], interest_start: i64, interest_end: i64) -> (f64, f64, f64) {
    let interest: Vec<f64> = points
        .iter()
        .filter(|p| p.timestamp_ms >= interest_start && p.timestamp_ms <= interest_end)
        .map(|p| p.value)
        .collect();
    let baseline: Vec<f64> = points
        .iter()
        .filter(|p| p.timestamp_ms < interest_start || p.timestamp_ms > interest_end)
        .map(|p| p.value)
        .collect();
    if interest.is_empty() || baseline.len() < 2 {
        return (0.0, 0.0, 0.0);
    }
    let i_mean = interest.iter().sum::<f64>() / interest.len() as f64;
    let (b_mean, b_std) = mean_std(&baseline);
    let z = if b_std > 1e-9 {
        (i_mean - b_mean) / b_std
    } else {
        0.0
    };
    (z, i_mean, b_mean)
}

fn mean_std(vals: &[f64]) -> (f64, f64) {
    if vals.is_empty() {
        return (0.0, 0.0);
    }
    let mean = vals.iter().sum::<f64>() / vals.len() as f64;
    let var = vals.iter().map(|v| (v - mean).powi(2)).sum::<f64>() / vals.len() as f64;
    (mean, var.sqrt())
}

fn pearson_aligned(a: &[Sample], b: &[Sample]) -> f64 {
    if a.is_empty() || b.is_empty() {
        return 0.0;
    }
    let mut map: BTreeMap<i64, f64> = BTreeMap::new();
    for p in a {
        map.insert(p.timestamp_ms, p.value);
    }
    let mut xs = Vec::new();
    let mut ys = Vec::new();
    for p in b {
        if let Some(&x) = map.get(&p.timestamp_ms) {
            xs.push(x);
            ys.push(p.value);
        }
    }
    if xs.len() < 3 {
        return 0.0;
    }
    let (mx, sx) = mean_std(&xs);
    let (my, sy) = mean_std(&ys);
    if sx < 1e-9 || sy < 1e-9 {
        return 0.0;
    }
    let cov: f64 = xs
        .iter()
        .zip(ys.iter())
        .map(|(x, y)| (x - mx) * (y - my))
        .sum::<f64>()
        / xs.len() as f64;
    (cov / (sx * sy)).clamp(-1.0, 1.0)
}

fn merge_avg(points: &[Sample]) -> Vec<Sample> {
    let mut buckets: BTreeMap<i64, Vec<f64>> = BTreeMap::new();
    for p in points {
        buckets.entry(p.timestamp_ms).or_default().push(p.value);
    }
    buckets
        .into_iter()
        .map(|(ts, vals)| Sample {
            timestamp_ms: ts,
            value: vals.iter().sum::<f64>() / vals.len() as f64,
        })
        .collect()
}

fn detect_anomaly_region(points: &[Sample]) -> Option<GraphAnomalyRegion> {
    if points.len() < 8 {
        return None;
    }
    let vals: Vec<f64> = points.iter().map(|p| p.value).collect();
    let (mean, std) = mean_std(&vals);
    if std < 1e-9 {
        return None;
    }
    // Find max |z| point and expand a window around it
    let mut best_i = 0usize;
    let mut best_z: f64 = 0.0;
    for (i, v) in vals.iter().enumerate() {
        let z = (v - mean) / std;
        if z.abs() > best_z.abs() {
            best_z = z;
            best_i = i;
        }
    }
    if best_z.abs() < 2.5 {
        return None;
    }
    let left = best_i.saturating_sub(2);
    let right = (best_i + 2).min(points.len() - 1);
    let expected = mean;
    let observed = vals[best_i];
    let deviation_pct = if expected.abs() > 1e-9 {
        ((observed - expected) / expected.abs()) * 100.0
    } else {
        best_z.abs() * 20.0
    };
    let kind = if best_z > 0.0 { "spike" } else { "dip" };
    let severity = if best_z.abs() >= 4.5 {
        "critical"
    } else if best_z.abs() >= 3.5 {
        "high"
    } else {
        "medium"
    };
    Some(GraphAnomalyRegion {
        start_ms: points[left].timestamp_ms,
        end_ms: points[right].timestamp_ms,
        severity: severity.into(),
        summary: format!(
            "The {kind} is {:.0}% {} than the expected range as inferred from recent data",
            deviation_pct.abs(),
            if best_z > 0.0 { "higher" } else { "lower" }
        ),
        deviation_pct,
        z_score: best_z,
        influential_tags: Vec::new(),
    })
}

fn peak_in_region(points: &[Sample], region: Option<&GraphAnomalyRegion>) -> f64 {
    let iter = points.iter().filter(|p| {
        region
            .map(|r| p.timestamp_ms >= r.start_ms && p.timestamp_ms <= r.end_ms)
            .unwrap_or(true)
    });
    iter.map(|p| p.value)
        .fold(0.0_f64, |a, b| a.max(b))
}

fn windows_overlap(a0: i64, a1: i64, b0: i64, b1: i64) -> bool {
    a0 <= b1 && b0 <= a1
}
