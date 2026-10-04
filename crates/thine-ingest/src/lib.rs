//! Dual-path intake (Datadog-style):
//!   Native (JSON / StatsD) ─┐
//!   OTel OTLP (M/T/L)      ─┴─► normalize tags ─► fan-out
//!                                              ├─► MetricStore (RTDB stand-in)
//!                                              ├─► Trace sink  (APM)
//!                                              └─► Log sink    (Husky stand-in)
//!
//! Source path stops mattering after normalization — same internal models.

mod bus;
mod otlp_logs;
mod otlp_traces;
mod tags;

use anyhow::{anyhow, Result};
use chrono::Utc;
use serde::Deserialize;
use serde_json::Value;
use std::collections::BTreeMap;
use std::sync::{Arc, RwLock};
use thine_common::{
    normalize_tags, tag_or, IngestPath, IngestStats, IntakeEvent, IntakeLog, IntakeSpan, MetricPoint,
    MetricType, Sample, SignalKind, Tags,
};
use thine_storage::MetricStore;
use tracing::warn;

pub use bus::{spawn_intake_bus, BusMessage, DurableBundle, IntakeBus};
pub use otlp_logs::parse_otlp_logs_json;
pub use otlp_traces::parse_otlp_traces_json;
pub use tags::kvs_to_tags;

/// Callback sinks for non-metric signals (hot path; durable via IntakeBus).
pub type SpanSink = Arc<dyn Fn(Vec<IntakeSpan>) + Send + Sync>;
pub type LogSink = Arc<dyn Fn(Vec<IntakeLog>) + Send + Sync>;
pub type EventSink = Arc<dyn Fn(Vec<IntakeEvent>) + Send + Sync>;

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

/// Simple JSON batch used by language SDKs (native path).
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

#[derive(Debug, Clone, Default)]
struct PathCounters {
    native_metrics: u64,
    otel_metrics: u64,
    native_traces: u64,
    otel_traces: u64,
    native_logs: u64,
    otel_logs: u64,
    events: u64,
}

/// Central intake — native + OTel converge here, then fan out by signal.
pub struct IngestService {
    store: Arc<MetricStore>,
    span_sink: RwLock<Option<SpanSink>>,
    log_sink: RwLock<Option<LogSink>>,
    event_sink: RwLock<Option<EventSink>>,
    bus: RwLock<Option<IntakeBus>>,
    counters: RwLock<PathCounters>,
    durable_stats: RwLock<Option<serde_json::Value>>,
}

impl IngestService {
    pub fn new(store: Arc<MetricStore>) -> Self {
        Self {
            store,
            span_sink: RwLock::new(None),
            log_sink: RwLock::new(None),
            event_sink: RwLock::new(None),
            bus: RwLock::new(None),
            counters: RwLock::new(PathCounters::default()),
            durable_stats: RwLock::new(None),
        }
    }

    /// Wire PlatformState hot sinks (query path).
    pub fn set_sinks(&self, spans: SpanSink, logs: LogSink, events: EventSink) {
        *self.span_sink.write().unwrap() = Some(spans);
        *self.log_sink.write().unwrap() = Some(logs);
        *self.event_sink.write().unwrap() = Some(events);
    }

    /// Attach Kafka-like durable bus (RTDB / TraceStore / Husky workers).
    pub fn set_bus(&self, bus: IntakeBus) {
        *self.bus.write().unwrap() = Some(bus);
    }

    pub fn set_durable_stats(&self, stats: serde_json::Value) {
        *self.durable_stats.write().unwrap() = Some(stats);
    }

    fn bump(&self, path: IngestPath, kind: SignalKind, n: u64) {
        let mut c = self.counters.write().unwrap();
        match (path, kind) {
            (IngestPath::Native, SignalKind::Metrics) => c.native_metrics += n,
            (IngestPath::Otel, SignalKind::Metrics) => c.otel_metrics += n,
            (IngestPath::Native, SignalKind::Traces) => c.native_traces += n,
            (IngestPath::Otel, SignalKind::Traces) => c.otel_traces += n,
            (IngestPath::Native, SignalKind::Logs) => c.native_logs += n,
            (IngestPath::Otel, SignalKind::Logs) => c.otel_logs += n,
            (_, SignalKind::Events) => c.events += n,
        }
    }

    fn fanout_metrics(&self, mut points: Vec<MetricPoint>, path: IngestPath) -> IngestStats {
        for p in &mut points {
            p.tags = normalize_tags(std::mem::take(&mut p.tags));
        }
        let n = points.len() as u64;
        // Hot RTDB path (sync) for dashboards/alerts
        let stats = self.store.ingest_batch(points.clone());
        // Durable bus → WAL + tag index
        if let Some(bus) = self.bus.read().unwrap().clone() {
            bus.publish(BusMessage::Metrics { path, points });
        }
        self.bump(path, SignalKind::Metrics, n);
        stats
    }

    fn fanout_spans(&self, mut spans: Vec<IntakeSpan>, path: IngestPath) -> usize {
        for s in &mut spans {
            s.tags = normalize_tags(std::mem::take(&mut s.tags));
            if s.service.is_empty() {
                s.service = tag_or(&s.tags, "service", "unknown").to_string();
            }
        }
        // Always-on APM stats → metric store (Datadog: RED over all spans)
        self.emit_apm_stats(&spans);
        let n = spans.len();
        // Hot query sink
        if let Some(sink) = self.span_sink.read().unwrap().clone() {
            sink(spans.clone());
        }
        // Durable TraceStore via bus
        if let Some(bus) = self.bus.read().unwrap().clone() {
            bus.publish(BusMessage::Traces { path, spans });
        }
        self.bump(path, SignalKind::Traces, n as u64);
        n
    }

    fn fanout_logs(&self, mut logs: Vec<IntakeLog>, path: IngestPath) -> usize {
        for l in &mut logs {
            l.attrs = normalize_tags(std::mem::take(&mut l.attrs));
            if l.service.is_empty() {
                l.service = tag_or(&l.attrs, "service", "unknown").to_string();
            }
        }
        let n = logs.len();
        if let Some(sink) = self.log_sink.read().unwrap().clone() {
            sink(logs.clone());
        }
        if let Some(bus) = self.bus.read().unwrap().clone() {
            bus.publish(BusMessage::Logs { path, logs });
        }
        self.bump(path, SignalKind::Logs, n as u64);
        n
    }

    fn fanout_events(&self, events: Vec<IntakeEvent>) -> usize {
        let n = events.len();
        if let Some(sink) = self.event_sink.read().unwrap().clone() {
            sink(events.clone());
        }
        if let Some(bus) = self.bus.read().unwrap().clone() {
            bus.publish(BusMessage::Events {
                path: IngestPath::Native,
                events,
            });
        }
        self.bump(IngestPath::Native, SignalKind::Events, n as u64);
        n
    }

    /// Emit trace.* RED metrics so service maps stay complete even if spans are dropped later.
    fn emit_apm_stats(&self, spans: &[IntakeSpan]) {
        let mut points = Vec::with_capacity(spans.len() * 3);
        for s in spans {
            let mut tags = s.tags.clone();
            tags.insert("service".into(), s.service.clone());
            tags.insert("resource".into(), s.name.clone());
            tags.insert("status".into(), s.status.clone());
            let ts = s.timestamp_ms;
            points.push(MetricPoint {
                name: "trace.hits".into(),
                metric_type: MetricType::Counter,
                tags: tags.clone(),
                sample: Sample {
                    timestamp_ms: ts,
                    value: 1.0,
                },
                unit: Some("1".into()),
                description: Some("APM hit (all spans)".into()),
            });
            points.push(MetricPoint {
                name: "trace.duration".into(),
                metric_type: MetricType::Histogram,
                tags: tags.clone(),
                sample: Sample {
                    timestamp_ms: ts,
                    value: s.duration_ms,
                },
                unit: Some("ms".into()),
                description: Some("APM span duration".into()),
            });
            if s.status == "error" || s.status == "erroring" {
                points.push(MetricPoint {
                    name: "trace.errors".into(),
                    metric_type: MetricType::Counter,
                    tags,
                    sample: Sample {
                        timestamp_ms: ts,
                        value: 1.0,
                    },
                    unit: Some("1".into()),
                    description: Some("APM error span".into()),
                });
            }
        }
        if !points.is_empty() {
            let _ = self.store.ingest_batch(points.clone());
            if let Some(bus) = self.bus.read().unwrap().clone() {
                bus.publish(BusMessage::Metrics {
                    path: IngestPath::Native,
                    points,
                });
            }
        }
    }

    // —— Path 2: OpenTelemetry ——
    pub fn ingest_otlp_json(&self, body: &[u8]) -> Result<IngestStats> {
        let req: OtlpExportRequest =
            serde_json::from_slice(body).map_err(|e| anyhow!("invalid OTLP JSON: {e}"))?;
        let points = otlp_to_points(req);
        Ok(self.fanout_metrics(points, IngestPath::Otel))
    }

    pub fn ingest_otlp_traces_json(&self, body: &[u8]) -> Result<serde_json::Value> {
        let spans = parse_otlp_traces_json(body)?;
        let n = self.fanout_spans(spans, IngestPath::Otel);
        Ok(serde_json::json!({
            "partialSuccess": { "rejectedSpans": 0, "errorMessage": "" },
            "thine": { "accepted": n, "path": "otel", "signal": "traces" }
        }))
    }

    pub fn ingest_otlp_logs_json(&self, body: &[u8]) -> Result<serde_json::Value> {
        let logs = parse_otlp_logs_json(body)?;
        let n = self.fanout_logs(logs, IngestPath::Otel);
        Ok(serde_json::json!({
            "partialSuccess": { "rejectedLogRecords": 0, "errorMessage": "" },
            "thine": { "accepted": n, "path": "otel", "signal": "logs" }
        }))
    }

    // —— Path 1: Native ——
    pub fn ingest_simple_json(&self, body: &[u8]) -> Result<IngestStats> {
        let value: Value = serde_json::from_slice(body)?;
        let points = if value.get("series").is_some() {
            let batch: SimpleBatch = serde_json::from_value(value)?;
            simple_to_points(batch)
        } else if value.is_array() {
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
        Ok(self.fanout_metrics(points, IngestPath::Native))
    }

    pub fn ingest_statsd_lines(&self, body: &str) -> IngestStats {
        let mut points = Vec::new();
        let mut events = Vec::new();
        for line in body.lines() {
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            // DogStatsD event: _e{title_len,text_len}:title|text|...
            if line.starts_with("_e{") {
                if let Ok(ev) = parse_statsd_event(line) {
                    events.push(ev);
                }
                continue;
            }
            match parse_statsd(line) {
                Ok(p) => points.push(p),
                Err(e) => warn!(error = %e, line, "skip bad statsd line"),
            }
        }
        if !events.is_empty() {
            self.fanout_events(events);
        }
        self.fanout_metrics(points, IngestPath::Native)
    }

    /// Native APM JSON (same model as OTLP after normalize).
    pub fn ingest_native_spans(&self, spans: Vec<IntakeSpan>) -> usize {
        self.fanout_spans(spans, IngestPath::Native)
    }

    /// Native logs JSON.
    pub fn ingest_native_logs(&self, logs: Vec<IntakeLog>) -> usize {
        self.fanout_logs(logs, IngestPath::Native)
    }

    pub fn ingest_native_events(&self, events: Vec<IntakeEvent>) -> usize {
        self.fanout_events(events)
    }

    /// Architecture snapshot for UI / docs (Datadog dual-path diagram).
    pub fn architecture(&self) -> serde_json::Value {
        let c = self.counters.read().unwrap().clone();
        serde_json::json!({
            "model": "datadog_dual_path",
            "description": "Native Agent/SDK and OpenTelemetry converge at Thine intake, then fan out to signal stores.",
            "paths": {
                "native": {
                    "endpoints": ["/api/v1/ingest", "/api/v1/series", "/api/v1/ingest/statsd", "/api/v1/apm/traces", "/api/v1/logs/ingest", "/api/v1/events"],
                    "formats": ["JSON series", "DogStatsD lines", "Span JSON", "Log JSON"],
                    "accepted": {
                        "metrics": c.native_metrics,
                        "traces": c.native_traces,
                        "logs": c.native_logs,
                        "events": c.events
                    }
                },
                "otel": {
                    "endpoints": ["/v1/metrics", "/otlp/v1/metrics", "/v1/traces", "/otlp/v1/traces", "/v1/logs", "/otlp/v1/logs"],
                    "formats": ["OTLP/HTTP JSON"],
                    "accepted": {
                        "metrics": c.otel_metrics,
                        "traces": c.otel_traces,
                        "logs": c.otel_logs
                    }
                }
            },
            "normalize": {
                "tag_remap": ["service.name→service", "deployment.environment→env", "host.name→host", "k8s.*→kube_*"]
            },
            "fanout": {
                "metrics": {
                    "hot": "MetricStore",
                    "durable": "RtdbEngine WAL + tag index",
                    "signal": "metrics"
                },
                "traces": {
                    "hot": "PlatformState.spans",
                    "durable": "TraceStore (sampled, indexed by trace_id)",
                    "also_emits": ["trace.hits", "trace.duration", "trace.errors"],
                    "signal": "traces"
                },
                "logs": {
                    "hot": "PlatformState.logs",
                    "durable": "HuskyStore fragments + catalog",
                    "signal": "logs"
                },
                "events": {
                    "hot": "PlatformState.events",
                    "durable": "HuskyStore event fragments",
                    "signal": "events"
                }
            },
            "bus": "tokio mpsc (Kafka stand-in) — metrics/traces/logs/events partitions",
            "dogstatsd_udp": "THINE_STATSD_PORT (default 8125)",
            "durable": self.durable_stats.read().unwrap().clone(),
            "diagram": [
                "App SDK / DogStatsD UDP:8125 / Checks  OR  OTel SDK",
                "        │                                         │",
                "   Native HTTP + UDP                      OTLP /v1/{metrics,traces,logs}",
                "        └──────────────────┬──────────────────────┘",
                "                           ▼",
                "              Thine Intake (normalize tags)",
                "                           │",
                "              ┌─────────────┼ intake bus ─────────────┐",
                "              ▼             ▼                         ▼",
                "     RTDB+Index      TraceStore                  Husky",
                "     (metrics)       (APM sampled)          (logs/events)"
            ]
        })
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
                        if let Some(p) =
                            number_dp_to_point(&metric, MetricType::Gauge, &resource_tags, dp.clone())
                        {
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
                        if let Some(count) = dp.count.as_deref().and_then(|c| c.parse::<f64>().ok()) {
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

fn parse_statsd_event(line: &str) -> Result<IntakeEvent> {
    // _e{5,4}:title|text|#env:prod
    let rest = line
        .strip_prefix("_e{")
        .ok_or_else(|| anyhow!("not an event"))?;
    let (lens, body) = rest
        .split_once("}:")
        .ok_or_else(|| anyhow!("bad event header"))?;
    let mut li = lens.split(',');
    let title_len: usize = li.next().unwrap_or("0").parse().unwrap_or(0);
    let text_len: usize = li.next().unwrap_or("0").parse().unwrap_or(0);
    let title = body.chars().take(title_len).collect::<String>();
    let after_title = body.chars().skip(title_len).collect::<String>();
    let text_part = after_title.strip_prefix('|').unwrap_or(&after_title);
    let text = text_part.chars().take(text_len).collect::<String>();
    let mut tags = Tags::new();
    if let Some(tag_blob) = text_part.get(text_len..).and_then(|s| s.split('|').find(|p| p.starts_with('#'))) {
        for pair in tag_blob.trim_start_matches('#').split(',') {
            if let Some((k, v)) = pair.split_once(':') {
                tags.insert(k.into(), v.into());
            }
        }
    }
    Ok(IntakeEvent {
        timestamp_ms: Utc::now().timestamp_millis(),
        title,
        text,
        alert_type: "info".into(),
        tags,
        source: "dogstatsd".into(),
    })
}

pub(crate) fn nano_to_ms(nano: Option<&str>) -> i64 {
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
    fn otlp_metrics_normalize_service_name() {
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
        let metas = store.list_metrics(None);
        assert_eq!(metas.len(), 1);
        assert_eq!(metas[0].tags.get("service").unwrap(), "demo");
    }

    #[test]
    fn otlp_traces_fanout_and_apm_stats() {
        let store = MetricStore::new(StorageConfig::default());
        let svc = Arc::new(IngestService::new(store.clone()));
        let captured = Arc::new(RwLock::new(Vec::new()));
        let c2 = captured.clone();
        svc.set_sinks(
            Arc::new(move |spans| {
                c2.write().unwrap().extend(spans);
            }),
            Arc::new(|_| {}),
            Arc::new(|_| {}),
        );
        let body = r#"{
          "resourceSpans": [{
            "resource": {"attributes": [
              {"key": "service.name", "value": {"stringValue": "api"}},
              {"key": "deployment.environment", "value": {"stringValue": "prod"}}
            ]},
            "scopeSpans": [{
              "spans": [{
                "traceId": "abc123",
                "spanId": "def456",
                "name": "HTTP GET",
                "startTimeUnixNano": "1700000000000000000",
                "endTimeUnixNano": "1700000000100000000",
                "status": {"code": 1}
              }]
            }]
          }]
        }"#;
        let r = svc.ingest_otlp_traces_json(body.as_bytes()).unwrap();
        assert_eq!(r["thine"]["accepted"], 1);
        let spans = captured.read().unwrap();
        assert_eq!(spans.len(), 1);
        assert_eq!(spans[0].service, "api");
        assert!(store.list_metrics(Some("trace.")).iter().any(|m| m.name == "trace.hits"));
    }
}
