//! Close remaining Datadog-parity debt with honest, product-shaped implementations:
//! log patterns (Drain-like), catalog scorecards / service-definition YAML,
//! kube container map topology, notebook sharing.

use std::collections::BTreeMap;

use chrono::Utc;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use uuid::Uuid;

use crate::state::{CatalogService, ErrorGroup, Notebook, PlatformState};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LogPattern {
    pub id: String,
    pub template: String,
    pub count: u64,
    pub services: Vec<String>,
    pub sample_message: String,
    pub first_seen_ms: i64,
    pub last_seen_ms: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScorecardCheck {
    pub id: String,
    pub name: String,
    pub passed: bool,
    pub weight: u8,
    pub detail: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServiceScorecard {
    pub service: String,
    pub score_pct: f64,
    pub checks: Vec<ScorecardCheck>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KubeMapNode {
    pub id: String,
    pub kind: String, // namespace | deployment | pod | container
    pub name: String,
    pub parent: Option<String>,
    pub cpu_pct: f64,
    pub mem_pct: f64,
    pub status: String,
    pub restarts: u32,
    pub service: String,
    pub env: String,
}

/// Drain-style template: replace volatile tokens so similar messages cluster.
pub fn message_template(msg: &str) -> String {
    let mut out = String::with_capacity(msg.len());
    let bytes = msg.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        let c = bytes[i] as char;
        if c.is_ascii_digit() {
            out.push_str("<*>");
            while i < bytes.len()
                && ((bytes[i] as char).is_ascii_digit() || bytes[i] == b'.' || bytes[i] == b'-')
            {
                i += 1;
            }
            continue;
        }
        // UUID-ish hex runs of length >= 8
        if c.is_ascii_hexdigit() {
            let start = i;
            while i < bytes.len()
                && ((bytes[i] as char).is_ascii_hexdigit() || bytes[i] == b'-')
            {
                i += 1;
            }
            let slice = &msg[start..i];
            let hexish = slice.chars().filter(|ch| ch.is_ascii_hexdigit()).count();
            if hexish >= 8 {
                out.push_str("<UUID>");
            } else {
                out.push_str(slice);
            }
            continue;
        }
        out.push(c);
        i += 1;
    }
    out.split_whitespace().collect::<Vec<_>>().join(" ")
}

impl PlatformState {
    /// Log Patterns Explorer — cluster messages into templates (Datadog Log Patterns).
    pub fn log_patterns(&self, limit: usize) -> Vec<LogPattern> {
        let mut map: BTreeMap<String, LogPattern> = BTreeMap::new();
        for e in self.logs.read().iter() {
            let template = message_template(&e.message);
            let id = format!("{:x}", fnv1a64(&template));
            map.entry(id.clone())
                .and_modify(|p| {
                    p.count += 1;
                    p.last_seen_ms = p.last_seen_ms.max(e.timestamp_ms);
                    p.first_seen_ms = p.first_seen_ms.min(e.timestamp_ms);
                    if !p.services.iter().any(|s| s == &e.service) {
                        p.services.push(e.service.clone());
                    }
                })
                .or_insert_with(|| LogPattern {
                    id,
                    template: template.clone(),
                    count: 1,
                    services: vec![e.service.clone()],
                    sample_message: e.message.clone(),
                    first_seen_ms: e.timestamp_ms,
                    last_seen_ms: e.timestamp_ms,
                });
        }
        let mut out: Vec<_> = map.into_values().collect();
        out.sort_by(|a, b| b.count.cmp(&a.count));
        out.truncate(limit.max(1).min(200));
        out
    }

    /// Software Catalog scorecards — ownership, SLOs, monitors, runbooks, tier.
    pub fn catalog_scorecard(&self, service: &str) -> Option<ServiceScorecard> {
        let cat = self
            .list_catalog()
            .into_iter()
            .find(|c| c.name == service)?;
        let svc_l = service.to_ascii_lowercase();
        let has_slo = self.list_slos().iter().any(|s| {
            s.slo.name.to_ascii_lowercase().contains(&svc_l)
                || s.slo.metric.to_ascii_lowercase().contains(&svc_l)
                || s.slo
                    .tags
                    .get("service")
                    .map(|v| v == service)
                    .unwrap_or(false)
        });
        let has_monitor = self.metrics.list_alerts().iter().any(|a| {
            a.name.to_ascii_lowercase().contains(&svc_l)
                || a.tags.get("service").map(|v| v == service).unwrap_or(false)
                || a.options.message.to_ascii_lowercase().contains(&svc_l)
        });
        let has_runbook = cat.links.contains_key("runbook") || cat.links.contains_key("docs");
        let has_repo = cat.links.contains_key("repo");
        let has_oncall = !cat.team.is_empty();
        let has_definition = cat
            .definition_yaml
            .as_ref()
            .map(|y| !y.trim().is_empty())
            .unwrap_or(false);
        let checks = vec![
            ScorecardCheck {
                id: "owner".into(),
                name: "Has owning team".into(),
                passed: has_oncall,
                weight: 20,
                detail: if has_oncall {
                    format!("team={}", cat.team)
                } else {
                    "missing team".into()
                },
            },
            ScorecardCheck {
                id: "tier".into(),
                name: "Tier declared".into(),
                passed: !cat.tier.is_empty(),
                weight: 10,
                detail: cat.tier.clone(),
            },
            ScorecardCheck {
                id: "repo".into(),
                name: "Source repo linked".into(),
                passed: has_repo,
                weight: 15,
                detail: cat
                    .links
                    .get("repo")
                    .cloned()
                    .unwrap_or_else(|| "missing".into()),
            },
            ScorecardCheck {
                id: "runbook".into(),
                name: "Runbook / docs linked".into(),
                passed: has_runbook,
                weight: 15,
                detail: if has_runbook {
                    "ok".into()
                } else {
                    "missing".into()
                },
            },
            ScorecardCheck {
                id: "slo".into(),
                name: "SLO defined".into(),
                passed: has_slo,
                weight: 20,
                detail: if has_slo {
                    "found".into()
                } else {
                    "no matching SLO".into()
                },
            },
            ScorecardCheck {
                id: "monitor".into(),
                name: "Monitor coverage".into(),
                passed: has_monitor,
                weight: 10,
                detail: if has_monitor {
                    "found".into()
                } else {
                    "no monitor".into()
                },
            },
            ScorecardCheck {
                id: "definition".into(),
                name: "Service definition YAML".into(),
                passed: has_definition,
                weight: 10,
                detail: if has_definition {
                    "present".into()
                } else {
                    "missing".into()
                },
            },
        ];
        let total_w: u32 = checks.iter().map(|c| c.weight as u32).sum();
        let earned: u32 = checks
            .iter()
            .filter(|c| c.passed)
            .map(|c| c.weight as u32)
            .sum();
        let score_pct = if total_w == 0 {
            0.0
        } else {
            (earned as f64 / total_w as f64) * 100.0
        };
        Some(ServiceScorecard {
            service: service.into(),
            score_pct: (score_pct * 10.0).round() / 10.0,
            checks,
        })
    }

    pub fn list_catalog_scorecards(&self) -> Vec<ServiceScorecard> {
        self.list_catalog_scorecards_for(crate::tenant::DEMO_ORG_ID)
    }

    pub fn list_catalog_scorecards_for(&self, org_id: &str) -> Vec<ServiceScorecard> {
        let mut out: Vec<_> = self
            .list_catalog_for(org_id)
            .into_iter()
            .filter_map(|c| self.catalog_scorecard(&c.name))
            .collect();
        out.sort_by(|a, b| {
            b.score_pct
                .partial_cmp(&a.score_pct)
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        out
    }

    pub fn catalog_key(org_id: &str, name: &str) -> String {
        format!("{org_id}:{name}")
    }

    pub fn get_service_definition_yaml(&self, name: &str) -> Option<String> {
        self.get_service_definition_yaml_for(crate::tenant::DEMO_ORG_ID, name)
    }

    pub fn get_service_definition_yaml_for(&self, org_id: &str, name: &str) -> Option<String> {
        let key = Self::catalog_key(org_id, name);
        let c = self.catalog.get(&key)?;
        Some(
            c.definition_yaml
                .clone()
                .unwrap_or_else(|| default_service_definition_yaml(&c)),
        )
    }

    pub fn put_service_definition_yaml(&self, name: &str, yaml: String) -> Option<CatalogService> {
        self.put_service_definition_yaml_for(crate::tenant::DEMO_ORG_ID, name, yaml)
    }

    pub fn put_service_definition_yaml_for(
        &self,
        org_id: &str,
        name: &str,
        yaml: String,
    ) -> Option<CatalogService> {
        let key = Self::catalog_key(org_id, name);
        let mut e = self.catalog.get_mut(&key)?;
        if !(yaml.contains("dd-service") || yaml.contains("kind:") || yaml.contains("name:")) {
            return None;
        }
        e.definition_yaml = Some(yaml.clone());
        for line in yaml.lines() {
            let t = line.trim();
            if let Some(v) = t.strip_prefix("team:") {
                let v = v.trim().trim_matches('"').to_string();
                if !v.is_empty() {
                    e.team = v;
                }
            }
            if let Some(v) = t.strip_prefix("tier:") {
                let v = v.trim().trim_matches('"').to_string();
                if !v.is_empty() {
                    e.tier = v;
                }
            }
            if let Some(v) = t.strip_prefix("lifecycle:") {
                e.lifecycle = Some(v.trim().trim_matches('"').to_string());
            }
        }
        self.audit("update", &format!("catalog:{name}"), "system");
        Some(e.clone())
    }

    /// Dedicated kube Container Map topology (not Host Map reuse).
    pub fn kube_container_map(&self) -> Value {
        let containers = self.list_containers();
        let mut nodes: Vec<KubeMapNode> = Vec::new();
        let mut ns_seen = BTreeMap::<String, (f64, f64, u32)>::new();
        let mut dep_seen = BTreeMap::<String, (f64, f64, u32)>::new();
        let mut pod_seen = BTreeMap::<String, bool>::new();
        for c in &containers {
            let ns = c
                .kube_namespace
                .clone()
                .unwrap_or_else(|| "default".into());
            let dep = c
                .kube_deployment
                .clone()
                .unwrap_or_else(|| c.service.clone());
            let pod = c
                .pod_name
                .clone()
                .unwrap_or_else(|| format!("{}-pod", c.name));
            let mem_pct = if c.mem_limit_mb > 0.0 {
                (c.mem_usage_mb / c.mem_limit_mb) * 100.0
            } else {
                0.0
            };
            let e_ns = ns_seen.entry(ns.clone()).or_insert((0.0, 0.0, 0));
            e_ns.0 += c.cpu_pct;
            e_ns.1 += mem_pct;
            e_ns.2 += 1;
            let dep_id = format!("dep:{ns}/{dep}");
            let e_dep = dep_seen.entry(dep_id.clone()).or_insert((0.0, 0.0, 0));
            e_dep.0 += c.cpu_pct;
            e_dep.1 += mem_pct;
            e_dep.2 += 1;
            let pod_id = format!("pod:{ns}/{pod}");
            if pod_seen.insert(pod_id.clone(), true).is_none() {
                nodes.push(KubeMapNode {
                    id: pod_id.clone(),
                    kind: "pod".into(),
                    name: pod,
                    parent: Some(dep_id.clone()),
                    cpu_pct: c.cpu_pct,
                    mem_pct,
                    status: c.status.clone(),
                    restarts: c.restarts,
                    service: c.service.clone(),
                    env: c.env.clone(),
                });
            }
            nodes.push(KubeMapNode {
                id: format!("ctr:{}", c.id),
                kind: "container".into(),
                name: if c.name.is_empty() {
                    c.id.chars().take(12).collect()
                } else {
                    c.name.clone()
                },
                parent: Some(pod_id),
                cpu_pct: c.cpu_pct,
                mem_pct,
                status: c.status.clone(),
                restarts: c.restarts,
                service: c.service.clone(),
                env: c.env.clone(),
            });
        }
        for (ns, (cpu, mem, n)) in &ns_seen {
            nodes.push(KubeMapNode {
                id: format!("ns:{ns}"),
                kind: "namespace".into(),
                name: ns.clone(),
                parent: None,
                cpu_pct: cpu / (*n as f64).max(1.0),
                mem_pct: mem / (*n as f64).max(1.0),
                status: "Active".into(),
                restarts: 0,
                service: String::new(),
                env: String::new(),
            });
        }
        for (dep_id, (cpu, mem, n)) in &dep_seen {
            let path = dep_id.strip_prefix("dep:").unwrap_or(dep_id);
            let mut parts = path.split('/');
            let ns = parts.next().unwrap_or("default");
            let dep_name = parts.next().unwrap_or(path);
            nodes.push(KubeMapNode {
                id: dep_id.clone(),
                kind: "deployment".into(),
                name: dep_name.into(),
                parent: Some(format!("ns:{ns}")),
                cpu_pct: cpu / (*n as f64).max(1.0),
                mem_pct: mem / (*n as f64).max(1.0),
                status: "Running".into(),
                restarts: 0,
                service: String::new(),
                env: String::new(),
            });
        }
        let rank = |k: &str| match k {
            "namespace" => 0,
            "deployment" => 1,
            "pod" => 2,
            _ => 3,
        };
        nodes.sort_by(|a, b| rank(&a.kind).cmp(&rank(&b.kind)).then(a.name.cmp(&b.name)));
        json!({
            "nodes": nodes,
            "counts": {
                "namespaces": ns_seen.len(),
                "deployments": dep_seen.len(),
                "containers": containers.len(),
            },
            "fill_by": ["CPU", "Memory", "Restarts"],
            "docs_ref": "https://docs.datadoghq.com/infrastructure/containermap/"
        })
    }

    pub fn share_notebook(&self, id: Uuid) -> Option<Notebook> {
        let mut e = self.notebooks.get_mut(&id)?;
        if e.share_token.is_none() {
            e.share_token = Some(format!("nb_{}", Uuid::new_v4().as_simple()));
        }
        e.updated_at_ms = Utc::now().timestamp_millis();
        Some(e.clone())
    }

    pub fn get_notebook_by_share(&self, token: &str) -> Option<Notebook> {
        self.notebooks
            .iter()
            .find(|e| e.share_token.as_deref() == Some(token))
            .map(|e| e.value().clone())
    }

    pub fn link_error_issue(
        &self,
        id: &str,
        issue: String,
        trace_id: Option<String>,
    ) -> Option<ErrorGroup> {
        let mut e = self.errors.get_mut(id)?;
        if !e.linked_issues.iter().any(|x| x == &issue) {
            e.linked_issues.push(issue);
        }
        if let Some(t) = trace_id {
            e.linked_trace_id = Some(t);
        }
        Some(e.clone())
    }
}

pub fn default_service_definition_yaml(c: &CatalogService) -> String {
    let langs = c
        .languages
        .iter()
        .map(|l| format!("  - {l}"))
        .collect::<Vec<_>>()
        .join("\n");
    let repo = c.links.get("repo").cloned().unwrap_or_default();
    let runbook = c
        .links
        .get("runbook")
        .cloned()
        .unwrap_or_else(|| format!("https://runbooks.thine.local/{}", c.name));
    format!(
        "---\nschema-version: v2.2\ndd-service: {}\nteam: {}\ntier: {}\nlifecycle: {}\nlanguages:\n{}\ncontacts:\n  - type: slack\n    contact: \"#{}\"\nlinks:\n  - name: repo\n    type: repo\n    url: {}\n  - name: runbook\n    type: runbook\n    url: {}\n",
        c.name,
        c.team,
        c.tier,
        c.lifecycle.as_deref().unwrap_or("production"),
        langs,
        c.team,
        repo,
        runbook
    )
}

fn fnv1a64(s: &str) -> u64 {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for b in s.as_bytes() {
        h ^= *b as u64;
        h = h.wrapping_mul(0x1000_0000_01b3);
    }
    h
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn templates_normalize_numbers() {
        let a = message_template("handled request #14 status=500");
        let b = message_template("handled request #7 status=500");
        assert_eq!(a, b);
        assert!(a.contains("<*>"));
    }
}
