//! OTLP traces (ExportTraceServiceRequest JSON subset) → IntakeSpan.

use anyhow::{anyhow, Result};
use serde::Deserialize;
use thine_common::IntakeSpan;

use crate::tags::{kvs_to_tags, service_from_tags};
use crate::{nano_to_ms, OtlpKeyValue, OtlpResource};

#[derive(Debug, Clone, Deserialize)]
struct OtlpTracesRequest {
    #[serde(default, rename = "resourceSpans")]
    resource_spans: Vec<OtlpResourceSpans>,
}

#[derive(Debug, Clone, Deserialize)]
struct OtlpResourceSpans {
    #[serde(default)]
    resource: Option<OtlpResource>,
    #[serde(default, rename = "scopeSpans")]
    scope_spans: Vec<OtlpScopeSpans>,
}

#[derive(Debug, Clone, Deserialize)]
struct OtlpScopeSpans {
    #[serde(default)]
    spans: Vec<OtlpSpan>,
}

#[derive(Debug, Clone, Deserialize)]
struct OtlpSpan {
    #[serde(default, rename = "traceId")]
    trace_id: String,
    #[serde(default, rename = "spanId")]
    span_id: String,
    #[serde(default, rename = "parentSpanId")]
    parent_span_id: Option<String>,
    #[serde(default)]
    name: String,
    #[serde(default, rename = "startTimeUnixNano")]
    start_time_unix_nano: Option<String>,
    #[serde(default, rename = "endTimeUnixNano")]
    end_time_unix_nano: Option<String>,
    #[serde(default)]
    attributes: Vec<OtlpKeyValue>,
    #[serde(default)]
    status: Option<OtlpStatus>,
}

#[derive(Debug, Clone, Deserialize)]
struct OtlpStatus {
    #[serde(default)]
    code: Option<i32>,
    #[serde(default)]
    message: Option<String>,
}

pub fn parse_otlp_traces_json(body: &[u8]) -> Result<Vec<IntakeSpan>> {
    let req: OtlpTracesRequest =
        serde_json::from_slice(body).map_err(|e| anyhow!("invalid OTLP traces JSON: {e}"))?;
    let mut out = Vec::new();
    for rs in req.resource_spans {
        let resource_tags = rs
            .resource
            .as_ref()
            .map(|r| kvs_to_tags(&r.attributes))
            .unwrap_or_default();
        for ss in rs.scope_spans {
            for sp in ss.spans {
                let mut tags = resource_tags.clone();
                tags.extend(kvs_to_tags(&sp.attributes));
                let start = nano_to_ms(sp.start_time_unix_nano.as_deref());
                let end = nano_to_ms(sp.end_time_unix_nano.as_deref());
                let duration_ms = if end > start {
                    (end - start) as f64
                } else {
                    0.0
                };
                let status = match sp.status.as_ref().and_then(|s| s.code) {
                    Some(2) => "error".into(), // STATUS_CODE_ERROR
                    Some(1) => "ok".into(),    // STATUS_CODE_OK
                    _ => {
                        if sp
                            .status
                            .as_ref()
                            .and_then(|s| s.message.as_ref())
                            .map(|m| m.to_ascii_lowercase().contains("error"))
                            .unwrap_or(false)
                        {
                            "error".into()
                        } else {
                            "ok".into()
                        }
                    }
                };
                let service = service_from_tags(&tags);
                out.push(IntakeSpan {
                    trace_id: sp.trace_id,
                    span_id: sp.span_id,
                    parent_span_id: sp.parent_span_id.filter(|s| !s.is_empty()),
                    service,
                    name: if sp.name.is_empty() {
                        "span".into()
                    } else {
                        sp.name
                    },
                    duration_ms,
                    timestamp_ms: start,
                    status,
                    resource: tags.get("http.route").cloned().or_else(|| tags.get("db.statement").cloned()),
                    tags,
                });
            }
        }
    }
    Ok(out)
}
