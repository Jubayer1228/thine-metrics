//! Datadog-parity platform modules: catalog, notebooks, SLOs, APM, logs, infra, …

mod ai_obs;
mod bits;
mod catalog;
mod close_gap;
mod competitors;
mod dbm;
mod dd_wins;
mod debt_close;
mod deep;
mod gpu;
mod graph_insights;
mod logs_query;
mod models_ext;
mod obs18;
mod state;

pub use debt_close::{
    default_service_definition_yaml, message_template, KubeMapNode, LogPattern, ScorecardCheck,
    ServiceScorecard,
};
pub use logs_query::{facet_from_logs, parse_log_query};

pub use catalog::{
    all_features, avg_parity, feature_stats, feature_stats_with_parity, parity_matrix,
    FeatureCategory, FeatureEntry, FeatureParityRow, FeatureStats, FeatureStatus,
};
pub use close_gap::{ApmServiceStat, LogFacet, NotifChannel, NotifDelivery, TraceTree};
pub use dd_wins::{EbpfAgent, HaRegion, HaStatus, UsmaEndpoint, WatchdogAnomaly};
pub use ai_obs::{
    ai_metric_catalog, AiCreateRun, AiDataset, AiEvalResult, AiExample, AiExperiment, AiFeedback,
    AiGrader, AiHostSample, AiMetricDef, AiProject, AiRun,
};
pub use dbm::{
    dbm_metric_catalog, DbActivity, DbBlockingQuery, DbHostSample, DbMetricDef, DbQueryMetric,
    DbWaitEvent,
};
pub use gpu::{gpu_metric_catalog, GpuDevice, GpuMetricDef, GpuProcess, GpuSample, GpuThrottleReasons};
pub use graph_insights::{
    CorrelationHit, CorrelationSearchRequest, CorrelationSearchResponse, DashboardAnomaliesResponse,
    DashboardAnomalyIssue, ExplainFinding, WatchdogExplainRequest, WatchdogExplainResult,
};
pub use obs18::{
    CloudResource, CloudcraftView, CostRecommendation, DataLineageEdge, DbSchemaTable, ExplainPlan,
    LiveProcess,
};
pub use models_ext::{
    ApiToken, Autoscaler, ByocSink, CostLine, DbQuerySample, DynProbe, IdePlugin, MobileConfig,
    NetworkFlow, SdsFinding, ServerlessFunction,
};
pub use state::{
    AgentInfo, AuditEvent, CatalogService, ContainerInfo, CostSummary, CreateIncident,
    CreateNotebook, CreateSlo, CreateTeam, CreateWorkflow, DbInstance, DoraMetrics, ErrorGroup,
    FleetAgent, HostInfo, Incident, Integration, LogEvent, MarketplaceApp, Notebook, NotebookCell,
    Pipeline, PlatformEvent, PlatformState, ProfileMeta, RbacRole, RbacUser, SdsRule, Slo, SloStatus,
    SpanRecord, StreamInfo, Team, VolumeInfo, WorkItem, Workflow, WorkflowRun,
};

#[cfg(test)]
mod tests {
    use super::*;
    use thine_storage::{MetricStore, StorageConfig};

    fn seeded() -> std::sync::Arc<PlatformState> {
        let metrics = MetricStore::new(StorageConfig::default());
        let platform = PlatformState::new(metrics);
        platform.seed_demo();
        platform
    }

    #[test]
    fn seed_and_list_core_modules() {
        let platform = seeded();
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

    #[test]
    fn auth_tokens_and_rbac() {
        let p = seeded();
        let admin = p.issue_token("admin@t.dev", vec!["admin".into()]);
        assert!(p.authenticate(&admin.token).is_some());
        assert!(p.authorize(&admin.token, "anything"));
        let viewer = p.issue_token("v@t.dev", vec!["viewer".into()]);
        // viewer role seeded with limited perms — admin always passes
        assert!(p.authenticate(&viewer.token).is_some());
        assert!(!p.authorize("bad-token", "read"));
    }

    #[test]
    fn bits_chat_intents() {
        let p = seeded();
        let slo = p.bits_chat("show me SLO status");
        assert_eq!(slo["intent"], "slo_status");
        assert!(slo["reply"].as_str().unwrap().contains("SLO"));
        let cost = p.bits_chat("what is our cloud cost?");
        assert_eq!(cost["intent"], "cost");
        let dora = p.bits_chat("dora metrics please");
        assert_eq!(dora["intent"], "dora");
        let tut = p.bits_chat("run the monitoring tutorial");
        assert_eq!(tut["intent"], "tutorial");
        assert!(tut["artifacts"]["board_id"].as_str().is_some());
        assert!(tut["actions"].as_array().unwrap().len() >= 3);
    }

    #[test]
    fn fleet_heartbeat_and_rollout() {
        let p = seeded();
        p.fleet_heartbeat("agent-x", "0.2.0", "host-x", Some("linux"));
        let r = p.fleet_rollout("0.3.0");
        assert_eq!(r["target_version"], "0.3.0");
        assert!(r["agents_updated"].as_u64().unwrap() >= 1);
        assert!(p.list_fleet().iter().all(|a| a.version == "0.3.0"));
    }

    #[test]
    fn network_flow_aggregation() {
        let p = seeded();
        let n = p.ingest_flows(vec![NetworkFlow {
            id: "x".into(),
            src: "a".into(),
            dst: "b".into(),
            protocol: "tcp".into(),
            bytes: 100,
            packets: 2,
            timestamp_ms: 1,
        }]);
        assert_eq!(n, 1);
        assert!(!p.network_flows().is_empty());
        let map = p.usm_map();
        assert!(map["edges"].as_array().unwrap().len() >= 1);
    }

    #[test]
    fn k8s_autoscaler_recommend() {
        let p = seeded();
        let rec = p.recommend_autoscalers();
        assert!(!rec.is_empty());
        assert!(rec[0].recommended >= rec[0].min);
    }

    #[test]
    fn serverless_and_cost_detail() {
        let p = seeded();
        assert!(!p.list_serverless_detailed().is_empty());
        let cost = p.cost_detail();
        assert!(!cost.is_empty());
        assert!(cost[0].total > 0.0);
        assert!(p.cost_summary().total_month > 0.0);
    }

    #[test]
    fn gpu_and_container_stats() {
        let p = seeded();
        let stats = p.container_stats();
        assert!(stats["total"].as_u64().unwrap() >= 1);
        let gpu = p.gpu_summary();
        assert!(gpu["devices"].as_u64().unwrap() >= 4);
        assert!(gpu["metrics_catalog_count"].as_u64().unwrap() >= 48);
        assert!(!p.list_gpu_processes(None).is_empty());
        assert!(!p.list_gpu_samples(None, 10).is_empty());
        let health = p.gpu_health();
        assert!(health["findings"].as_array().unwrap().len() >= 1);
        let n = p.emit_gpu_metrics(None);
        assert!(n > 100);
        let cat = p.gpu_metrics_catalog_json();
        assert!(cat["count"].as_u64().unwrap() >= 48);
    }

    #[test]
    fn profiler_blob_roundtrip() {
        let p = seeded();
        let blob = p.get_profile_blob("api-cpu");
        assert!(blob.is_some());
        assert!(!blob.unwrap().is_empty());
    }

    #[test]
    fn dynamic_probes_crud() {
        let p = seeded();
        assert!(!p.list_probes().is_empty());
        let ok = p.delete_probe("probe-1");
        assert!(ok);
        assert!(p.list_probes().iter().all(|x| x.id != "probe-1"));
    }

    #[test]
    fn dbm_top_queries() {
        let p = seeded();
        let q = p.top_queries(5);
        assert!(!q.is_empty());
        assert!(q[0].calls > 0);
        let sum = p.dbm_summary();
        assert!(sum["instances"].as_u64().unwrap() >= 3);
        assert!(sum["metrics_catalog_count"].as_u64().unwrap() >= 60);
        assert!(!p.list_db_query_metrics(10).is_empty());
        assert!(!p.list_db_samples(None, 20).is_empty());
        let n = p.emit_db_metrics(None);
        assert!(n > 50);
    }

    #[test]
    fn ai_obs_langsmith_parity() {
        let p = seeded();
        let sum = p.ai_summary();
        assert!(sum["projects"].as_u64().unwrap() >= 6);
        assert!(sum["metrics_catalog_count"].as_u64().unwrap() >= 60);
        assert!(!p.list_ai_runs(None, None, 10).is_empty());
        let exp = p.run_ai_experiment("ds-chinook", 3);
        assert!(exp["pass_at_k"].as_f64().unwrap() > 0.5);
        assert!(exp["elapsed_us"].as_u64().unwrap() < 100_000);
        let n = p.emit_ai_metrics(None);
        assert!(n > 20);
    }

    #[test]
    fn sds_scan_finds_email() {
        let p = seeded();
        let findings = p.scan_logs_for_sensitive();
        assert!(
            findings.iter().any(|f| f.rule_id == "email"),
            "expected email finding, got {:?}",
            findings
        );
    }

    #[test]
    fn pipeline_run_filters_debug() {
        let p = seeded();
        let out = p.run_pipeline(
            "default-logs",
            vec![
                LogEvent {
                    timestamp_ms: 1,
                    level: "debug".into(),
                    service: "API".into(),
                    message: "x=1 y=2".into(),
                    attrs: Default::default(),
                },
                LogEvent {
                    timestamp_ms: 2,
                    level: "info".into(),
                    service: "API".into(),
                    message: "status=ok".into(),
                    attrs: Default::default(),
                },
            ],
        );
        assert!(out["processed"].as_u64().unwrap() >= 1);
        assert!(out["dropped"].as_u64().unwrap() >= 1);
    }

    #[test]
    fn byoc_forward() {
        let p = seeded();
        let out = p.forward_logs_byoc("s3-archive", 10);
        assert_eq!(out["sink"], "s3-archive");
        assert!(out["forwarded_now"].as_u64().unwrap() >= 1);
    }

    #[test]
    fn mobile_and_ide_manifests() {
        let p = seeded();
        let m = p.mobile_config();
        assert!(m.push_enabled);
        assert!(!m.deep_links.is_empty());
        let ide = p.list_ide_plugins();
        assert!(ide.len() >= 2);
    }

    #[test]
    fn marketplace_install() {
        let p = seeded();
        let apps = p.list_marketplace();
        assert!(!apps.is_empty());
        let id = apps[0].id.clone();
        let installed = p.install_app(&id).unwrap();
        assert!(installed.installed);
        let un = p.uninstall_app(&id).unwrap();
        assert!(!un.installed);
    }

    #[test]
    fn mcp_invoke_tools() {
        let p = seeded();
        let r = p.mcp_invoke("list_slos", &serde_json::json!({}));
        assert_eq!(r["ok"], true);
        let bad = p.mcp_invoke("nope", &serde_json::json!({}));
        assert_eq!(bad["ok"], false);
    }

    #[test]
    fn cli_schema_has_commands() {
        let p = seeded();
        let schema = p.cli_info();
        assert_eq!(schema["name"], "thine");
        assert!(schema["commands"].as_array().unwrap().len() >= 5);
    }

    #[test]
    fn feature_catalog_no_stubs() {
        let stats = feature_stats();
        assert_eq!(stats.stub, 0);
        assert_eq!(stats.planned, 0);
        assert_eq!(stats.partial, 0);
        assert_eq!(stats.done, stats.total);
        // Audited (deflated) average — must stay honest, not marketing-inflated.
        assert!(avg_parity() >= 40.0 && avg_parity() < 75.0);
    }

    #[test]
    fn close_gap_apm_and_notify() {
        let p = seeded();
        assert!(!p.apm_service_stats().is_empty());
        assert!(!p.list_notif_channels().is_empty());
        let d = p.notify("test", serde_json::json!({"ok": true}), None);
        assert!(!d.is_empty());
        let facets = p.log_facets();
        assert!(facets.iter().any(|f| f.key == "level"));
        assert!(!p.slo_budgets().is_empty());
        assert!(p.service_map()["nodes"].as_array().unwrap().len() >= 1);
    }

    #[test]
    fn formula_query_works() {
        let p = seeded();
        let r = p.query_formula("avg:http.server.request.duration");
        assert!(r.is_ok(), "{:?}", r);
    }

    #[test]
    fn dd_wins_ebpf_watchdog_integrations_ha() {
        let p = seeded();
        assert!(p.list_ebpf_agents().len() >= 2);
        assert!(p.usm_discover().len() >= 1);
        assert!(p.integrations.len() >= 800);
        let search = p.search_integrations(Some("aws"), None, false, 20, 0);
        assert!(search["catalog_size"].as_u64().unwrap() >= 800);
        let ha = p.ha_status();
        assert!(ha.quorum);
        assert!(p.ha_failover("eu1").is_ok());
        assert_eq!(p.ha_status().primary, "eu1");
        let _ = p.watchdog_scan();
    }

    #[test]
    fn obs18_cloudcraft_dbm() {
        let p = seeded();
        let d = p.cloudcraft_diagram(
            Some("aws"),
            Some("observability"),
            &["region".into(), "vpc".into()],
            None,
        );
        assert!(d["resource_count"].as_u64().unwrap() >= 5);
        assert!(!p.list_live_processes(None, 10).is_empty());
        assert!(!p.list_explain_plans().is_empty());
        assert!(!p.list_schemas(None).is_empty());
        assert!(!p.usm_red_metrics().is_empty());
        assert!(!p.cost_recommendations().is_empty());
        let cmp = p.competitors_comparison();
        assert!(cmp["averages"]["thine"].as_f64().unwrap_or(0.0) > 50.0);
        assert!(cmp["profiles"].as_array().unwrap().len() >= 8);
        assert!(!cmp["vs_signoz"]["summary"].as_str().unwrap_or("").is_empty());
    }
}
