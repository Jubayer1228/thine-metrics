//! OTLP logs (ExportLogsServiceRequest JSON subset) → IntakeLog.

use anyhow::{anyhow, Result};
use serde::Deserialize;
use thine_common::IntakeLog;

use crate::tags::{any_to_string, kvs_to_tags, service_from_tags};
use crate::{nano_to_ms, OtlpAnyValue, OtlpKeyValue, OtlpResource};

#[derive(Debug, Clone, Deserialize)]
struct OtlpLogsRequest {
    #[serde(default, rename = "resourceLogs")]
    resource_logs: Vec<OtlpResourceLogs>,
}

#[derive(Debug, Clone, Deserialize)]
struct OtlpResourceLogs {
    #[serde(default)]
    resource: Option<OtlpResource>,
    #[serde(default, rename = "scopeLogs")]
    scope_logs: Vec<OtlpScopeLogs>,
}

#[derive(Debug, Clone, Deserialize)]
struct OtlpScopeLogs {
    #[serde(default, rename = "logRecords")]
    log_records: Vec<OtlpLogRecord>,
}

#[derive(Debug, Clone, Deserialize)]
struct OtlpLogRecord {
    #[serde(default, rename = "timeUnixNano")]
    time_unix_nano: Option<String>,
    #[serde(default, rename = "observedTimeUnixNano")]
    observed_time_unix_nano: Option<String>,
    #[serde(default, rename = "severityText")]
    severity_text: Option<String>,
    #[serde(default, rename = "severityNumber")]
    severity_number: Option<i32>,
    #[serde(default)]
    body: Option<OtlpAnyValue>,
    #[serde(default)]
    attributes: Vec<OtlpKeyValue>,
}

pub fn parse_otlp_logs_json(body: &[u8]) -> Result<Vec<IntakeLog>> {
    let req: OtlpLogsRequest =
        serde_json::from_slice(body).map_err(|e| anyhow!("invalid OTLP logs JSON: {e}"))?;
    let mut out = Vec::new();
    for rl in req.resource_logs {
        let resource_tags = rl
            .resource
            .as_ref()
            .map(|r| kvs_to_tags(&r.attributes))
            .unwrap_or_default();
        for sl in rl.scope_logs {
            for rec in sl.log_records {
                let mut attrs = resource_tags.clone();
                attrs.extend(kvs_to_tags(&rec.attributes));
                let level = rec
                    .severity_text
                    .clone()
                    .unwrap_or_else(|| severity_number_to_text(rec.severity_number))
                    .to_ascii_lowercase();
                let message = rec
                    .body
                    .as_ref()
                    .and_then(any_to_string)
                    .unwrap_or_else(|| attrs.get("message").cloned().unwrap_or_default());
                let ts = nano_to_ms(
                    rec.time_unix_nano
                        .as_deref()
                        .or(rec.observed_time_unix_nano.as_deref()),
                );
                out.push(IntakeLog {
                    timestamp_ms: ts,
                    level,
                    service: service_from_tags(&attrs),
                    message,
                    attrs,
                });
            }
        }
    }
    Ok(out)
}

fn severity_number_to_text(n: Option<i32>) -> String {
    match n.unwrap_or(0) {
        1..=4 => "trace".into(),
        5..=8 => "debug".into(),
        9..=12 => "info".into(),
        13..=16 => "warn".into(),
        17..=20 => "error".into(),
        21..=24 => "fatal".into(),
        _ => "info".into(),
    }
}
