//! Protocol adapters: OTLP/HTTP JSON, Prometheus remote-write-ish JSON, StatsD-like lines.

use anyhow::{anyhow, Result};
use chrono::Utc;
use serde::Deserialize;
use serde_json::Value;
use std::collections::BTreeMap;
use std::sync::Arc;
use thine_common::{IngestStats, MetricPoint, MetricType, Sample, Tags};
use thine_storage::MetricStore;
use tracing::warn;

/// OTLP metrics ExportMetricsServiceRequest (JSON mapping, subset).
#[derive(Debug, Clone, Deserialize)]
pub struct OtlpExportRequest {
    #[serde(default, rename = "resourceMetrics")]
    pub resource_metrics: Vec<OtlpResourceMetrics>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct OtlpResourceMetrics {
    #[serde(default)]
    pub resource: Option<OtlpResource>,
    #[serde(default, rename = "scopeMetrics")]
    pub scope_metrics: Vec<OtlpScopeMetrics>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct OtlpResource {
    #[serde(default)]
    pub attributes: Vec<OtlpKeyValue>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct OtlpScopeMetrics {
    #[serde(default)]
    pub metrics: Vec<OtlpMetric>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct OtlpMetric {
    pub name: String,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub unit: Option<String>,
    #[serde(default)]
    pub gauge: Option<OtlpGauge>,
    #[serde(default)]
    pub sum: Option<OtlpSum>,
    #[serde(default)]
    pub histogram: Option<OtlpHistogram>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct OtlpGauge {
    #[serde(default, rename = "dataPoints")]
    pub data_points: Vec<OtlpNumberDataPoint>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct OtlpSum {
    #[serde(default, rename = "dataPoints")]
    pub data_points: Vec<OtlpNumberDataPoint>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct OtlpHistogram {
    #[serde(default, rename = "dataPoints")]
    pub data_points: Vec<OtlpHistogramDataPoint>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct OtlpNumberDataPoint {
    #[serde(default)]
    pub attributes: Vec<OtlpKeyValue>,
    #[serde(default, rename = "timeUnixNano")]
    pub time_unix_nano: Option<String>,
    #[serde(default, rename = "asDouble")]
    pub as_double: Option<f64>,
    #[serde(default, rename = "asInt")]
    pub as_int: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct OtlpHistogramDataPoint {
    #[serde(default)]
    pub attributes: Vec<OtlpKeyValue>,
    #[serde(default, rename = "timeUnixNano")]
    pub time_unix_nano: Option<String>,
    #[serde(default)]
    pub count: Option<String>,
    #[serde(default)]
    pub sum: Option<f64>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct OtlpKeyValue {
    pub key: String,
    #[serde(default)]
    pub value: Option<OtlpAnyValue>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct OtlpAnyValue {
    #[serde(default, rename = "stringValue")]
    pub string_value: Option<String>,
    #[serde(default, rename = "intValue")]
    pub int_value: Option<String>,
    #[serde(default, rename = "doubleValue")]
    pub double_value: Option<f64>,
    #[serde(default, rename = "boolValue")]
    pub bool_value: Option<bool>,
}

/// Simple JSON batch used by language SDKs.
#[derive(Debug, Deserialize)]
pub struct SimpleBatch {
    pub series: Vec<SimpleSeries>,
}

#[derive(Debug, Deserialize)]
pub struct SimpleSeries {
    pub metric: String,
    #[serde(default)]
    pub r#type: Option<String>,
    #[serde(default)]
    pub tags: BTreeMap<String, String>,
    pub points: Vec<[f64; 2]>,
    #[serde(default)]
    pub unit: Option<String>,
    #[serde(default)]
    pub description: Option<String>,
}

pub struct IngestService {
    store: Arc<MetricStore>,
}

impl IngestService {
    pub fn new(store: Arc<MetricStore>) -> Self {
        Self { store }
    }

    pub fn ingest_otlp_json(&self, body: &[u8]) -> Result<IngestStats> {
        let req: OtlpExportRequest = serde_json::from_slice(body)
            .map_err(|e| anyhow!("invalid OTLP JSON: {e}"))?;
        let points = otlp_to_points(req);
        Ok(self.store.ingest_batch(points))
    }

    pub fn ingest_simple_json(&self, body: &[u8]) -> Result<IngestStats> {
        // Accept either {series:[...]} or a bare array of points.
        let value: Value = serde_json::from_slice(body)?;
        let points = if value.get("series").is_some() {
            let batch: SimpleBatch = serde_json::from_value(value)?;
            simple_to_points(batch)
        } else if value.is_array() {
            // [{name, value, tags?, timestamp?}]
            let arr = value.as_array().cloned().unwrap_or_default();
            arr.into_iter()
                .filter_map(|v| {
                    let name = v.get("name")?.as_str()?.to_string();
                    let metric_value = v.get("value")?.as_f64()?;
                    let ts = v
                        .get("timestamp_ms")
                        .and_then(|t| t.as_i64())
                        .unwrap_or_else(|| Utc::now().timestamp_millis());
                    let tags = v
                        .get("tags")
                        .and_then(|t| serde_json::from_value::<Tags>(t.clone()).ok())
                        .unwrap_or_default();
                    let metric_type = parse_type(v.get("type").and_then(|t| t.as_str()));
                    Some(MetricPoint {
                        name,
                        metric_type,
                        tags,
                        sample: Sample {
                            timestamp_ms: ts,
                            value: metric_value,
                        },
                        unit: v
                            .get("unit")
                            .and_then(|u| u.as_str())
                            .map(|s| s.to_string()),
                        description: None,
                    })
                })
                .collect()
        } else {
            return Err(anyhow!("expected series object or point array"));
        };
        Ok(self.store.ingest_batch(points))
    }

    pub fn ingest_statsd_lines(&self, body: &str) -> IngestStats {
        let mut points = Vec::new();
        for line in body.lines() {
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            match parse_statsd(line) {
                Ok(p) => points.push(p),
                Err(e) => warn!(error = %e, line, "skip bad statsd line"),
            }
        }
        self.store.ingest_batch(points)
    }
}

fn otlp_to_points(req: OtlpExportRequest) -> Vec<MetricPoint> {
    let mut points = Vec::new();
    for rm in req.resource_metrics {
        let resource_tags = rm
            .resource
            .as_ref()
            .map(|r| kvs_to_tags(&r.attributes))
            .unwrap_or_default();

        for sm in rm.scope_metrics {
            for metric in sm.metrics {
                if let Some(ref gauge) = metric.gauge {
                    for dp in &gauge.data_points {
                        if let Some(p) = number_dp_to_point(
                            &metric,
                            MetricType::Gauge,
                            &resource_tags,
                            dp.clone(),
                        ) {
                            points.push(p);
                        }
                    }
                }
                if let Some(ref sum) = metric.sum {
                    for dp in &sum.data_points {
                        if let Some(p) = number_dp_to_point(
                            &metric,
                            MetricType::Counter,
                            &resource_tags,
                            dp.clone(),
                        ) {
                            points.push(p);
                        }
                    }
                }
                if let Some(ref hist) = metric.histogram {
                    for dp in &hist.data_points {
                        let mut tags = resource_tags.clone();
                        tags.extend(kvs_to_tags(&dp.attributes));
                        let ts = nano_to_ms(dp.time_unix_nano.as_deref());
                        if let Some(sum) = dp.sum {
                            points.push(MetricPoint {
                                name: format!("{}.sum", metric.name),
                                metric_type: MetricType::Histogram,
                                tags: tags.clone(),
                                sample: Sample {
                                    timestamp_ms: ts,
                                    value: sum,
                                },
                                unit: metric.unit.clone(),
                                description: metric.description.clone(),
                            });
                        }
                        if let Some(count) = dp.count.as_deref().and_then(|c| c.parse::<f64>().ok())
                        {
                            points.push(MetricPoint {
                                name: format!("{}.count", metric.name),
                                metric_type: MetricType::Histogram,
                                tags,
                                sample: Sample {
                                    timestamp_ms: ts,
                                    value: count,
                                },
                                unit: Some("1".into()),
                                description: metric.description.clone(),
                            });
                        }
                    }
                }
            }
        }
    }
    points
}

fn number_dp_to_point(
    metric: &OtlpMetric,
    metric_type: MetricType,
    resource_tags: &Tags,
    dp: OtlpNumberDataPoint,
) -> Option<MetricPoint> {
    let value = dp
        .as_double
        .or_else(|| dp.as_int.as_deref().and_then(|s| s.parse().ok()))?;
    let mut tags = resource_tags.clone();
    tags.extend(kvs_to_tags(&dp.attributes));
    Some(MetricPoint {
        name: metric.name.clone(),
        metric_type,
        tags,
        sample: Sample {
            timestamp_ms: nano_to_ms(dp.time_unix_nano.as_deref()),
            value,
        },
        unit: metric.unit.clone(),
        description: metric.description.clone(),
    })
}

fn simple_to_points(batch: SimpleBatch) -> Vec<MetricPoint> {
    let mut points = Vec::new();
    for series in batch.series {
        let metric_type = parse_type(series.r#type.as_deref());
        for point in series.points {
            // [timestamp_seconds_or_ms, value]
            let ts_raw = point[0] as i64;
            let timestamp_ms = if ts_raw > 1_000_000_000_000 {
                ts_raw
            } else {
                ts_raw * 1000
            };
            points.push(MetricPoint {
                name: series.metric.clone(),
                metric_type,
                tags: series.tags.clone(),
                sample: Sample {
                    timestamp_ms,
                    value: point[1],
                },
                unit: series.unit.clone(),
                description: series.description.clone(),
            });
        }
    }
    points
}

fn parse_statsd(line: &str) -> Result<MetricPoint> {
    // name:value|type|#tag1:v1,tag2:v2
    let (name_value, rest) = line
        .split_once('|')
        .ok_or_else(|| anyhow!("missing type"))?;
    let (name, value_s) = name_value
        .split_once(':')
        .ok_or_else(|| anyhow!("missing value"))?;
    let value: f64 = value_s.parse()?;
    let mut parts = rest.split('|');
    let type_s = parts.next().unwrap_or("g");
    let metric_type = match type_s {
        "c" => MetricType::Counter,
        "g" => MetricType::Gauge,
        "h" | "ms" => MetricType::Histogram,
        _ => MetricType::Untyped,
    };
    let mut tags = Tags::new();
    for part in parts {
        if let Some(tag_blob) = part.strip_prefix('#') {
            for pair in tag_blob.split(',') {
                if let Some((k, v)) = pair.split_once(':') {
                    tags.insert(k.to_string(), v.to_string());
                } else if !pair.is_empty() {
                    tags.insert(pair.to_string(), "true".into());
                }
            }
        }
    }
    Ok(MetricPoint {
        name: name.to_string(),
        metric_type,
        tags,
        sample: Sample {
            timestamp_ms: Utc::now().timestamp_millis(),
            value,
        },
        unit: None,
        description: None,
    })
}

fn kvs_to_tags(attrs: &[OtlpKeyValue]) -> Tags {
    let mut tags = Tags::new();
    for kv in attrs {
        let value = kv
            .value
            .as_ref()
            .and_then(|v| {
                v.string_value
                    .clone()
                    .or_else(|| v.int_value.clone())
                    .or_else(|| v.double_value.map(|d| d.to_string()))
                    .or_else(|| v.bool_value.map(|b| b.to_string()))
            })
            .unwrap_or_default();
        if !kv.key.is_empty() {
            tags.insert(kv.key.clone(), value);
        }
    }
    tags
}

fn nano_to_ms(nano: Option<&str>) -> i64 {
    nano.and_then(|s| s.parse::<i128>().ok())
        .map(|n| (n / 1_000_000) as i64)
        .unwrap_or_else(|| Utc::now().timestamp_millis())
}

fn parse_type(s: Option<&str>) -> MetricType {
    match s.map(|s| s.to_ascii_lowercase()).as_deref() {
        Some("counter") | Some("c") | Some("sum") => MetricType::Counter,
        Some("histogram") | Some("h") => MetricType::Histogram,
        Some("summary") => MetricType::Summary,
        Some("gauge") | Some("g") | None => MetricType::Gauge,
        _ => MetricType::Untyped,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use thine_storage::StorageConfig;

    #[test]
    fn parses_statsd() {
        let p = parse_statsd("api.latency:42|h|#service:web,env:dev").unwrap();
        assert_eq!(p.name, "api.latency");
        assert_eq!(p.sample.value, 42.0);
        assert_eq!(p.tags.get("service").unwrap(), "web");
    }

    #[test]
    fn ingests_otlp_gauge() {
        let store = MetricStore::new(StorageConfig::default());
        let svc = IngestService::new(store.clone());
        let body = r#"{
          "resourceMetrics": [{
            "resource": {"attributes": [{"key": "service.name", "value": {"stringValue": "demo"}}]},
            "scopeMetrics": [{
              "metrics": [{
                "name": "process.cpu.utilization",
                "gauge": {"dataPoints": [{"asDouble": 0.42, "timeUnixNano": "1700000000000000000"}]}
              }]
            }]
          }]
        }"#;
        let stats = svc.ingest_otlp_json(body.as_bytes()).unwrap();
        assert_eq!(stats.accepted, 1);
        assert_eq!(store.list_metrics(None).len(), 1);
    }
}
