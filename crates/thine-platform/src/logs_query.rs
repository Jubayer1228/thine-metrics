//! Datadog Log Explorer–style query parsing: `service:api @http.status_code:500 -env:dev`

use std::collections::BTreeMap;

use crate::state::LogEvent;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LogClause {
    /// `service:api` or `level:error`
    Tag { key: String, value: String, neg: bool },
    /// `@http.status_code:500`
    Attr { key: String, value: String, neg: bool },
    /// Free-text term matched against message (case-insensitive contains)
    Text { term: String, neg: bool },
}

/// Parse a Datadog-like log query into clauses (AND semantics).
pub fn parse_log_query(q: &str) -> Vec<LogClause> {
    let mut out = Vec::new();
    for raw in q.split_whitespace() {
        let mut tok = raw.trim();
        if tok.is_empty() {
            continue;
        }
        let neg = tok.starts_with('-');
        if neg {
            tok = &tok[1..];
        }
        if tok.is_empty() {
            continue;
        }
        if let Some(rest) = tok.strip_prefix('@') {
            if let Some((k, v)) = rest.split_once(':') {
                out.push(LogClause::Attr {
                    key: k.to_string(),
                    value: strip_quotes(v),
                    neg,
                });
                continue;
            }
        }
        if let Some((k, v)) = tok.split_once(':') {
            if !k.is_empty() && !v.is_empty() && k.chars().all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '.') {
                out.push(LogClause::Tag {
                    key: k.to_string(),
                    value: strip_quotes(v),
                    neg,
                });
                continue;
            }
        }
        out.push(LogClause::Text {
            term: strip_quotes(tok),
            neg,
        });
    }
    out
}

fn strip_quotes(s: &str) -> String {
    let t = s.trim();
    if (t.starts_with('"') && t.ends_with('"')) || (t.starts_with('\'') && t.ends_with('\'')) {
        t[1..t.len() - 1].to_string()
    } else {
        t.to_string()
    }
}

fn match_value(actual: &str, pattern: &str) -> bool {
    if pattern.contains('*') {
        let pat = pattern.to_ascii_lowercase();
        let act = actual.to_ascii_lowercase();
        if let Some((pre, rest)) = pat.split_once('*') {
            return act.starts_with(pre) && (rest.is_empty() || act.contains(rest.trim_start_matches('*')));
        }
    }
    actual.eq_ignore_ascii_case(pattern)
}

pub fn log_matches(event: &LogEvent, clauses: &[LogClause]) -> bool {
    if clauses.is_empty() {
        return true;
    }
    clauses.iter().all(|c| {
        let hit = match c {
            LogClause::Tag { key, value, .. } => {
                let actual = match key.as_str() {
                    "service" => Some(event.service.as_str()),
                    "level" | "status" => Some(event.level.as_str()),
                    other => event.attrs.get(other).map(|s| s.as_str()),
                };
                actual.map(|a| match_value(a, value)).unwrap_or(false)
            }
            LogClause::Attr { key, value, .. } => event
                .attrs
                .get(key)
                .map(|a| match_value(a, value))
                .unwrap_or(false),
            LogClause::Text { term, .. } => event.message.to_ascii_lowercase().contains(&term.to_ascii_lowercase()),
        };
        match c {
            LogClause::Tag { neg, .. } | LogClause::Attr { neg, .. } | LogClause::Text { neg, .. } => {
                if *neg {
                    !hit
                } else {
                    hit
                }
            }
        }
    })
}

/// Uniform reservoir-style sample for Live Tail when volume is high.
pub fn sample_logs(mut logs: Vec<LogEvent>, max: usize) -> (Vec<LogEvent>, f64) {
    if logs.len() <= max {
        return (logs, 1.0);
    }
    let rate = max as f64 / logs.len() as f64;
    // Keep every Nth for deterministic sampling (statistically representative).
    let step = (logs.len() as f64 / max as f64).ceil() as usize;
    let sampled: Vec<_> = logs.drain(..).enumerate().filter(|(i, _)| i % step == 0).map(|(_, e)| e).take(max).collect();
    (sampled, rate)
}

pub fn facet_from_logs(logs: &[LogEvent]) -> Vec<(String, Vec<(String, u64)>)> {
    let mut maps: BTreeMap<String, BTreeMap<String, u64>> = BTreeMap::new();
    for e in logs {
        *maps
            .entry("level".into())
            .or_default()
            .entry(e.level.clone())
            .or_default() += 1;
        *maps
            .entry("service".into())
            .or_default()
            .entry(e.service.clone())
            .or_default() += 1;
        for (k, v) in &e.attrs {
            *maps
                .entry(format!("@{k}"))
                .or_default()
                .entry(v.clone())
                .or_default() += 1;
        }
    }
    maps.into_iter()
        .map(|(k, m)| {
            let mut vals: Vec<_> = m.into_iter().collect();
            vals.sort_by(|a, b| b.1.cmp(&a.1));
            (k, vals)
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_datadog_style() {
        let c = parse_log_query(r#"service:api @http.status_code:500 -env:dev timeout"#);
        assert_eq!(c.len(), 4);
    }
}
