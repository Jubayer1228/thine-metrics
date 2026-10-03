//! Full Datadog-surface feature inventory for tracking parity.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum FeatureStatus {
    /// Substantial working implementation
    Done,
    /// Working MVP API + storage
    Partial,
    /// Endpoint exists, placeholder/minimal
    Stub,
    /// Documented only — not exposed yet
    Planned,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum FeatureCategory {
    BuiltIn,
    Extensibility,
    ObservabilityInfra,
    Applications,
    Data,
    Logs,
}

#[derive(Debug, Clone, Serialize)]
pub struct FeatureEntry {
    pub id: &'static str,
    pub name: &'static str,
    pub category: FeatureCategory,
    pub status: FeatureStatus,
    pub endpoints: &'static [&'static str],
    pub notes: &'static str,
}

pub fn all_features() -> Vec<FeatureEntry> {
    FEATURES.to_vec()
}

pub fn feature_stats() -> FeatureStats {
    let mut stats = FeatureStats::default();
    for f in FEATURES {
        stats.total += 1;
        match f.status {
            FeatureStatus::Done => stats.done += 1,
            FeatureStatus::Partial => stats.partial += 1,
            FeatureStatus::Stub => stats.stub += 1,
            FeatureStatus::Planned => stats.planned += 1,
        }
    }
    stats
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct FeatureStats {
    pub total: usize,
    pub done: usize,
    pub partial: usize,
    pub stub: usize,
    pub planned: usize,
}

const FEATURES: &[FeatureEntry] = &[
    // —— Built-in ——
    FeatureEntry {
        id: "bits_ai",
        name: "Bits AI",
        category: FeatureCategory::BuiltIn,
        status: FeatureStatus::Stub,
        endpoints: &["/api/v1/bits/chat"],
        notes: "Structured chat endpoint; no LLM yet",
    },
    FeatureEntry {
        id: "bits_chat",
        name: "Bits Chat",
        category: FeatureCategory::BuiltIn,
        status: FeatureStatus::Stub,
        endpoints: &["/api/v1/bits/chat"],
        notes: "Shares Bits AI chat surface",
    },
    FeatureEntry {
        id: "bits_agent_builder",
        name: "Bits Agent Builder",
        category: FeatureCategory::BuiltIn,
        status: FeatureStatus::Stub,
        endpoints: &["/api/v1/agents"],
        notes: "Agent directory CRUD; builder UI TBD",
    },
    FeatureEntry {
        id: "alerts",
        name: "Alerts",
        category: FeatureCategory::BuiltIn,
        status: FeatureStatus::Done,
        endpoints: &["/api/v1/alerts", "/api/v1/alerts/events"],
        notes: "Threshold monitors + firing events",
    },
    FeatureEntry {
        id: "custom_metrics",
        name: "Custom Metrics",
        category: FeatureCategory::BuiltIn,
        status: FeatureStatus::Done,
        endpoints: &["/api/v1/ingest", "/api/v1/query", "/api/v1/metrics"],
        notes: "OTLP/JSON/StatsD ingest + query",
    },
    FeatureEntry {
        id: "dashboards",
        name: "Dashboards",
        category: FeatureCategory::BuiltIn,
        status: FeatureStatus::Done,
        endpoints: &["/api/v1/boards", "/api/v1/boards/{id}/render"],
        notes: "query_value / timeseries / toplist screenboards",
    },
    FeatureEntry {
        id: "notebooks",
        name: "Notebooks",
        category: FeatureCategory::BuiltIn,
        status: FeatureStatus::Partial,
        endpoints: &["/api/v1/notebooks"],
        notes: "Cells with markdown + metric queries",
    },
    FeatureEntry {
        id: "mobile_app",
        name: "Mobile App",
        category: FeatureCategory::BuiltIn,
        status: FeatureStatus::Planned,
        endpoints: &[],
        notes: "Responsive web only today",
    },
    FeatureEntry {
        id: "fleet_automation",
        name: "Fleet Automation",
        category: FeatureCategory::BuiltIn,
        status: FeatureStatus::Stub,
        endpoints: &["/api/v1/fleet/agents"],
        notes: "Agent inventory stub",
    },
    FeatureEntry {
        id: "access_control",
        name: "Access Control",
        category: FeatureCategory::BuiltIn,
        status: FeatureStatus::Partial,
        endpoints: &["/api/v1/rbac/users", "/api/v1/rbac/roles"],
        notes: "Users/roles model; no auth middleware yet",
    },
    FeatureEntry {
        id: "governance_console",
        name: "Governance Console",
        category: FeatureCategory::BuiltIn,
        status: FeatureStatus::Partial,
        endpoints: &["/api/v1/governance/policies"],
        notes: "Metric retention & tag policies",
    },
    FeatureEntry {
        id: "dora_metrics",
        name: "DORA Metrics + AI Impact",
        category: FeatureCategory::BuiltIn,
        status: FeatureStatus::Partial,
        endpoints: &["/api/v1/dora"],
        notes: "Deploy frequency, lead time, CFR, MTTR from seeded events",
    },
    FeatureEntry {
        id: "incident_management",
        name: "Incident Management",
        category: FeatureCategory::BuiltIn,
        status: FeatureStatus::Partial,
        endpoints: &["/api/v1/incidents"],
        notes: "Create/update/list incidents with severity",
    },
    FeatureEntry {
        id: "work_management",
        name: "Work Management",
        category: FeatureCategory::BuiltIn,
        status: FeatureStatus::Partial,
        endpoints: &["/api/v1/work"],
        notes: "Work items linked to incidents/services",
    },
    FeatureEntry {
        id: "workflow_automation",
        name: "Workflow Automation",
        category: FeatureCategory::BuiltIn,
        status: FeatureStatus::Partial,
        endpoints: &["/api/v1/workflows"],
        notes: "Workflow definitions + run history",
    },
    FeatureEntry {
        id: "teams",
        name: "Teams",
        category: FeatureCategory::BuiltIn,
        status: FeatureStatus::Partial,
        endpoints: &["/api/v1/teams"],
        notes: "Team CRUD with member lists",
    },
    // —— Extensibility ——
    FeatureEntry {
        id: "opentelemetry",
        name: "OpenTelemetry",
        category: FeatureCategory::Extensibility,
        status: FeatureStatus::Done,
        endpoints: &["/v1/metrics", "/api/v1/apm/traces"],
        notes: "OTLP/HTTP JSON metrics + span ingest",
    },
    FeatureEntry {
        id: "integrations",
        name: "Integrations",
        category: FeatureCategory::Extensibility,
        status: FeatureStatus::Partial,
        endpoints: &["/api/v1/integrations"],
        notes: "Integration catalog with enable/disable",
    },
    FeatureEntry {
        id: "ide_plugins",
        name: "IDE Plugins",
        category: FeatureCategory::Extensibility,
        status: FeatureStatus::Planned,
        endpoints: &[],
        notes: "VS Code / JetBrains plugins not shipped",
    },
    FeatureEntry {
        id: "mcp_server",
        name: "MCP Server",
        category: FeatureCategory::Extensibility,
        status: FeatureStatus::Stub,
        endpoints: &["/api/v1/mcp/tools"],
        notes: "Lists Thine tools for MCP clients",
    },
    FeatureEntry {
        id: "agent_directory",
        name: "Agent Directory",
        category: FeatureCategory::Extensibility,
        status: FeatureStatus::Partial,
        endpoints: &["/api/v1/agents"],
        notes: "Registered agents with capabilities",
    },
    FeatureEntry {
        id: "thine_apps",
        name: "Thine Apps",
        category: FeatureCategory::Extensibility,
        status: FeatureStatus::Stub,
        endpoints: &["/api/v1/marketplace"],
        notes: "App marketplace listings",
    },
    FeatureEntry {
        id: "api",
        name: "API",
        category: FeatureCategory::Extensibility,
        status: FeatureStatus::Done,
        endpoints: &["/api/v1/features", "/health"],
        notes: "REST surface + feature catalog",
    },
    FeatureEntry {
        id: "marketplace",
        name: "Marketplace",
        category: FeatureCategory::Extensibility,
        status: FeatureStatus::Stub,
        endpoints: &["/api/v1/marketplace"],
        notes: "List/install app stubs",
    },
    FeatureEntry {
        id: "thine_cli",
        name: "Thine CLI (Pup)",
        category: FeatureCategory::Extensibility,
        status: FeatureStatus::Stub,
        endpoints: &["/api/v1/cli/info"],
        notes: "CLI metadata endpoint; binary TBD",
    },
    FeatureEntry {
        id: "software_catalog",
        name: "Software Catalog",
        category: FeatureCategory::Extensibility,
        status: FeatureStatus::Partial,
        endpoints: &["/api/v1/catalog/services"],
        notes: "Service entities with owners/teams",
    },
    FeatureEntry {
        id: "slos",
        name: "Service Level Objectives (SLOs)",
        category: FeatureCategory::Extensibility,
        status: FeatureStatus::Partial,
        endpoints: &["/api/v1/slos"],
        notes: "Metric SLOs with burn/status from store",
    },
    // —— Observability Infra ——
    FeatureEntry {
        id: "infra_monitoring",
        name: "Infrastructure Monitoring",
        category: FeatureCategory::ObservabilityInfra,
        status: FeatureStatus::Partial,
        endpoints: &["/api/v1/infra/hosts"],
        notes: "Host inventory from tags + seeded hosts",
    },
    FeatureEntry {
        id: "custom_metrics_obs",
        name: "Custom Metrics (Observability)",
        category: FeatureCategory::ObservabilityInfra,
        status: FeatureStatus::Done,
        endpoints: &["/api/v1/metrics", "/api/v1/metrics/summary"],
        notes: "Same metrics engine; granular summary",
    },
    FeatureEntry {
        id: "network_monitoring",
        name: "Network Monitoring",
        category: FeatureCategory::ObservabilityInfra,
        status: FeatureStatus::Stub,
        endpoints: &["/api/v1/network/flows"],
        notes: "Aggregated flow summaries",
    },
    FeatureEntry {
        id: "container_monitoring",
        name: "Container Monitoring",
        category: FeatureCategory::ObservabilityInfra,
        status: FeatureStatus::Stub,
        endpoints: &["/api/v1/containers"],
        notes: "Container inventory stub",
    },
    FeatureEntry {
        id: "k8s_autoscaling",
        name: "Kubernetes Autoscaling",
        category: FeatureCategory::ObservabilityInfra,
        status: FeatureStatus::Stub,
        endpoints: &["/api/v1/k8s/autoscalers"],
        notes: "HPA-like recommendations stub",
    },
    FeatureEntry {
        id: "serverless",
        name: "Serverless Monitoring",
        category: FeatureCategory::ObservabilityInfra,
        status: FeatureStatus::Stub,
        endpoints: &["/api/v1/serverless/functions"],
        notes: "Function inventory + cold starts",
    },
    FeatureEntry {
        id: "cloud_cost",
        name: "Cloud Cost Management",
        category: FeatureCategory::ObservabilityInfra,
        status: FeatureStatus::Stub,
        endpoints: &["/api/v1/cost/summary"],
        notes: "Synthetic cost by service",
    },
    FeatureEntry {
        id: "storage_mgmt",
        name: "Storage Management",
        category: FeatureCategory::ObservabilityInfra,
        status: FeatureStatus::Stub,
        endpoints: &["/api/v1/storage/volumes"],
        notes: "Volume usage stubs",
    },
    FeatureEntry {
        id: "gpu_monitoring",
        name: "GPU Monitoring",
        category: FeatureCategory::ObservabilityInfra,
        status: FeatureStatus::Stub,
        endpoints: &["/api/v1/gpu/devices"],
        notes: "GPU util/memory stubs",
    },
    // —— Applications ——
    FeatureEntry {
        id: "apm",
        name: "Application Performance Monitoring",
        category: FeatureCategory::Applications,
        status: FeatureStatus::Partial,
        endpoints: &["/api/v1/apm/traces", "/api/v1/apm/services"],
        notes: "Span ingest + service list; flamegraphs TBD",
    },
    FeatureEntry {
        id: "usm",
        name: "Universal Service Monitoring",
        category: FeatureCategory::Applications,
        status: FeatureStatus::Stub,
        endpoints: &["/api/v1/usm/services"],
        notes: "eBPF-less: derived from catalog + metrics",
    },
    FeatureEntry {
        id: "continuous_profiler",
        name: "Continuous Profiler",
        category: FeatureCategory::Applications,
        status: FeatureStatus::Stub,
        endpoints: &["/api/v1/profiler/profiles"],
        notes: "Profile metadata only",
    },
    FeatureEntry {
        id: "dynamic_instrumentation",
        name: "Dynamic Instrumentation",
        category: FeatureCategory::Applications,
        status: FeatureStatus::Planned,
        endpoints: &[],
        notes: "Requires agent instrumentation protocol",
    },
    FeatureEntry {
        id: "agent_observability",
        name: "Agent Observability",
        category: FeatureCategory::Applications,
        status: FeatureStatus::Stub,
        endpoints: &["/api/v1/fleet/agents"],
        notes: "Agent health via fleet endpoint",
    },
    // —— Data ——
    FeatureEntry {
        id: "database_monitoring",
        name: "Database Monitoring",
        category: FeatureCategory::Data,
        status: FeatureStatus::Stub,
        endpoints: &["/api/v1/dbm/instances"],
        notes: "DB instance stubs + query samples",
    },
    FeatureEntry {
        id: "data_observability",
        name: "Data Observability",
        category: FeatureCategory::Data,
        status: FeatureStatus::Stub,
        endpoints: &["/api/v1/data/assets"],
        notes: "Dataset asset registry",
    },
    FeatureEntry {
        id: "data_streams",
        name: "Data Streams Monitoring",
        category: FeatureCategory::Data,
        status: FeatureStatus::Stub,
        endpoints: &["/api/v1/streams"],
        notes: "Kafka-like stream lag stubs",
    },
    // —— Logs ——
    FeatureEntry {
        id: "log_management",
        name: "Log Management",
        category: FeatureCategory::Logs,
        status: FeatureStatus::Partial,
        endpoints: &["/api/v1/logs/ingest", "/api/v1/logs/search"],
        notes: "Ring-buffer ingest + filtered search",
    },
    FeatureEntry {
        id: "audit_trail",
        name: "Audit Trail",
        category: FeatureCategory::Logs,
        status: FeatureStatus::Partial,
        endpoints: &["/api/v1/audit"],
        notes: "Mutation audit log",
    },
    FeatureEntry {
        id: "obs_pipelines",
        name: "Observability Pipelines",
        category: FeatureCategory::Logs,
        status: FeatureStatus::Stub,
        endpoints: &["/api/v1/pipelines"],
        notes: "Pipeline definitions",
    },
    FeatureEntry {
        id: "error_tracking",
        name: "Error Tracking",
        category: FeatureCategory::Logs,
        status: FeatureStatus::Partial,
        endpoints: &["/api/v1/errors"],
        notes: "Error groups with counts",
    },
    FeatureEntry {
        id: "byoc_logs",
        name: "Bring Your Own Cloud Log Management",
        category: FeatureCategory::Logs,
        status: FeatureStatus::Planned,
        endpoints: &[],
        notes: "External object-store backends TBD",
    },
    FeatureEntry {
        id: "sensitive_data_scanner",
        name: "Sensitive Data Scanner",
        category: FeatureCategory::Logs,
        status: FeatureStatus::Stub,
        endpoints: &["/api/v1/sds/rules"],
        notes: "Regex rule stubs for PII patterns",
    },
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn catalog_covers_datadog_surface() {
        assert!(FEATURES.len() >= 45);
        let stats = feature_stats();
        assert_eq!(
            stats.total,
            stats.done + stats.partial + stats.stub + stats.planned
        );
        assert!(stats.done + stats.partial >= 15);
    }

    #[test]
    fn ids_unique() {
        let mut seen = std::collections::HashSet::new();
        for f in FEATURES {
            assert!(seen.insert(f.id), "duplicate {}", f.id);
        }
    }
}
