//! Observability (18) surfaces aligned to Datadog docs crawl + llms.txt.
//! Focus: Cloudcraft diagrams/overlays, Live Processes, DBM explain/schema,
//! USM RED metrics, data lineage, cost recommendations.

use chrono::Utc;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::collections::{BTreeMap, BTreeSet};

use crate::state::PlatformState;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CloudResource {
    pub id: String,
    pub name: String,
    pub kind: String, // ec2 | rds | s3 | lambda | k8s_node | vpc | elb | redis
    pub provider: String,
    pub region: String,
    pub vpc: Option<String>,
    pub service: Option<String>,
    pub team: Option<String>,
    pub tags: BTreeMap<String, String>,
    pub cost_month: f64,
    pub agent_installed: bool,
    pub alert_status: String, // ok | warn | alert
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CloudcraftView {
    pub id: String,
    pub name: String,
    pub provider: String,
    pub regions: Vec<String>,
    pub group_by: Vec<String>,
    pub overlay: String,
    pub filter_tags: BTreeMap<String, String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LiveProcess {
    pub pid: u32,
    pub host: String,
    pub user: String,
    pub cmdline: String,
    pub cpu_pct: f64,
    pub mem_rss_mb: f64,
    pub service: Option<String>,
    #[serde(default)]
    pub command: String,
    #[serde(default)]
    pub env: String,
    #[serde(default)]
    pub team: String,
    #[serde(default)]
    pub az: String,
    #[serde(default)]
    pub started_ms: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExplainPlan {
    pub query_fingerprint: String,
    pub db: String,
    pub plan: serde_json::Value,
    pub total_cost: f64,
    pub estimated_rows: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DbSchemaTable {
    pub db: String,
    pub schema: String,
    pub table: String,
    pub columns: Vec<String>,
    pub indexes: Vec<String>,
    pub approx_rows: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DataLineageEdge {
    pub from_asset: String,
    pub to_asset: String,
    pub kind: String, // produces | consumes | transforms
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CostRecommendation {
    pub id: String,
    pub resource_id: String,
    pub title: String,
    pub savings_month: f64,
    pub rationale: String,
}

impl PlatformState {
    // —— Cloudcraft (docs: cloudcraft_in_datadog) ——
    pub fn upsert_cloud_resource(&self, r: CloudResource) -> CloudResource {
        self.cloud_resources.insert(r.id.clone(), r.clone());
        r
    }

    pub fn list_cloud_resources(&self) -> Vec<CloudResource> {
        self.cloud_resources.iter().map(|e| e.value().clone()).collect()
    }

    pub fn save_cloudcraft_view(&self, v: CloudcraftView) -> CloudcraftView {
        self.cloudcraft_views.insert(v.id.clone(), v.clone());
        v
    }

    pub fn list_cloudcraft_views(&self) -> Vec<CloudcraftView> {
        self.cloudcraft_views.iter().map(|e| e.value().clone()).collect()
    }

    /// Build architecture diagram JSON with Group By + overlay (Datadog Cloudcraft model).
    pub fn cloudcraft_diagram(
        &self,
        provider: Option<&str>,
        overlay: Option<&str>,
        group_by: &[String],
        q: Option<&str>,
    ) -> serde_json::Value {
        let overlay = overlay.unwrap_or("infrastructure");
        let mut resources: Vec<_> = self.list_cloud_resources();
        if let Some(p) = provider {
            resources.retain(|r| r.provider.eq_ignore_ascii_case(p));
        }
        if let Some(qq) = q {
            let qq = qq.to_ascii_lowercase();
            resources.retain(|r| {
                r.name.to_ascii_lowercase().contains(&qq)
                    || r.id.to_ascii_lowercase().contains(&qq)
                    || r.tags.values().any(|v| v.to_ascii_lowercase().contains(&qq))
            });
        }

        // Edges from network flows / service map
        let edges: Vec<_> = self
            .network_flows
            .read()
            .iter()
            .filter_map(|f| {
                Some(json!({
                    "from": f.get("src")?,
                    "to": f.get("dst")?,
                    "bytes": f.get("bytes")?,
                }))
            })
            .collect();

        let groups = group_resources(&resources, group_by);

        let nodes: Vec<_> = resources
            .iter()
            .map(|r| {
                // Always expose effectiveness fields; overlays add extra context.
                let mut node = json!({
                    "id": r.id,
                    "name": r.name,
                    "kind": r.kind,
                    "provider": r.provider,
                    "region": r.region,
                    "vpc": r.vpc,
                    "service": r.service,
                    "team": r.team,
                    "tags": r.tags,
                    "cost_month": r.cost_month,
                    "agent_installed": r.agent_installed,
                    "alert_status": r.alert_status,
                });
                // Overlays per Datadog docs
                match overlay {
                    "observability" => {
                        node["features"] = json!(["metrics", "logs", if r.agent_installed { "apm" } else { "" }]);
                    }
                    "cost" | "cloud_cost" => {
                        node["cost_focus"] = json!(true);
                    }
                    "security" => {
                        node["misconfigurations"] = json!(if r.kind == "s3" { 1 } else { 0 });
                        node["public_exposure"] = json!(r.kind == "elb" || r.kind == "s3");
                    }
                    "apm" => {
                        node["trace_links"] = json!(r.service.clone());
                    }
                    "monitors" => {
                        node["monitor_focus"] = json!(true);
                    }
                    _ => {}
                }
                node
            })
            .collect();

        json!({
            "product": "cloudcraft",
            "overlay": overlay,
            "group_by": group_by,
            "projection": "3d",
            "providers": ["aws", "azure", "gcp", "oci", "vsphere"],
            "resource_count": nodes.len(),
            "groups": groups,
            "nodes": nodes,
            "edges": edges,
            "overlays_available": ["infrastructure", "observability", "security", "cost", "apm", "monitors"],
            "docs_ref": "https://docs.datadoghq.com/datadog_cloudcraft.md"
        })
    }

    // —— Live Processes (infra docs) ——
    pub fn upsert_process(&self, p: LiveProcess) -> LiveProcess {
        let key = format!("{}:{}", p.host, p.pid);
        self.live_processes.insert(key, p.clone());
        p
    }

    pub fn list_live_processes(&self, host: Option<&str>, limit: usize) -> Vec<LiveProcess> {
        let mut v: Vec<_> = self
            .live_processes
            .iter()
            .map(|e| e.value().clone())
            .filter(|p| host.map(|h| p.host == h).unwrap_or(true))
            .collect();
        v.sort_by(|a, b| {
            b.cpu_pct
                .partial_cmp(&a.cpu_pct)
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        v.truncate(limit);
        v
    }

    /// Datadog Live Processes explorer — facets + enrichment for scatter/table.
    pub fn processes_explorer(
        &self,
        host: Option<&str>,
        user: Option<&str>,
        command: Option<&str>,
        service: Option<&str>,
        env: Option<&str>,
        q: Option<&str>,
        limit: usize,
    ) -> serde_json::Value {
        let q = q.map(|s| s.to_ascii_lowercase()).unwrap_or_default();
        let mut rows: Vec<_> = self
            .live_processes
            .iter()
            .map(|e| e.value().clone())
            .filter(|p| host.map(|h| p.host == h).unwrap_or(true))
            .filter(|p| user.map(|u| p.user == u).unwrap_or(true))
            .filter(|p| {
                command
                    .map(|c| {
                        let cmd = if p.command.is_empty() {
                            p.cmdline.split_whitespace().next().unwrap_or("")
                        } else {
                            p.command.as_str()
                        };
                        cmd == c || cmd.ends_with(c)
                    })
                    .unwrap_or(true)
            })
            .filter(|p| {
                service
                    .map(|s| p.service.as_deref() == Some(s))
                    .unwrap_or(true)
            })
            .filter(|p| env.map(|e| p.env == e).unwrap_or(true))
            .filter(|p| {
                if q.is_empty() {
                    return true;
                }
                let blob = format!(
                    "{} {} {} {} {} {}",
                    p.cmdline,
                    p.command,
                    p.host,
                    p.user,
                    p.service.as_deref().unwrap_or(""),
                    p.env
                )
                .to_ascii_lowercase();
                blob.contains(&q)
            })
            .collect();
        rows.sort_by(|a, b| {
            b.cpu_pct
                .partial_cmp(&a.cpu_pct)
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        let total = rows.len();
        rows.truncate(limit.max(1));

        let all: Vec<_> = self
            .live_processes
            .iter()
            .map(|e| e.value().clone())
            .collect();
        let command_facet = {
            let mut m: BTreeMap<String, u64> = BTreeMap::new();
            for p in &all {
                let cmd = if p.command.is_empty() {
                    p.cmdline
                        .split_whitespace()
                        .next()
                        .unwrap_or("unknown")
                        .to_string()
                } else {
                    p.command.clone()
                };
                let short = cmd.rsplit('/').next().unwrap_or(&cmd).to_string();
                *m.entry(short).or_insert(0) += 1;
            }
            facet_pairs(m)
        };
        let user_facet = facet_pairs({
            let mut m: BTreeMap<String, u64> = BTreeMap::new();
            for p in &all {
                *m.entry(p.user.clone()).or_insert(0) += 1;
            }
            m
        });
        let service_facet = facet_pairs({
            let mut m: BTreeMap<String, u64> = BTreeMap::new();
            for p in &all {
                *m.entry(p.service.clone().unwrap_or_else(|| "untagged".into()))
                    .or_insert(0) += 1;
            }
            m
        });
        let env_facet = facet_pairs({
            let mut m: BTreeMap<String, u64> = BTreeMap::new();
            for p in &all {
                *m.entry(if p.env.is_empty() {
                    "untagged".into()
                } else {
                    p.env.clone()
                })
                .or_insert(0) += 1;
            }
            m
        });
        let team_facet = facet_pairs({
            let mut m: BTreeMap<String, u64> = BTreeMap::new();
            for p in &all {
                *m.entry(if p.team.is_empty() {
                    "untagged".into()
                } else {
                    p.team.clone()
                })
                .or_insert(0) += 1;
            }
            m
        });
        let host_facet = facet_pairs({
            let mut m: BTreeMap<String, u64> = BTreeMap::new();
            for p in &all {
                *m.entry(p.host.clone()).or_insert(0) += 1;
            }
            m
        });

        let enriched: Vec<_> = rows
            .into_iter()
            .map(|p| {
                let command = if p.command.is_empty() {
                    p.cmdline
                        .split_whitespace()
                        .next()
                        .unwrap_or("unknown")
                        .rsplit('/')
                        .next()
                        .unwrap_or("unknown")
                        .to_string()
                } else {
                    p.command.clone()
                };
                let env = if p.env.is_empty() {
                    "prod".to_string()
                } else {
                    p.env.clone()
                };
                let team = if p.team.is_empty() {
                    "platform".to_string()
                } else {
                    p.team.clone()
                };
                json!({
                    "pid": p.pid,
                    "host": p.host,
                    "user": p.user,
                    "cmdline": p.cmdline,
                    "command": command,
                    "cpu_pct": p.cpu_pct,
                    "mem_rss_mb": p.mem_rss_mb,
                    "service": p.service,
                    "env": env,
                    "team": team,
                    "az": p.az,
                    "started_ms": p.started_ms,
                })
            })
            .collect();

        // Command groups for scatter (avg CPU / avg RSS / count)
        let mut groups: BTreeMap<String, (f64, f64, u64)> = BTreeMap::new();
        for e in &enriched {
            let cmd = e["command"].as_str().unwrap_or("unknown").to_string();
            let g = groups.entry(cmd).or_insert((0.0, 0.0, 0));
            g.0 += e["cpu_pct"].as_f64().unwrap_or(0.0);
            g.1 += e["mem_rss_mb"].as_f64().unwrap_or(0.0);
            g.2 += 1;
        }
        let command_groups: Vec<_> = groups
            .into_iter()
            .map(|(command, (cpu, mem, n))| {
                json!({
                    "command": command,
                    "count": n,
                    "avg_cpu_pct": cpu / n as f64,
                    "avg_mem_rss_mb": mem / n as f64,
                })
            })
            .collect();

        json!({
            "count": total,
            "returned": enriched.len(),
            "processes": enriched,
            "command_groups": command_groups,
            "facets": {
                "command": command_facet,
                "user": user_facet,
                "service": service_facet,
                "env": env_facet,
                "team": team_facet,
                "host": host_facet,
            },
            "significance": "Datadog Live Processes — 2s-resolution CPU/RSS, faceted search, scatter by command group"
        })
    }

    // —— USM RED metrics (universal_service_monitoring docs) ——
    pub fn usm_red_metrics(&self) -> Vec<serde_json::Value> {
        self.usm_discover()
            .into_iter()
            .map(|e| {
                json!({
                    "service": e.service,
                    "protocol": e.protocol,
                    "requests_per_sec": e.requests_per_sec, // Rate
                    "error_rate": e.error_rate,             // Errors
                    "duration_p95_ms": e.p95_latency_ms,    // Duration
                    "detected_by": e.detected_by,
                    "metric_names": [
                        format!("universal.http.server.hits{{service:{}}}", e.service),
                        format!("universal.http.server.errors{{service:{}}}", e.service),
                        format!("universal.http.server.duration.by.service_95p{{service:{}}}", e.service),
                    ]
                })
            })
            .collect()
    }

    // —— DBM explain + schema (database_monitoring docs) ——
    pub fn upsert_explain_plan(&self, plan: ExplainPlan) -> ExplainPlan {
        self.explain_plans
            .insert(plan.query_fingerprint.clone(), plan.clone());
        plan
    }

    pub fn list_explain_plans(&self) -> Vec<ExplainPlan> {
        self.explain_plans.iter().map(|e| e.value().clone()).collect()
    }

    pub fn upsert_schema_table(&self, t: DbSchemaTable) -> DbSchemaTable {
        let key = format!("{}.{}.{}", t.db, t.schema, t.table);
        self.db_schemas.insert(key, t.clone());
        t
    }

    pub fn list_schemas(&self, db: Option<&str>) -> Vec<DbSchemaTable> {
        self.db_schemas
            .iter()
            .map(|e| e.value().clone())
            .filter(|t| db.map(|d| t.db == d).unwrap_or(true))
            .collect()
    }

    pub fn dbm_apm_correlation(&self) -> serde_json::Value {
        // Correlate top queries with APM services (connect_dbm_and_apm)
        let queries = self.top_queries(10);
        let services = self.list_apm_services();
        json!({
            "mode": "full",
            "calling_services": services,
            "query_samples": queries.into_iter().map(|q| json!({
                "sql": q.sql,
                "db": q.db,
                "duration_ms": q.duration_ms,
                "calls": q.calls,
                "linked_service": services.first().cloned().unwrap_or_else(|| "api".into()),
            })).collect::<Vec<_>>(),
            "docs_ref": "https://docs.datadoghq.com/database_monitoring/connect_dbm_and_apm.md"
        })
    }

    // —— Data Observability lineage ——
    pub fn upsert_lineage(&self, edge: DataLineageEdge) {
        let key = format!("{}->{}:{}", edge.from_asset, edge.to_asset, edge.kind);
        self.data_lineage.insert(key, edge);
    }

    pub fn data_lineage_graph(&self) -> serde_json::Value {
        let edges: Vec<_> = self.data_lineage.iter().map(|e| e.value().clone()).collect();
        let mut nodes = BTreeSet::new();
        for e in &edges {
            nodes.insert(e.from_asset.clone());
            nodes.insert(e.to_asset.clone());
        }
        let assets = self.list_data_assets();
        json!({
            "nodes": nodes,
            "edges": edges,
            "assets": assets,
            "freshness_anomalies": assets.iter().filter(|a| {
                a.get("freshness_min").and_then(|v| v.as_u64()).unwrap_or(0) > 60
            }).count()
        })
    }

    // —— Cloud Cost recommendations (CCM overlay) ——
    pub fn cost_recommendations(&self) -> Vec<CostRecommendation> {
        let mut out = Vec::new();
        for r in self.list_cloud_resources() {
            if r.kind == "ec2" && r.cost_month > 80.0 {
                out.push(CostRecommendation {
                    id: format!("rec-{}", r.id),
                    resource_id: r.id.clone(),
                    title: format!("Rightsize underutilized {}", r.name),
                    savings_month: r.cost_month * 0.35,
                    rationale: "CPU p95 < 20% over 7d — consider smaller instance".into(),
                });
            }
            if r.kind == "s3" {
                out.push(CostRecommendation {
                    id: format!("rec-s3-{}", r.id),
                    resource_id: r.id.clone(),
                    title: format!("Enable lifecycle policy on {}", r.name),
                    savings_month: 12.0,
                    rationale: "Objects >30d not accessed — move to infrequent access".into(),
                });
            }
        }
        out.sort_by(|a, b| {
            b.savings_month
                .partial_cmp(&a.savings_month)
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        out
    }

    // —— Profiler flame summary ——
    pub fn profiler_flame_summary(&self, id: &str) -> Option<serde_json::Value> {
        let meta = self.profiles.get(id)?.clone();
        let blob_len = self
            .profile_blobs
            .get(id)
            .map(|b| b.len())
            .unwrap_or(0);
        Some(json!({
            "id": meta.id,
            "service": meta.service,
            "profile_type": meta.profile_type,
            "duration_ms": meta.duration_ms,
            "blob_bytes": blob_len,
            "top_frames": [
                {"frame": "runtime.main", "pct": 22.0},
                {"frame": "http.Handler.ServeHTTP", "pct": 18.5},
                {"frame": "database/sql.(*DB).Query", "pct": 14.2},
                {"frame": "encoding/json.Marshal", "pct": 9.1},
            ],
            "docs_ref": "https://docs.datadoghq.com/profiler/"
        }))
    }

    // —— Observability Pipelines worker status (docs) ——
    pub fn pipeline_workers(&self) -> serde_json::Value {
        let pipes = self.list_pipelines();
        json!({
            "workers": [
                {"id": "opw-1", "region": "us1", "status": "running", "pipelines": pipes.len(), "events_per_sec": 4200},
                {"id": "opw-2", "region": "eu1", "status": "running", "pipelines": pipes.len(), "events_per_sec": 3100},
            ],
            "destinations": ["thine_logs", "s3_archive", "splunk_hec"],
            "docs_ref": "https://docs.datadoghq.com/observability_pipelines/"
        })
    }

    pub fn seed_obs18(&self) {
        // Cloud resources for Cloudcraft
        for (id, name, kind, region, vpc, svc, cost, agent, alert) in [
            ("i-api-1", "api-asg-1", "ec2", "us-east-1", Some("vpc-prod"), Some("api"), 120.0, true, "ok"),
            ("i-api-2", "api-asg-2", "ec2", "us-east-1", Some("vpc-prod"), Some("api"), 118.0, true, "ok"),
            ("i-worker-1", "worker-1", "ec2", "us-east-1", Some("vpc-prod"), Some("worker"), 95.0, true, "warn"),
            ("i-worker-2", "worker-2", "ec2", "us-west-2", Some("vpc-west"), Some("worker"), 88.0, true, "ok"),
            ("rds-pg", "pg-primary", "rds", "us-east-1", Some("vpc-prod"), Some("postgres"), 210.0, false, "ok"),
            ("rds-pg-ro", "pg-replica", "rds", "us-east-1", Some("vpc-prod"), Some("postgres"), 160.0, false, "ok"),
            ("s3-logs", "thine-logs-archive", "s3", "us-east-1", None, None, 40.0, false, "ok"),
            ("s3-assets", "thine-static-assets", "s3", "us-east-1", None, None, 22.0, false, "ok"),
            ("lambda-checkout", "checkout", "lambda", "us-east-1", None, Some("checkout"), 28.0, false, "ok"),
            ("lambda-notify", "notify", "lambda", "us-east-1", None, Some("notify"), 18.0, false, "ok"),
            ("elb-public", "api-alb", "elb", "us-east-1", Some("vpc-prod"), Some("api"), 55.0, false, "alert"),
            ("redis-cache", "cache-1", "redis", "us-east-1", Some("vpc-prod"), Some("redis"), 70.0, false, "ok"),
            ("node-k8s-1", "k8s-node-1", "k8s_node", "us-east-1", Some("vpc-prod"), Some("kubernetes"), 110.0, true, "ok"),
            ("node-k8s-2", "k8s-node-2", "k8s_node", "us-east-1", Some("vpc-prod"), Some("kubernetes"), 105.0, true, "warn"),
            ("msk-1", "kafka-broker-1", "kafka", "us-east-1", Some("vpc-prod"), Some("kafka"), 145.0, false, "ok"),
        ] {
            let mut tags = BTreeMap::new();
            tags.insert("env".into(), "prod".into());
            if let Some(s) = svc {
                tags.insert("service".into(), s.into());
            }
            tags.insert("team".into(), "platform".into());
            self.upsert_cloud_resource(CloudResource {
                id: id.into(),
                name: name.into(),
                kind: kind.into(),
                provider: "aws".into(),
                region: region.into(),
                vpc: vpc.map(|s| s.into()),
                service: svc.map(|s| s.into()),
                team: Some("platform".into()),
                tags,
                cost_month: cost,
                agent_installed: agent,
                alert_status: alert.into(),
            });
        }

        self.save_cloudcraft_view(CloudcraftView {
            id: "prod-us".into(),
            name: "Production US".into(),
            provider: "aws".into(),
            regions: vec!["us-east-1".into()],
            group_by: vec!["region".into(), "vpc".into(), "service".into()],
            overlay: "observability".into(),
            filter_tags: BTreeMap::from([("env".into(), "prod".into())]),
        });
        self.save_cloudcraft_view(CloudcraftView {
            id: "cost-hotspots".into(),
            name: "Cost hotspots".into(),
            provider: "aws".into(),
            regions: vec!["us-east-1".into(), "us-west-2".into()],
            group_by: vec!["kind".into()],
            overlay: "cost".into(),
            filter_tags: BTreeMap::new(),
        });

        // Dense Live Processes fleet (Datadog Processes screenshot parity)
        let now = Utc::now().timestamp_millis();
        let templates: &[(&str, &str, &str, &str, &str, &str, f64, f64)] = &[
            ("java", "root", "java -Xmx2g -jar /opt/thine/api.jar", "api", "prod", "platform", 42.0, 1024.0),
            ("java", "root", "java com.datadog.demo.Application --spring.profiles.active=prod", "api", "prod", "platform", 38.5, 980.0),
            ("python", "thine", "python /opt/thine/worker/main.py --queues orders", "worker", "prod", "commerce", 22.0, 320.0),
            ("node", "thine", "node /srv/checkout/server.js", "checkout", "prod", "commerce", 18.4, 256.0),
            ("nginx", "root", "nginx: master process /usr/sbin/nginx", "edge", "prod", "platform", 2.1, 48.0),
            ("postgres", "postgres", "postgres: checkpointer", "postgres", "prod", "data", 6.5, 420.0),
            ("redis-server", "redis", "redis-server *:6379", "redis", "prod", "data", 3.2, 180.0),
            ("thine-agent", "root", "/opt/thine/bin/thine-agent run", "agent", "prod", "platform", 4.0, 128.0),
            ("kubelet", "root", "kubelet --config=/var/lib/kubelet/config.yaml", "kubernetes", "prod", "platform", 11.2, 640.0),
            ("containerd", "root", "/usr/bin/containerd", "kubernetes", "prod", "platform", 5.5, 210.0),
            ("dockerd", "root", "/usr/bin/dockerd -H fd://", "docker", "prod", "platform", 7.8, 290.0),
            ("sshd", "root", "sshd: /usr/sbin/sshd -D", "ssh", "prod", "platform", 0.2, 12.0),
            ("CRON", "root", "/usr/sbin/CRON -f", "cron", "prod", "platform", 0.1, 8.0),
            ("systemd", "root", "/usr/lib/systemd/systemd --system", "system", "prod", "platform", 0.8, 32.0),
            ("fluent-bit", "thine", "/opt/fluent-bit/bin/fluent-bit -c /etc/fb.conf", "logging", "prod", "observability", 3.5, 96.0),
            ("vector", "thine", "/usr/bin/vector --config /etc/vector.toml", "logging", "prod", "observability", 5.1, 140.0),
            ("envoy", "thine", "/usr/local/bin/envoy -c /etc/envoy.yaml", "mesh", "prod", "platform", 9.4, 220.0),
            ("api", "thine", "/usr/local/bin/api --port 8080", "api", "prod", "platform", 31.0, 480.0),
            ("worker", "thine", "/usr/local/bin/worker --queues payments", "worker", "prod", "commerce", 55.0, 720.0),
            ("checkout-handler", "thine", "checkout-handler --addr :9090", "checkout", "prod", "commerce", 14.0, 210.0),
            ("python", "thine", "python /opt/ml/infer.py", "ml-infer", "prod", "ml", 67.0, 1800.0),
            ("ruby", "thine", "puma -C /etc/puma.rb", "billing", "prod", "commerce", 12.0, 310.0),
            ("go", "thine", "/usr/local/bin/ingest --workers 8", "ingest", "prod", "data", 28.0, 540.0),
            ("java", "thine", "java -jar /opt/payments/payments.jar", "payments", "prod", "commerce", 25.0, 860.0),
            ("node", "thine", "node /srv/notify/index.js", "notify", "staging", "platform", 6.0, 120.0),
            ("python", "thine", "python manage.py runserver 0.0.0.0:8000", "admin", "dev", "platform", 4.5, 180.0),
            ("Explorer", "NT AUTHORITY\\SYSTEM", "C:\\Windows\\Explorer.EXE", "windows", "prod", "desktop", 1.2, 90.0),
            ("ApplicationFrameHost", "NT AUTHORITY\\SYSTEM", "ApplicationFrameHost.exe", "windows", "prod", "desktop", 0.4, 45.0),
            ("FAHClient", "thine", "FAHClient --config=/etc/fah", "folding", "dev", "ml", 88.0, 2100.0),
        ];
        let hosts = [
            ("i-api-1", "us-east-1a"),
            ("i-api-2", "us-east-1b"),
            ("i-worker-1", "us-east-1a"),
            ("i-worker-2", "us-east-1c"),
            ("k8s-node-1", "us-east-1a"),
            ("k8s-node-2", "us-east-1b"),
            ("i-ingest-1", "us-east-1a"),
            ("i-api-stg-1", "us-east-1a"),
        ];
        let mut pid: u32 = 1000;
        for (hi, (host, az)) in hosts.iter().enumerate() {
            for (ti, (command, user, cmdline, svc, env, team, cpu, mem)) in templates.iter().enumerate() {
                if (hi + ti) % 3 == 0 && ti > 20 {
                    continue; // thin some combos
                }
                pid += 17 + (ti as u32 % 5);
                let env = if host.contains("stg") {
                    "staging"
                } else if host.contains("dev") {
                    "dev"
                } else {
                    *env
                };
                let jitter = ((pid % 13) as f64) * 0.7;
                self.upsert_process(LiveProcess {
                    pid,
                    host: (*host).into(),
                    user: (*user).into(),
                    cmdline: (*cmdline).into(),
                    cpu_pct: (*cpu + jitter).min(99.0),
                    mem_rss_mb: *mem + (pid % 40) as f64,
                    service: Some((*svc).into()),
                    command: (*command).into(),
                    env: env.into(),
                    team: (*team).into(),
                    az: (*az).into(),
                    started_ms: now - (3_600_000 + (pid as i64 % 200) * 60_000),
                });
            }
        }

        self.upsert_explain_plan(ExplainPlan {
            query_fingerprint: "sel_orders_by_id".into(),
            db: "pg-primary".into(),
            plan: json!({
                "Node Type": "Index Scan",
                "Relation Name": "orders",
                "Index Name": "orders_pkey",
                "Startup Cost": 0.42,
                "Total Cost": 8.44,
                "Plan Rows": 1
            }),
            total_cost: 8.44,
            estimated_rows: 1,
        });
        self.upsert_explain_plan(ExplainPlan {
            query_fingerprint: "upd_inventory".into(),
            db: "pg-primary".into(),
            plan: json!({
                "Node Type": "ModifyTable",
                "Relation Name": "inventory",
                "Total Cost": 25.1,
                "Plans": [{"Node Type": "Seq Scan", "Relation Name": "inventory"}]
            }),
            total_cost: 25.1,
            estimated_rows: 1,
        });
        self.upsert_explain_plan(ExplainPlan {
            query_fingerprint: "sel_orders_join_users".into(),
            db: "pg-primary".into(),
            plan: json!({
                "Node Type": "Hash Join",
                "Total Cost": 1840.2,
                "Plan Rows": 12000,
                "Plans": [
                    {"Node Type": "Seq Scan", "Relation Name": "orders", "Total Cost": 920.0},
                    {"Node Type": "Hash", "Plans": [{"Node Type": "Seq Scan", "Relation Name": "users"}]}
                ]
            }),
            total_cost: 1840.2,
            estimated_rows: 12_000,
        });

        self.upsert_schema_table(DbSchemaTable {
            db: "pg-primary".into(),
            schema: "public".into(),
            table: "orders".into(),
            columns: vec!["id".into(), "user_id".into(), "amount".into(), "created_at".into()],
            indexes: vec!["orders_pkey".into(), "orders_user_id_idx".into()],
            approx_rows: 2_400_000,
        });
        self.upsert_schema_table(DbSchemaTable {
            db: "pg-primary".into(),
            schema: "public".into(),
            table: "inventory".into(),
            columns: vec!["sku".into(), "qty".into()],
            indexes: vec!["inventory_pkey".into()],
            approx_rows: 50_000,
        });
        self.upsert_schema_table(DbSchemaTable {
            db: "pg-primary".into(),
            schema: "public".into(),
            table: "users".into(),
            columns: vec!["id".into(), "email".into(), "created_at".into()],
            indexes: vec!["users_pkey".into(), "users_email_uidx".into()],
            approx_rows: 890_000,
        });
        self.upsert_schema_table(DbSchemaTable {
            db: "pg-primary".into(),
            schema: "public".into(),
            table: "payments".into(),
            columns: vec!["id".into(), "order_id".into(), "status".into(), "amount".into()],
            indexes: vec!["payments_pkey".into(), "payments_order_id_idx".into()],
            approx_rows: 1_100_000,
        });

        self.upsert_lineage(DataLineageEdge {
            from_asset: "orders_fact".into(),
            to_asset: "revenue_dashboard".into(),
            kind: "produces".into(),
        });
        self.upsert_lineage(DataLineageEdge {
            from_asset: "kafka:orders".into(),
            to_asset: "orders_fact".into(),
            kind: "transforms".into(),
        });
        self.upsert_lineage(DataLineageEdge {
            from_asset: "kafka:payments".into(),
            to_asset: "payments_fact".into(),
            kind: "transforms".into(),
        });
        self.upsert_lineage(DataLineageEdge {
            from_asset: "payments_fact".into(),
            to_asset: "fraud_model".into(),
            kind: "consumes".into(),
        });
        self.upsert_lineage(DataLineageEdge {
            from_asset: "orders_fact".into(),
            to_asset: "sla_report".into(),
            kind: "produces".into(),
        });

        // Extra effectiveness demos: APM latency spread, logs, errors, DB samples, streams
        let now = Utc::now().timestamp_millis();
        let mut spans = Vec::new();
        for i in 0..220 {
            let svc = ["api", "worker", "checkout", "postgres", "redis", "ingest"][i % 6];
            let dur = match svc {
                "postgres" => 8.0 + (i % 7) as f64,
                "redis" => 1.5 + (i % 3) as f64 * 0.4,
                "checkout" => 90.0 + (i % 11) as f64 * 12.0 + if i % 29 == 0 { 400.0 } else { 0.0 },
                "worker" => 40.0 + (i % 9) as f64 * 5.0,
                "ingest" => 22.0 + (i % 8) as f64 * 3.0,
                _ => 35.0 + (i % 13) as f64 * 8.0 + if i % 23 == 0 { 280.0 } else { 0.0 },
            };
            spans.push(crate::state::SpanRecord {
                trace_id: format!("t-demo-{}", i / 4),
                span_id: format!("s-{i}"),
                parent_span_id: if i % 4 != 0 {
                    Some(format!("s-{}", i - (i % 4)))
                } else {
                    None
                },
                service: svc.into(),
                name: if svc == "postgres" {
                    "db.query".into()
                } else {
                    "http.request".into()
                },
                duration_ms: dur,
                timestamp_ms: now - (220 - i) as i64 * 1_500,
                status: if i % 14 == 0 { "error".into() } else { "ok".into() },
                resource: Some(match svc {
                    "api" => "/v1/orders".into(),
                    "checkout" => "/checkout/pay".into(),
                    "postgres" => "SELECT orders".into(),
                    _ => svc.into(),
                }),
            });
        }
        self.ingest_spans(spans);

        for i in 0..25 {
            let level = if i % 9 == 0 {
                "error"
            } else if i % 5 == 0 {
                "warn"
            } else {
                "info"
            };
            self.ingest_log(crate::state::LogEvent {
                timestamp_ms: now - i as i64 * 3_000,
                level: level.into(),
                service: ["api", "worker", "checkout"][i % 3].into(),
                message: if level == "error" {
                    format!("upstream timeout talking to payments id={i}")
                } else if level == "warn" {
                    format!("retrying checkout for cart={i} email=user{i}@example.com")
                } else {
                    format!("handled request route=/v1/orders status=200 latency_ms={}", 40 + i)
                },
                attrs: BTreeMap::from([("env".into(), "prod".into())]),
            });
        }

        self.add_query_sample(crate::models_ext::DbQuerySample {
            sql: "SELECT o.*, u.email FROM orders o JOIN users u ON u.id = o.user_id WHERE o.created_at > $1".into(),
            duration_ms: 240.0,
            calls: 8_200,
            db: "pg-primary".into(),
        });
        self.add_query_sample(crate::models_ext::DbQuerySample {
            sql: "SELECT sku, qty FROM inventory WHERE qty < 10".into(),
            duration_ms: 55.0,
            calls: 15_000,
            db: "pg-primary".into(),
        });

        self.upsert_stream(crate::state::StreamInfo {
            name: "orders".into(),
            lag: 128,
            throughput: 2100.0,
        });
        self.upsert_stream(crate::state::StreamInfo {
            name: "payments".into(),
            lag: 14,
            throughput: 980.0,
        });
        self.upsert_stream(crate::state::StreamInfo {
            name: "notifications".into(),
            lag: 3,
            throughput: 450.0,
        });

        let _ = self.ingest_flows(vec![
            crate::models_ext::NetworkFlow {
                id: "demo-f1".into(),
                src: "api".into(),
                dst: "postgres".into(),
                protocol: "tcp".into(),
                bytes: 4_200_000,
                packets: 28_000,
                timestamp_ms: now,
            },
            crate::models_ext::NetworkFlow {
                id: "demo-f2".into(),
                src: "api".into(),
                dst: "redis".into(),
                protocol: "tcp".into(),
                bytes: 900_000,
                packets: 40_000,
                timestamp_ms: now,
            },
            crate::models_ext::NetworkFlow {
                id: "demo-f3".into(),
                src: "checkout".into(),
                dst: "api".into(),
                protocol: "tcp".into(),
                bytes: 1_100_000,
                packets: 9_000,
                timestamp_ms: now,
            },
            crate::models_ext::NetworkFlow {
                id: "demo-f4".into(),
                src: "worker".into(),
                dst: "kafka".into(),
                protocol: "tcp".into(),
                bytes: 6_500_000,
                packets: 22_000,
                timestamp_ms: now,
            },
        ]);

        let _ = self.usm_discover();
    }
}

fn facet_pairs(map: BTreeMap<String, u64>) -> Vec<serde_json::Value> {
    let mut v: Vec<_> = map
        .into_iter()
        .map(|(value, count)| json!({ "value": value, "count": count }))
        .collect();
    v.sort_by(|a, b| {
        b["count"]
            .as_u64()
            .unwrap_or(0)
            .cmp(&a["count"].as_u64().unwrap_or(0))
    });
    v
}

fn group_resources(resources: &[CloudResource], group_by: &[String]) -> serde_json::Value {
    if group_by.is_empty() {
        return json!([]);
    }
    let key_fn = |r: &CloudResource, g: &str| -> String {
        match g {
            "region" => r.region.clone(),
            "vpc" => r.vpc.clone().unwrap_or_else(|| "none".into()),
            "service" => r.service.clone().unwrap_or_else(|| "untagged".into()),
            "team" => r.team.clone().unwrap_or_else(|| "untagged".into()),
            "kind" => r.kind.clone(),
            "provider" => r.provider.clone(),
            _ => r.tags.get(g).cloned().unwrap_or_else(|| "untagged".into()),
        }
    };
    let mut map: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for r in resources {
        let key = group_by
            .iter()
            .map(|g| key_fn(r, g))
            .collect::<Vec<_>>()
            .join("/");
        map.entry(key).or_default().push(r.id.clone());
    }
    json!(map
        .into_iter()
        .map(|(k, ids)| json!({ "group": k, "resource_ids": ids, "count": ids.len() }))
        .collect::<Vec<_>>())
}
