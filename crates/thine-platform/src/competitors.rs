//! Multi-vendor competitive matrix: Thine vs SigNoz, Grafana, Datadog, New Relic,
//! CloudWatch, ClickStack, and Dash0.

use serde::Serialize;
use serde_json::{json, Value};

use crate::PlatformState;

#[derive(Debug, Clone, Copy, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum CompetitorId {
    Thine,
    Signoz,
    Grafana,
    Datadog,
    NewRelic,
    Cloudwatch,
    Clickstack,
    Dash0,
}

#[derive(Debug, Clone, Serialize)]
pub struct CompetitorProfile {
    pub id: CompetitorId,
    pub name: &'static str,
    pub positioning: &'static str,
    pub deployment: &'static str,
    pub license_model: &'static str,
    pub otel_stance: &'static str,
    pub strengths: &'static [&'static str],
    pub weaknesses: &'static [&'static str],
    pub best_when: &'static str,
    pub url: &'static str,
}

#[derive(Debug, Clone, Serialize)]
pub struct CapabilityScore {
    pub capability: &'static str,
    pub category: &'static str,
    pub thine: u8,
    pub signoz: u8,
    pub grafana: u8,
    pub datadog: u8,
    pub new_relic: u8,
    pub cloudwatch: u8,
    pub clickstack: u8,
    pub dash0: u8,
    pub note: &'static str,
}

pub fn competitor_profiles() -> Vec<CompetitorProfile> {
    vec![
        CompetitorProfile {
            id: CompetitorId::Thine,
            name: "Thine Metrics",
            positioning: "Self-hosted Datadog-surface observability in Rust — single binary, Apache-2.0",
            deployment: "Self-host (binary / compose); data stays in your VPC",
            license_model: "Apache-2.0",
            otel_stance: "OTLP-first ingest + Datadog-parity product surfaces (USM, Cloudcraft, CCM, HA)",
            strengths: &[
                "Predictable infra cost — no host/custom-metric tax",
                "Deep product map: Cloudcraft, USM, Watchdog, DBM, cost, multi-region HA",
                "MCP + Bits over local store for agentic ops",
                "Minutes to first dashboard",
            ],
            weaknesses: &[
                "Younger ecosystem vs Datadog/Grafana communities",
                "Not multi-tenant SaaS at hyperscale yet",
                "eBPF is userspace-simulator depth vs kernel-signed agents",
            ],
            best_when: "You want Datadog-like product breadth without SaaS billing, on your own metal",
            url: "http://localhost:4318",
        },
        CompetitorProfile {
            id: CompetitorId::Signoz,
            name: "SigNoz",
            positioning: "Open-source / Cloud OpenTelemetry-native APM — logs, metrics, traces on ClickHouse",
            deployment: "Self-host community, Cloud, enterprise self-host / BYOC",
            license_model: "Open-core (community + enterprise)",
            otel_stance: "OTel-first docs, exceptions from traces, messaging queues, correlation",
            strengths: &[
                "Best-in-class OTel-native UX for L/M/T",
                "ClickHouse query speed at high cardinality",
                "Usage-based Cloud pricing vs Datadog SKUs",
                "Trace funnels + messaging queue observability",
            ],
            weaknesses: &[
                "Narrower infra/CCM/SIEM/RUM vs Datadog",
                "ClickHouse ops burden when self-hosting",
                "Fewer Datadog-style enterprise product packs",
            ],
            best_when: "You want an OTel-native Datadog alternative with strong APM/logs/traces",
            url: "https://signoz.io",
        },
        CompetitorProfile {
            id: CompetitorId::Grafana,
            name: "Grafana LGTM",
            positioning: "Composable OSS stack: Loki + Grafana + Tempo + Mimir/Prometheus",
            deployment: "Self-host each component or Grafana Cloud",
            license_model: "AGPL (OSS components) + Cloud",
            otel_stance: "Strong OTLP support across backends; PromQL/LogQL/TraceQL",
            strengths: &[
                "Best dashboarding & multi-datasource pane of glass",
                "Object-storage-cheap retention (Loki/Tempo)",
                "Huge plugin / community ecosystem",
                "Mature Grafana Alerting",
            ],
            weaknesses: &[
                "4+ backends to operate for full L/M/T",
                "Correlation is configured, not always out-of-box",
                "Not a single opinionated APM product",
            ],
            best_when: "You already run Prometheus/Loki or need flexible multi-backend viz",
            url: "https://grafana.com",
        },
        CompetitorProfile {
            id: CompetitorId::Datadog,
            name: "Datadog",
            positioning: "Full-stack SaaS observability + security + digital experience",
            deployment: "SaaS (agent + OTLP)",
            license_model: "Proprietary SaaS SKUs",
            otel_stance: "Supports OTLP but agent-first; OTel metrics often billed as custom",
            strengths: &[
                "Broadest product surface (APM, infra, security, RUM, synthetics)",
                "800+ integrations, kernel eBPF USM",
                "Watchdog ML + Bits at multi-tenant scale",
                "Enterprise SLAs / compliance",
            ],
            weaknesses: &[
                "Complex unpredictable billing",
                "Custom metrics & host SKUs drive cost",
                "Data leaves your VPC (unless exceptional contracts)",
            ],
            best_when: "You need enterprise SaaS breadth and can absorb SKU pricing",
            url: "https://www.datadoghq.com",
        },
        CompetitorProfile {
            id: CompetitorId::NewRelic,
            name: "New Relic",
            positioning: "Full-stack SaaS observability with NRQL over NRDB",
            deployment: "SaaS only",
            license_model: "Proprietary (usage + user seats; free tier)",
            otel_stance: "Good OTLP ingest; platform still NRQL-centric",
            strengths: &[
                "Unified NRQL across signals",
                "Strong APM + browser/mobile",
                "Generous free ingest tier",
                "Code-level IDE hooks",
            ],
            weaknesses: &[
                "No true self-host / data residency product",
                "Proprietary query language lock-in",
                "Cost climbs with users + ingest",
            ],
            best_when: "You want SaaS APM with a free tier and accept NRQL lock-in",
            url: "https://newrelic.com",
        },
        CompetitorProfile {
            id: CompetitorId::Cloudwatch,
            name: "Amazon CloudWatch",
            positioning: "AWS-native metrics, logs, alarms; Omni / OTel metrics expanding",
            deployment: "AWS-managed",
            license_model: "AWS usage pricing",
            otel_stance: "Native OTel metrics (2026+); traces still X-Ray oriented",
            strengths: &[
                "Zero extra vendor if you are all-in on AWS",
                "Tight coupling to AWS services & IAM",
                "Alarms + EventBridge routing",
                "Improving PromQL / OTel metrics path",
            ],
            weaknesses: &[
                "Weaker cross-cloud / multi-signal UX",
                "Tracing/logs historically siloed (X-Ray / Logs Insights)",
                "High-cardinality and APM depth lag specialists",
            ],
            best_when: "Workloads are AWS-centric and you want managed native tooling",
            url: "https://aws.amazon.com/cloudwatch/",
        },
        CompetitorProfile {
            id: CompetitorId::Clickstack,
            name: "ClickStack",
            positioning: "ClickHouse Inc. OTel stack — Collector + ClickHouse + HyperDX UI",
            deployment: "OSS self-host or Managed on ClickHouse Cloud",
            license_model: "ClickHouse / HyperDX stack (OSS + managed)",
            otel_stance: "OTel-native ingestion with opinionated ClickHouse schemas",
            strengths: &[
                "Blazing ClickHouse analytics on telemetry",
                "Unified logs/traces/metrics/session direction",
                "Managed option removes CH ops",
                "Schema opinionated for high cardinality",
            ],
            weaknesses: &[
                "Younger product surface vs Datadog/SigNoz APM packs",
                "Self-host still means ClickHouse expertise",
                "Fewer enterprise packs (CCM, SIEM, synthetics)",
            ],
            best_when: "You want ClickHouse-grade query power with an OTel-native UI",
            url: "https://clickhouse.com/docs/clickstack/overview",
        },
        CompetitorProfile {
            id: CompetitorId::Dash0,
            name: "Dash0",
            positioning: "OTel-native SaaS observability on ClickHouse (Instana alumni)",
            deployment: "SaaS",
            license_model: "Proprietary SaaS",
            otel_stance: "Built OTel-first with ClickHouse as primary store",
            strengths: &[
                "Modern OTel UX with CH performance",
                "Cross-signal correlation focus",
                "Less SKU chaos than legacy APM giants",
            ],
            weaknesses: &[
                "SaaS-only — no self-host data residency",
                "Smaller ecosystem / integrations catalog",
                "Less infra/security breadth than Datadog",
            ],
            best_when: "You want a clean OTel SaaS on ClickHouse without self-hosting",
            url: "https://www.dash0.com",
        },
    ]
}

/// Capability scores 0–100 — honest relative strength, not marketing.
pub fn capability_scores() -> Vec<CapabilityScore> {
    vec![
        CapabilityScore {
            capability: "Distributed tracing / APM",
            category: "Applications",
            thine: 78,
            signoz: 90,
            grafana: 82,
            datadog: 95,
            new_relic: 93,
            cloudwatch: 70,
            clickstack: 85,
            dash0: 88,
            note: "SigNoz/Dash0/ClickStack strong OTel APM; Thine solid local stats + waterfall",
        },
        CapabilityScore {
            capability: "Log management",
            category: "Logs",
            thine: 75,
            signoz: 88,
            grafana: 85,
            datadog: 92,
            new_relic: 88,
            cloudwatch: 80,
            clickstack: 90,
            dash0: 88,
            note: "ClickHouse backends (SigNoz/ClickStack/Dash0) excel at high-cardinality filters",
        },
        CapabilityScore {
            capability: "Metrics & dashboards",
            category: "BuiltIn",
            thine: 82,
            signoz: 80,
            grafana: 95,
            datadog: 93,
            new_relic: 88,
            cloudwatch: 78,
            clickstack: 78,
            dash0: 80,
            note: "Grafana still wins flexible multi-source dashboards",
        },
        CapabilityScore {
            capability: "Alerts & routing",
            category: "BuiltIn",
            thine: 85,
            signoz: 78,
            grafana: 90,
            datadog: 95,
            new_relic: 88,
            cloudwatch: 85,
            clickstack: 72,
            dash0: 75,
            note: "Datadog/Grafana Alerting most mature; Thine solid threshold + webhook",
        },
        CapabilityScore {
            capability: "Exceptions / error tracking",
            category: "Logs",
            thine: 78,
            signoz: 92,
            grafana: 70,
            datadog: 90,
            new_relic: 90,
            cloudwatch: 65,
            clickstack: 80,
            dash0: 82,
            note: "SigNoz auto-lists exceptions from OTel traces — clear product lead",
        },
        CapabilityScore {
            capability: "Messaging queues (Kafka/Celery)",
            category: "Data",
            thine: 72,
            signoz: 90,
            grafana: 75,
            datadog: 88,
            new_relic: 80,
            cloudwatch: 70,
            clickstack: 78,
            dash0: 75,
            note: "SigNoz OTel-native queue monitoring is a headline differentiator",
        },
        CapabilityScore {
            capability: "Trace funnels (multi-step drop-off)",
            category: "Applications",
            thine: 40,
            signoz: 88,
            grafana: 45,
            datadog: 70,
            new_relic: 75,
            cloudwatch: 35,
            clickstack: 55,
            dash0: 60,
            note: "SigNoz productizes OTel-native funnels; Thine gap to close",
        },
        CapabilityScore {
            capability: "Infrastructure / processes",
            category: "ObsInfra",
            thine: 75,
            signoz: 78,
            grafana: 80,
            datadog: 95,
            new_relic: 85,
            cloudwatch: 88,
            clickstack: 70,
            dash0: 72,
            note: "Datadog still deepest infra maps; CloudWatch strong on AWS hosts",
        },
        CapabilityScore {
            capability: "OpenTelemetry-native experience",
            category: "Extensibility",
            thine: 80,
            signoz: 95,
            grafana: 88,
            datadog: 65,
            new_relic: 78,
            cloudwatch: 72,
            clickstack: 92,
            dash0: 94,
            note: "SigNoz/Dash0/ClickStack designed OTel-first; Datadog remains agent-first",
        },
        CapabilityScore {
            capability: "Self-host & data residency",
            category: "Platform",
            thine: 95,
            signoz: 90,
            grafana: 92,
            datadog: 25,
            new_relic: 15,
            cloudwatch: 40,
            clickstack: 88,
            dash0: 20,
            note: "Thine/SigNoz/Grafana/ClickStack can keep data on-prem; NR/Dash0 SaaS-only",
        },
        CapabilityScore {
            capability: "Cost predictability",
            category: "Platform",
            thine: 95,
            signoz: 85,
            grafana: 80,
            datadog: 35,
            new_relic: 55,
            cloudwatch: 60,
            clickstack: 75,
            dash0: 70,
            note: "Thine = infra bill only; SigNoz usage simpler than Datadog SKUs",
        },
        CapabilityScore {
            capability: "Cloud cost management",
            category: "ObsInfra",
            thine: 72,
            signoz: 40,
            grafana: 55,
            datadog: 90,
            new_relic: 70,
            cloudwatch: 75,
            clickstack: 35,
            dash0: 40,
            note: "Datadog CCM leads; Thine ships recommendations + Cloudcraft cost overlay",
        },
        CapabilityScore {
            capability: "USM / eBPF service discovery",
            category: "Applications",
            thine: 78,
            signoz: 45,
            grafana: 40,
            datadog: 95,
            new_relic: 55,
            cloudwatch: 40,
            clickstack: 40,
            dash0: 45,
            note: "Datadog eBPF USM is the gold standard; Thine has discover/RED simulator",
        },
        CapabilityScore {
            capability: "Watchdog / anomaly ML",
            category: "BuiltIn",
            thine: 72,
            signoz: 50,
            grafana: 55,
            datadog: 95,
            new_relic: 80,
            cloudwatch: 70,
            clickstack: 45,
            dash0: 50,
            note: "Datadog Watchdog corpora unmatched; Thine local z-score scan",
        },
        CapabilityScore {
            capability: "Integrations catalog",
            category: "Extensibility",
            thine: 82,
            signoz: 70,
            grafana: 90,
            datadog: 98,
            new_relic: 88,
            cloudwatch: 85,
            clickstack: 60,
            dash0: 55,
            note: "Datadog 800+; Thine 820-tile catalog (health toggles); Grafana plugins",
        },
        CapabilityScore {
            capability: "Multi-region HA",
            category: "Platform",
            thine: 80,
            signoz: 65,
            grafana: 70,
            datadog: 95,
            new_relic: 90,
            cloudwatch: 92,
            clickstack: 70,
            dash0: 75,
            note: "Hyperscalers win; Thine exposes quorum/failover/RPO APIs for self-host HA",
        },
        CapabilityScore {
            capability: "Database monitoring",
            category: "Data",
            thine: 78,
            signoz: 70,
            grafana: 72,
            datadog: 92,
            new_relic: 88,
            cloudwatch: 75,
            clickstack: 65,
            dash0: 68,
            note: "Datadog DBM deepest; Thine explain/schema/APM correlation",
        },
        CapabilityScore {
            capability: "Architecture maps (Cloudcraft-style)",
            category: "ObsInfra",
            thine: 85,
            signoz: 35,
            grafana: 40,
            datadog: 90,
            new_relic: 45,
            cloudwatch: 50,
            clickstack: 35,
            dash0: 40,
            note: "Thine Cloudcraft overlays (cost/obs/monitors) are a product differentiator vs SigNoz",
        },
        CapabilityScore {
            capability: "SaaS maturity & support SLAs",
            category: "Platform",
            thine: 45,
            signoz: 75,
            grafana: 85,
            datadog: 98,
            new_relic: 92,
            cloudwatch: 95,
            clickstack: 70,
            dash0: 65,
            note: "Honest: Thine is self-host-first; enterprise SaaS polish favors DD/NR/AWS/Grafana Cloud",
        },
    ]
}

fn avg_for(scores: &[CapabilityScore], pick: fn(&CapabilityScore) -> u8) -> f64 {
    if scores.is_empty() {
        return 0.0;
    }
    let s: u32 = scores.iter().map(|c| pick(c) as u32).sum();
    ((s as f64 / scores.len() as f64) * 10.0).round() / 10.0
}

impl PlatformState {
    pub fn competitors_comparison(&self) -> Value {
        let scores = capability_scores();
        let profiles = competitor_profiles();
        let averages = json!({
            "thine": avg_for(&scores, |c| c.thine),
            "signoz": avg_for(&scores, |c| c.signoz),
            "grafana": avg_for(&scores, |c| c.grafana),
            "datadog": avg_for(&scores, |c| c.datadog),
            "new_relic": avg_for(&scores, |c| c.new_relic),
            "cloudwatch": avg_for(&scores, |c| c.cloudwatch),
            "clickstack": avg_for(&scores, |c| c.clickstack),
            "dash0": avg_for(&scores, |c| c.dash0),
        });

        let thine_leads: Vec<_> = scores
            .iter()
            .filter(|c| {
                c.thine
                    >= *[
                        c.signoz,
                        c.grafana,
                        c.datadog,
                        c.new_relic,
                        c.cloudwatch,
                        c.clickstack,
                        c.dash0,
                    ]
                    .iter()
                    .max()
                    .unwrap()
            })
            .map(|c| c.capability)
            .collect();

        let vs_signoz_wins: Vec<_> = scores
            .iter()
            .filter(|c| c.thine > c.signoz)
            .map(|c| {
                json!({
                    "capability": c.capability,
                    "thine": c.thine,
                    "signoz": c.signoz,
                    "delta": c.thine as i16 - c.signoz as i16,
                })
            })
            .collect();

        let vs_signoz_gaps: Vec<_> = scores
            .iter()
            .filter(|c| c.signoz > c.thine)
            .map(|c| {
                json!({
                    "capability": c.capability,
                    "thine": c.thine,
                    "signoz": c.signoz,
                    "delta": c.signoz as i16 - c.thine as i16,
                })
            })
            .collect();

        json!({
            "generated_for": "thine_metrics",
            "catalog_features": crate::all_features().len(),
            "datadog_avg_parity_pct": (crate::avg_parity() * 10.0).round() / 10.0,
            "averages": averages,
            "profiles": profiles,
            "capabilities": scores,
            "thine_category_leads": thine_leads,
            "vs_signoz": {
                "thine_wins": vs_signoz_wins,
                "signoz_leads": vs_signoz_gaps,
                "summary": "Thine leads on self-host cost predictability, Cloudcraft/CCM, USM/Watchdog product packs, and multi-region HA APIs. SigNoz leads on OTel-native APM UX, exceptions-from-traces, messaging queues, and ClickHouse log query scale."
            },
            "pick_guide": [
                {"choose": "Thine", "when": "Datadog-like product breadth, Apache-2.0 self-host, predictable cost, Cloudcraft/USM/CCM/HA in one binary"},
                {"choose": "SigNoz", "when": "OTel-native APM/logs/traces with ClickHouse speed and optional Cloud"},
                {"choose": "Grafana LGTM", "when": "You need PromQL/LogQL/TraceQL flexibility and multi-datasource dashboards"},
                {"choose": "Datadog", "when": "Enterprise SaaS breadth, eBPF USM, Watchdog ML, compliance SLAs — and budget for SKUs"},
                {"choose": "New Relic", "when": "SaaS APM with NRQL + free ingest tier; no self-host requirement"},
                {"choose": "CloudWatch", "when": "AWS-only footprint and IAM-native ops"},
                {"choose": "ClickStack", "when": "ClickHouse-first analytics UI (HyperDX) with managed CH option"},
                {"choose": "Dash0", "when": "Modern OTel SaaS on ClickHouse without operating storage"},
            ],
            "sources": [
                "https://signoz.io/comparisons/signoz-vs-datadog/",
                "https://signoz.io (Product: tracing, logs, alerts, metrics, funnels, exceptions, messaging queues)",
                "https://clickhouse.com/docs/clickstack/overview",
                "https://www.dash0.com",
                "Thine live catalog GET /api/v1/features"
            ]
        })
    }
}
