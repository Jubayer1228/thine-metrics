//! Datadog-parity platform modules: catalog, notebooks, SLOs, APM, logs, infra, …

mod catalog;
mod state;

pub use catalog::{
    all_features, feature_stats, FeatureCategory, FeatureEntry, FeatureStats, FeatureStatus,
};
pub use state::{
    AgentInfo, AuditEvent, CatalogService, ContainerInfo, CostSummary, CreateIncident,
    CreateNotebook, CreateSlo, CreateTeam, CreateWorkflow, DbInstance, DoraMetrics, ErrorGroup,
    FleetAgent, GpuDevice, HostInfo, Incident, Integration, LogEvent, MarketplaceApp, Notebook,
    NotebookCell, Pipeline, PlatformState, ProfileMeta, RbacRole, RbacUser, SdsRule, Slo,
    SloStatus, SpanRecord, StreamInfo, Team, VolumeInfo, WorkItem, Workflow, WorkflowRun,
};

#[cfg(test)]
mod tests {
    use super::*;
    use thine_storage::{MetricStore, StorageConfig};

    #[test]
    fn seed_and_list_core_modules() {
        let metrics = MetricStore::new(StorageConfig::default());
        let platform = PlatformState::new(metrics);
        platform.seed_demo();

        assert!(!platform.list_notebooks().is_empty());
        assert!(!platform.list_teams().is_empty());
        assert!(!platform.list_slos().is_empty());
        assert!(!platform.list_catalog().is_empty());
        assert!(!platform.list_integrations().is_empty());
        assert!(!platform.list_hosts().is_empty());
        assert!(!platform.list_incidents().is_empty());
        assert!(platform.search_logs(None, None, 10).len() >= 1);
        assert!(!platform.list_apm_services().is_empty());
        assert!(platform.dora().deploy_frequency_per_day > 0.0);
        assert!(!all_features().is_empty());
    }

    #[test]
    fn logs_and_traces_capped() {
        let metrics = MetricStore::new(StorageConfig::default());
        let platform = PlatformState::new(metrics);
        for i in 0..200 {
            platform.ingest_log(LogEvent {
                timestamp_ms: i,
                level: "info".into(),
                service: "api".into(),
                message: format!("msg {i}"),
                attrs: Default::default(),
            });
        }
        assert!(platform.search_logs(None, None, 10_000).len() <= 10_000);
    }
}
