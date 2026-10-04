//! Deeper Datadog-parity behaviors layered on PlatformState.

use chrono::Utc;
use serde_json::json;
use std::collections::{BTreeMap, BTreeSet};
use uuid::Uuid;

use crate::models_ext::{
    ApiToken, Autoscaler, ByocSink, CostLine, DbQuerySample, DynProbe, IdePlugin, MobileConfig,
    NetworkFlow, SdsFinding, ServerlessFunction,
};
use crate::state::{
    AgentInfo, ContainerInfo, FleetAgent, HostInfo, LogEvent, MarketplaceApp, PlatformState, Pipeline,
    ProfileMeta, SdsRule, StreamInfo, VolumeInfo,
};
use thine_common::{MetricPoint, MetricType, Sample, Tags};

impl PlatformState {
    // —— Auth tokens ——
    pub fn issue_token(&self, email: &str, roles: Vec<String>) -> ApiToken {
        let token = format!("thine_{}", Uuid::new_v4().simple());
        let t = ApiToken {
            token: token.clone(),
            user_email: email.into(),
            roles,
            created_at_ms: Utc::now().timestamp_millis(),
        };
        self.tokens.insert(token, t.clone());
        self.audit("issue_token", &format!("user:{email}"), email);
        t
    }

    pub fn authenticate(&self, token: &str) -> Option<ApiToken> {
        self.tokens.get(token).map(|e| e.value().clone())
    }

    pub fn authorize(&self, token: &str, permission: &str) -> bool {
        let Some(t) = self.authenticate(token) else {
            return false;
        };
        if t.roles.iter().any(|r| r == "admin") {
            return true;
        }
        for role_name in &t.roles {
            if let Some(role) = self.roles.iter().find(|r| r.name == *role_name) {
                if role
                    .permissions
                    .iter()
                    .any(|p| p == "*" || p == permission)
                {
                    return true;
                }
            }
        }
        false
    }

    // Bits AI implemented in bits.rs (agentic tutorial + RCA)

    // —— Agents CRUD ——
    pub fn upsert_agent(&self, agent: AgentInfo) -> AgentInfo {
        self.agents.insert(agent.id.clone(), agent.clone());
        self.audit("upsert", &format!("agent:{}", agent.id), "system");
        agent
    }

    pub fn delete_agent(&self, id: &str) -> bool {
        self.agents.remove(id).is_some()
    }

    // —— Fleet ——
    pub fn fleet_heartbeat(
        &self,
        id: &str,
        version: &str,
        host: &str,
        platform: Option<&str>,
    ) -> FleetAgent {
        let now = Utc::now().timestamp_millis();
        let prev = self.fleet.get(id).map(|e| e.clone());
        let agent = FleetAgent {
            id: id.into(),
            version: version.into(),
            host: host.into(),
            status: "healthy".into(),
            platform: platform
                .map(|p| p.to_string())
                .or_else(|| prev.as_ref().map(|p| p.platform.clone()))
                .unwrap_or_else(|| "linux".into()),
            last_seen_ms: now,
            config_profile: prev
                .as_ref()
                .map(|p| p.config_profile.clone())
                .unwrap_or_else(|| "none".into()),
            checks: prev
                .as_ref()
                .map(|p| p.checks.clone())
                .unwrap_or_default(),
            metrics_enabled: prev.as_ref().map(|p| p.metrics_enabled).unwrap_or(true),
            logs_enabled: prev.as_ref().map(|p| p.logs_enabled).unwrap_or(false),
            apm_enabled: prev.as_ref().map(|p| p.apm_enabled).unwrap_or(false),
        };
        self.fleet.insert(id.into(), agent.clone());
        agent
    }

    pub fn fleet_configure(
        &self,
        profile: &str,
        agent_ids: Option<&[String]>,
    ) -> serde_json::Value {
        let checks = match profile {
            "apm" => vec![
                "cpu".into(),
                "memory".into(),
                "disk".into(),
                "network".into(),
                "apm".into(),
            ],
            "logs" => vec!["cpu".into(), "memory".into(), "logs".into()],
            _ => vec![
                "cpu".into(),
                "memory".into(),
                "disk".into(),
                "network".into(),
                "process".into(),
            ],
        };
        let metrics = true;
        let logs = profile == "logs" || profile == "standard" || profile == "full";
        let apm = profile == "apm" || profile == "full" || profile == "standard";
        let mut updated = 0u32;
        for mut e in self.fleet.iter_mut() {
            if let Some(ids) = agent_ids {
                if !ids.is_empty() && !ids.iter().any(|id| id == &e.id) {
                    continue;
                }
            }
            e.config_profile = profile.into();
            e.checks = checks.clone();
            e.metrics_enabled = metrics;
            e.logs_enabled = logs;
            e.apm_enabled = apm;
            e.status = "healthy".into();
            e.last_seen_ms = Utc::now().timestamp_millis();
            updated += 1;
        }
        json!({
            "profile": profile,
            "agents_updated": updated,
            "checks": checks,
            "metrics_enabled": metrics,
            "logs_enabled": logs,
            "apm_enabled": apm,
        })
    }

    pub fn fleet_rollout(&self, target_version: &str) -> serde_json::Value {
        let mut updated = 0u32;
        for mut e in self.fleet.iter_mut() {
            e.version = target_version.into();
            e.status = "updating".into();
            e.last_seen_ms = Utc::now().timestamp_millis();
            updated += 1;
        }
        // mark healthy after "rollout"
        for mut e in self.fleet.iter_mut() {
            e.status = "healthy".into();
            e.version = target_version.into();
        }
        json!({ "target_version": target_version, "agents_updated": updated })
    }

    pub fn fleet_summary(&self) -> serde_json::Value {
        let agents = self.list_fleet();
        let healthy = agents.iter().filter(|a| a.status == "healthy").count();
        let configured = agents
            .iter()
            .filter(|a| a.config_profile != "none" && !a.config_profile.is_empty())
            .count();
        json!({
            "total": agents.len(),
            "healthy": healthy,
            "configured": configured,
            "install_pct": if agents.is_empty() { 0 } else { 100 },
            "platforms": {
                "linux": agents.iter().filter(|a| a.platform == "linux").count(),
                "docker": agents.iter().filter(|a| a.platform == "docker").count(),
                "kubernetes": agents.iter().filter(|a| a.platform == "kubernetes").count(),
                "macos": agents.iter().filter(|a| a.platform == "macos").count(),
                "windows": agents.iter().filter(|a| a.platform == "windows").count(),
                "other": agents.iter().filter(|a| {
                    !matches!(a.platform.as_str(), "linux"|"docker"|"kubernetes"|"macos"|"windows")
                }).count(),
            }
        })
    }

    // —— Network ——
    pub fn ingest_flows(&self, flows: Vec<NetworkFlow>) -> usize {
        let n = flows.len();
        let mut q = self.flow_records.write();
        for f in flows {
            q.push_back(f);
        }
        while q.len() > 5_000 {
            q.pop_front();
        }
        // refresh summary cache
        let mut agg: BTreeMap<(String, String, String), (u64, u64)> = BTreeMap::new();
        for f in q.iter() {
            let key = (f.src.clone(), f.dst.clone(), f.protocol.clone());
            let e = agg.entry(key).or_insert((0, 0));
            e.0 += f.bytes;
            e.1 += f.packets;
        }
        *self.network_flows.write() = agg
            .into_iter()
            .map(|((src, dst, protocol), (bytes, packets))| {
                json!({ "src": src, "dst": dst, "protocol": protocol, "bytes": bytes, "packets": packets })
            })
            .collect();
        n
    }

    // —— Containers ——
    pub fn upsert_container(&self, c: ContainerInfo) -> ContainerInfo {
        self.containers.insert(c.id.clone(), c.clone());
        c
    }

    pub fn container_stats(&self) -> serde_json::Value {
        let all = self.list_containers();
        let running = all.iter().filter(|c| c.status == "running").count();
        let k8s = all.iter().filter(|c| c.kube_namespace.is_some()).count();
        let by_env = facet_count(all.iter().map(|c| c.env.clone().if_empty("untagged")));
        let by_ns = facet_count(all.iter().filter_map(|c| c.kube_namespace.clone()));
        let avg_cpu = if all.is_empty() {
            0.0
        } else {
            all.iter().map(|c| c.cpu_pct).sum::<f64>() / all.len() as f64
        };
        let lim: f64 = all.iter().map(|c| c.mem_limit_mb.max(1.0)).sum();
        let used: f64 = all.iter().map(|c| c.mem_usage_mb).sum();
        let avg_mem_util = if lim > 0.0 { used / lim * 100.0 } else { 0.0 };
        json!({
            "total": all.len(),
            "running": running,
            "stopped": all.len().saturating_sub(running),
            "images": all.iter().map(|c| c.image.clone()).collect::<std::collections::BTreeSet<_>>().len(),
            "kubernetes": k8s,
            "docker": all.iter().filter(|c| c.runtime == "docker" || c.runtime.is_empty()).count(),
            "by_env": by_env,
            "by_namespace": by_ns,
            "avg_cpu_pct": avg_cpu,
            "avg_mem_util_pct": avg_mem_util,
        })
    }

    /// Datadog Containers Explorer — filterable inventory with utilization vs limits.
    pub fn containers_explorer(
        &self,
        env: Option<&str>,
        namespace: Option<&str>,
        host: Option<&str>,
        service: Option<&str>,
        q: Option<&str>,
    ) -> serde_json::Value {
        let q = q.map(|s| s.to_ascii_lowercase()).unwrap_or_default();
        let mut rows: Vec<_> = self
            .list_containers()
            .into_iter()
            .filter(|c| env.map(|e| c.env == e).unwrap_or(true))
            .filter(|c| {
                namespace
                    .map(|n| c.kube_namespace.as_deref() == Some(n))
                    .unwrap_or(true)
            })
            .filter(|c| host.map(|h| c.host == h).unwrap_or(true))
            .filter(|c| service.map(|s| c.service == s).unwrap_or(true))
            .filter(|c| {
                if q.is_empty() {
                    return true;
                }
                let blob = format!(
                    "{} {} {} {} {} {} {} {}",
                    c.id,
                    c.name,
                    c.image,
                    c.service,
                    c.host,
                    c.env,
                    c.status,
                    c.pod_name.as_deref().unwrap_or("")
                )
                .to_ascii_lowercase();
                // Simple AND / OR / NOT for Datadog-style explorer search
                container_query_match(&blob, &q)
            })
            .collect();
        rows.sort_by(|a, b| {
            b.cpu_pct
                .partial_cmp(&a.cpu_pct)
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        let facets = json!({
            "env": facet_count(self.list_containers().into_iter().map(|c| c.env.if_empty("untagged"))),
            "kube_namespace": facet_count(
                self.list_containers()
                    .into_iter()
                    .filter_map(|c| c.kube_namespace)
            ),
            "host": facet_count(self.list_containers().into_iter().map(|c| c.host)),
            "service": facet_count(self.list_containers().into_iter().map(|c| c.service.if_empty("untagged"))),
            "image": facet_count(self.list_containers().into_iter().map(|c| c.image)),
            "status": facet_count(self.list_containers().into_iter().map(|c| c.status)),
        });
        let enriched: Vec<_> = rows
            .into_iter()
            .map(|c| {
                let cpu_util = if c.cpu_limit > 0.0 {
                    (c.cpu_pct / (c.cpu_limit * 100.0) * 100.0).min(999.0)
                } else {
                    c.cpu_pct
                };
                let mem_util = if c.mem_limit_mb > 0.0 {
                    c.mem_usage_mb / c.mem_limit_mb * 100.0
                } else {
                    0.0
                };
                json!({
                    "id": c.id,
                    "name": c.name,
                    "image": c.image,
                    "host": c.host,
                    "status": c.status,
                    "env": c.env,
                    "service": c.service,
                    "version": c.version,
                    "runtime": c.runtime,
                    "kube_namespace": c.kube_namespace,
                    "pod_name": c.pod_name,
                    "kube_deployment": c.kube_deployment,
                    "cpu_pct": c.cpu_pct,
                    "cpu_limit": c.cpu_limit,
                    "cpu_util_vs_limit_pct": cpu_util,
                    "mem_usage_mb": c.mem_usage_mb,
                    "mem_limit_mb": c.mem_limit_mb,
                    "mem_rss_mb": c.mem_rss_mb,
                    "mem_util_vs_limit_pct": mem_util,
                    "net_rx_bps": c.net_rx_bps,
                    "net_tx_bps": c.net_tx_bps,
                    "restarts": c.restarts,
                    "started_ms": c.started_ms,
                    "over_provisioned": mem_util < 35.0 && cpu_util < 35.0 && c.status == "running",
                    "hot": cpu_util > 80.0 || mem_util > 85.0,
                })
            })
            .collect();
        json!({
            "count": enriched.len(),
            "facets": facets,
            "containers": enriched,
            "significance": "RSS vs limits like Datadog Containers Explorer — over-provisioned rows are bin-pack candidates"
        })
    }

    /// Host Map — Datadog model: Host/Pod/Container/Cluster cells, fill/size signals, hierarchy.
    pub fn infra_hostmap(
        &self,
        group_by: Option<&str>,
        resource: Option<&str>,
    ) -> serde_json::Value {
        let group_by = group_by.unwrap_or("availability-zone");
        let resource = resource.unwrap_or("host");
        let hosts = self.list_hosts();
        let containers = self.list_containers();

        // Index containers by host for secondary children
        let mut by_host: BTreeMap<String, Vec<&ContainerInfo>> = BTreeMap::new();
        for c in &containers {
            by_host.entry(c.host.clone()).or_default().push(c);
        }

        let host_cells: Vec<serde_json::Value> = hosts
            .iter()
            .map(|h| {
                let cpu = if h.cpu <= 1.0 { h.cpu * 100.0 } else { h.cpu };
                let mem_pct = (h.memory_mib / (h.cores.max(1.0) * 2048.0) * 100.0).min(100.0);
                let kids = by_host.get(&h.name).cloned().unwrap_or_default();
                let error_logs = (cpu * 0.35 + kids.iter().map(|c| c.restarts as f64 * 8.0).sum::<f64>())
                    .min(100.0);
                let ready = if kids.is_empty() {
                    100.0
                } else {
                    let ready_n = kids
                        .iter()
                        .filter(|c| c.status == "running" && c.restarts < 3)
                        .count() as f64;
                    ready_n / kids.len() as f64 * 100.0
                };
                let cost = (h.cores * 12.0 + h.memory_mib / 256.0 + kids.len() as f64 * 3.5).min(100.0);
                let agent_major: f64 = h
                    .agent_version
                    .split('.')
                    .next()
                    .and_then(|s| s.parse().ok())
                    .unwrap_or(0.0);
                let agent_outdated = if agent_major > 0.0 && agent_major < 7.0 {
                    85.0
                } else if h.agent_version.starts_with("0.") {
                    70.0
                } else {
                    10.0
                };
                let children: Vec<_> = kids
                    .iter()
                    .map(|c| {
                        let mem_util = if c.mem_limit_mb > 0.0 {
                            c.mem_usage_mb / c.mem_limit_mb * 100.0
                        } else {
                            0.0
                        };
                        let kind = if c.pod_name.is_some() {
                            "pod"
                        } else {
                            "container"
                        };
                        json!({
                            "id": c.pod_name.clone().unwrap_or_else(|| c.id.clone()),
                            "name": c.pod_name.clone().unwrap_or_else(|| c.name.clone()),
                            "kind": kind,
                            "container_id": c.id,
                            "cpu_pct": c.cpu_pct,
                            "mem_pct": mem_util,
                            "readiness": if c.status == "running" && c.restarts < 3 { 100.0 } else { 0.0 },
                            "restarts": c.restarts,
                            "status": c.status,
                            "service": c.service,
                            "env": c.env,
                            "kube_namespace": c.kube_namespace,
                            "kube_deployment": c.kube_deployment,
                            "image": c.image,
                        })
                    })
                    .collect();
                let group = hostmap_group_key(h, group_by);
                json!({
                    "id": h.name,
                    "name": h.name,
                    "kind": "host",
                    "alias": h.alias,
                    "group": group,
                    "env": h.env,
                    "service": h.service,
                    "az": h.az,
                    "availability-zone": h.az,
                    "instance_type": h.instance_type,
                    "cpu_pct": cpu,
                    "memory_mib": h.memory_mib,
                    "mem_pct": mem_pct,
                    "disk_pct": h.disk_pct,
                    "load_15": h.load_15,
                    "cores": h.cores,
                    "container_count": h.container_count.max(kids.len() as u32),
                    "pod_count": kids.iter().filter(|c| c.pod_name.is_some()).count(),
                    "status": h.status,
                    "agent_version": h.agent_version,
                    "apps": h.apps,
                    "tags": h.tags,
                    "fill": cpu,
                    "error_logs": error_logs,
                    "readiness": ready,
                    "cost_score": cost,
                    "agent_outdated": agent_outdated,
                    "children": children,
                })
            })
            .collect();

        // Pod cells (unique pod_name)
        let mut pod_map: BTreeMap<String, serde_json::Value> = BTreeMap::new();
        for c in &containers {
            let Some(pod) = &c.pod_name else { continue };
            let host = hosts.iter().find(|h| h.name == c.host);
            let mem_util = if c.mem_limit_mb > 0.0 {
                c.mem_usage_mb / c.mem_limit_mb * 100.0
            } else {
                0.0
            };
            let ready = if c.status == "running" && c.restarts < 3 {
                100.0
            } else if c.status == "running" {
                40.0
            } else {
                0.0
            };
            let az = host.map(|h| h.az.clone()).unwrap_or_else(|| "unknown".into());
            let itype = host
                .map(|h| h.instance_type.clone())
                .unwrap_or_else(|| "unknown".into());
            let group = match group_by {
                "az" | "availability-zone" => az.clone(),
                "service" => c.service.clone(),
                "env" => c.env.clone(),
                "instance_type" | "instance-type" => itype.clone(),
                "kube_namespace" | "kube_cluster_name" => c
                    .kube_namespace
                    .clone()
                    .unwrap_or_else(|| "default".into()),
                "kube_deployment" => c
                    .kube_deployment
                    .clone()
                    .unwrap_or_else(|| "unknown".into()),
                _ => az.clone(),
            };
            pod_map.insert(
                pod.clone(),
                json!({
                    "id": pod,
                    "name": pod,
                    "kind": "pod",
                    "alias": c.id,
                    "group": group,
                    "env": c.env,
                    "service": c.service,
                    "az": az,
                    "instance_type": itype,
                    "cpu_pct": c.cpu_pct,
                    "mem_pct": mem_util,
                    "memory_mib": c.mem_usage_mb,
                    "disk_pct": 0.0,
                    "load_15": c.cpu_pct / 25.0,
                    "cores": c.cpu_limit.max(0.5),
                    "container_count": 1,
                    "status": if ready >= 100.0 { "ready" } else { "unready" },
                    "agent_version": "",
                    "apps": [c.service.clone()],
                    "tags": {
                        "env": c.env,
                        "service": c.service,
                        "kube_namespace": c.kube_namespace,
                        "kube_deployment": c.kube_deployment,
                        "host": c.host,
                    },
                    "fill": c.cpu_pct,
                    "error_logs": (c.restarts as f64 * 20.0).min(100.0),
                    "readiness": ready,
                    "cost_score": (c.cpu_limit * 15.0 + c.mem_limit_mb / 64.0).min(100.0),
                    "agent_outdated": 0.0,
                    "host": c.host,
                    "kube_namespace": c.kube_namespace,
                    "kube_deployment": c.kube_deployment,
                    "children": [],
                }),
            );
        }
        let pod_cells: Vec<_> = pod_map.into_values().collect();

        let container_cells: Vec<_> = containers
            .iter()
            .map(|c| {
                let host = hosts.iter().find(|h| h.name == c.host);
                let mem_util = if c.mem_limit_mb > 0.0 {
                    c.mem_usage_mb / c.mem_limit_mb * 100.0
                } else {
                    0.0
                };
                let az = host.map(|h| h.az.clone()).unwrap_or_else(|| "unknown".into());
                let group = match group_by {
                    "az" | "availability-zone" => az.clone(),
                    "service" => c.service.clone(),
                    "env" => c.env.clone(),
                    "kube_namespace" => c
                        .kube_namespace
                        .clone()
                        .unwrap_or_else(|| "docker".into()),
                    _ => c.host.clone(),
                };
                json!({
                    "id": c.id,
                    "name": c.name,
                    "kind": "container",
                    "alias": c.image,
                    "group": group,
                    "env": c.env,
                    "service": c.service,
                    "az": az,
                    "instance_type": host.map(|h| h.instance_type.clone()).unwrap_or_default(),
                    "cpu_pct": c.cpu_pct,
                    "mem_pct": mem_util,
                    "memory_mib": c.mem_usage_mb,
                    "disk_pct": 0.0,
                    "load_15": 0.0,
                    "cores": c.cpu_limit,
                    "container_count": 1,
                    "status": c.status,
                    "agent_version": "",
                    "apps": [c.image.split('/').next_back().unwrap_or(&c.image)],
                    "tags": {
                        "env": c.env,
                        "service": c.service,
                        "host": c.host,
                        "image_name": c.image,
                    },
                    "fill": c.cpu_pct,
                    "error_logs": (c.restarts as f64 * 18.0).min(100.0),
                    "readiness": if c.status == "running" { 100.0 } else { 0.0 },
                    "cost_score": (c.mem_limit_mb / 32.0).min(100.0),
                    "agent_outdated": 0.0,
                    "host": c.host,
                    "children": [],
                })
            })
            .collect();

        // Cluster cells — roll up by kube_namespace as cluster surrogate
        let mut cluster_map: BTreeMap<String, (f64, f64, f64, u32, String, String)> = BTreeMap::new();
        for c in &containers {
            let Some(ns) = &c.kube_namespace else { continue };
            let host = hosts.iter().find(|h| h.name == c.host);
            let az = host.map(|h| h.az.as_str()).unwrap_or("unknown");
            let e = cluster_map.entry(ns.clone()).or_insert((0.0, 0.0, 0.0, 0, az.into(), c.env.clone()));
            e.0 += c.cpu_pct;
            e.1 += if c.mem_limit_mb > 0.0 {
                c.mem_usage_mb / c.mem_limit_mb * 100.0
            } else {
                0.0
            };
            e.2 += c.restarts as f64 * 10.0;
            e.3 += 1;
            e.4 = az.into();
            e.5 = c.env.clone();
        }
        let cluster_cells: Vec<_> = cluster_map
            .into_iter()
            .map(|(ns, (cpu_sum, mem_sum, err, n, az, env))| {
                let cpu = if n > 0 { cpu_sum / n as f64 } else { 0.0 };
                let mem = if n > 0 { mem_sum / n as f64 } else { 0.0 };
                let group = match group_by {
                    "az" | "availability-zone" => az.clone(),
                    "env" => env.clone(),
                    _ => ns.clone(),
                };
                json!({
                    "id": format!("cluster-{ns}"),
                    "name": ns,
                    "kind": "cluster",
                    "alias": format!("{n} workloads"),
                    "group": group,
                    "env": env,
                    "service": "kubernetes",
                    "az": az,
                    "instance_type": "node-pool",
                    "cpu_pct": cpu,
                    "mem_pct": mem,
                    "memory_mib": mem * 10.0,
                    "disk_pct": 30.0,
                    "load_15": cpu / 20.0,
                    "cores": n as f64,
                    "container_count": n,
                    "status": "up",
                    "agent_version": "7.50",
                    "apps": ["kubernetes"],
                    "tags": { "kube_cluster_name": ns, "env": env, "availability-zone": az },
                    "fill": cpu,
                    "error_logs": err.min(100.0),
                    "readiness": (100.0 - err.min(80.0)).max(20.0),
                    "cost_score": (n as f64 * 8.0).min(100.0),
                    "agent_outdated": 15.0,
                    "children": [],
                })
            })
            .collect();

        let cells: Vec<serde_json::Value> = match resource {
            "pod" => pod_cells.clone(),
            "container" => container_cells.clone(),
            "cluster" => cluster_cells.clone(),
            _ => host_cells.clone(),
        };

        let mut groups: BTreeMap<String, Vec<serde_json::Value>> = BTreeMap::new();
        for c in &cells {
            let g = c["group"].as_str().unwrap_or("other").to_string();
            groups.entry(g).or_default().push(c.clone());
        }

        let mut filter_tags: BTreeSet<String> = BTreeSet::new();
        filter_tags.insert("nginx".into());
        filter_tags.insert("tags.env:prod".into());
        filter_tags.insert("tags.availability-zone:us-east-1*".into());
        filter_tags.insert("NOT tags.team:*".into());
        filter_tags.insert("kubernetes_state.pod.status:unready".into());
        for h in &hosts {
            filter_tags.insert(format!("env:{}", h.env));
            filter_tags.insert(format!("tags.env:{}", h.env));
            filter_tags.insert(format!("service:{}", h.service));
            filter_tags.insert(format!("tags.availability-zone:{}", h.az));
            filter_tags.insert(format!("availability-zone:{}", h.az));
            filter_tags.insert(format!("tags.instance-type:{}", h.instance_type));
            filter_tags.insert(format!("tags.agent_version:{}", h.agent_version));
            filter_tags.insert(h.service.clone());
            if let Some(acct) = h.tags.get("account") {
                filter_tags.insert(format!("tags.account:{}", acct));
            }
            if let Some(cp) = h.tags.get("cloud_provider") {
                filter_tags.insert(format!("tags.cloud_provider:{}", cp));
            }
            for a in &h.apps {
                filter_tags.insert(a.clone());
                filter_tags.insert(format!("app:{}", a));
            }
        }

        json!({
            "resource": resource,
            "resource_options": ["host", "pod", "container", "cluster"],
            "secondary_options": ["none", "pod", "container"],
            "group_by": group_by,
            "fill_by": "CPU usage",
            "fill_options": [
                "CPU usage",
                "Memory usage",
                "Disk usage",
                "Load 15",
                "Error logs",
                "Readiness",
                "Cost score",
                "Agent outdated"
            ],
            "size_options": [
                "—",
                "CPU usage",
                "Memory usage",
                "Error logs",
                "Cost score",
                "Container count",
                "CPU cores"
            ],
            "group_options": [
                "availability-zone",
                "instance-type",
                "env",
                "service",
                "kube_namespace",
                "kube_deployment",
                "cloud_provider"
            ],
            "hosts": cells,
            "groups": groups,
            "counts": {
                "host": host_cells.len(),
                "pod": pod_cells.len(),
                "container": container_cells.len(),
                "cluster": cluster_cells.len(),
            },
            "filter_tags": filter_tags.into_iter().collect::<Vec<_>>(),
            "suggested_queries": [
                {
                    "id": "cpu-hosts",
                    "title": "What is the CPU usage across my infrastructure?",
                    "resource": "host",
                    "secondary": "none",
                    "fill_by": "CPU usage",
                    "size_by": "—",
                    "group_by": ["availability-zone"],
                    "filter": "",
                },
                {
                    "id": "error-infra",
                    "title": "How many errors are being logged across my infrastructure?",
                    "resource": "host",
                    "secondary": "none",
                    "fill_by": "Error logs",
                    "size_by": "Error logs",
                    "group_by": ["availability-zone", "service"],
                    "filter": "tags.env:prod",
                },
                {
                    "id": "busy-ready",
                    "title": "Which services run on the busiest hosts, and are their pods ready?",
                    "resource": "host",
                    "secondary": "pod",
                    "fill_by": "CPU usage",
                    "size_by": "—",
                    "group_by": ["service"],
                    "filter": "",
                    "secondary_fill": "Readiness",
                },
                {
                    "id": "troubleshoot",
                    "title": "Troubleshoot degraded server performance",
                    "resource": "host",
                    "secondary": "pod",
                    "fill_by": "CPU usage",
                    "size_by": "—",
                    "group_by": ["availability-zone"],
                    "filter": "tags.env:prod",
                    "secondary_fill": "Readiness",
                },
                {
                    "id": "cost-hotspots",
                    "title": "Identify cost hotspots",
                    "resource": "host",
                    "secondary": "none",
                    "fill_by": "Cost score",
                    "size_by": "Cost score",
                    "group_by": ["availability-zone", "instance-type"],
                    "filter": "tags.cloud_provider:aws",
                },
                {
                    "id": "agent-fleet",
                    "title": "Fleet-wide agent management",
                    "resource": "host",
                    "secondary": "none",
                    "fill_by": "Agent outdated",
                    "size_by": "—",
                    "group_by": ["availability-zone", "service"],
                    "filter": "",
                },
                {
                    "id": "k8s-rollout",
                    "title": "Monitor Kubernetes rollouts",
                    "resource": "cluster",
                    "secondary": "pod",
                    "fill_by": "Readiness",
                    "size_by": "Container count",
                    "group_by": ["kube_namespace"],
                    "filter": "",
                    "secondary_fill": "CPU usage",
                },
                {
                    "id": "tag-hygiene",
                    "title": "Verify tagging and metadata hygiene",
                    "resource": "host",
                    "secondary": "none",
                    "fill_by": "CPU usage",
                    "size_by": "—",
                    "group_by": ["env"],
                    "filter": "tags.env:prod AND NOT tags.team:*",
                }
            ],
            "significance": "Datadog Host Map — main/secondary resources, fill/size, multi-group, Boolean filters"
        })
    }

    /// Environment rollup — prod vs staging vs dev with hosts/containers/CPU/mem.
    pub fn infra_envs(&self) -> serde_json::Value {
        let hosts = self.list_hosts();
        let containers = self.list_containers();
        let mut envs: BTreeSet<String> = BTreeSet::new();
        for h in &hosts {
            envs.insert(h.env.clone().if_empty("untagged"));
        }
        for c in &containers {
            envs.insert(c.env.clone().if_empty("untagged"));
        }
        let rows: Vec<_> = envs
            .into_iter()
            .map(|env| {
                let hs: Vec<_> = hosts.iter().filter(|h| h.env == env).collect();
                let cs: Vec<_> = containers.iter().filter(|c| c.env == env).collect();
                let avg_cpu = if hs.is_empty() {
                    0.0
                } else {
                    hs.iter()
                        .map(|h| if h.cpu <= 1.0 { h.cpu * 100.0 } else { h.cpu })
                        .sum::<f64>()
                        / hs.len() as f64
                };
                let hot_containers = cs.iter().filter(|c| c.cpu_pct > 70.0).count();
                let k8s_ns: BTreeSet<_> = cs
                    .iter()
                    .filter_map(|c| c.kube_namespace.clone())
                    .collect();
                json!({
                    "env": env,
                    "hosts": hs.len(),
                    "containers": cs.len(),
                    "running": cs.iter().filter(|c| c.status == "running").count(),
                    "avg_host_cpu_pct": avg_cpu,
                    "hot_containers": hot_containers,
                    "namespaces": k8s_ns.len(),
                    "services": cs.iter().map(|c| c.service.clone()).collect::<BTreeSet<_>>().len(),
                    "mem_mib": hs.iter().map(|h| h.memory_mib).sum::<f64>(),
                })
            })
            .collect();
        json!({
            "envs": rows,
            "unified_tags": ["env", "service", "version"],
            "significance": "Unified service tagging (env/service/version) ties infra ↔ APM ↔ logs like Datadog"
        })
    }

    /// K8s-style resource utilization: usage vs requests/limits by deployment/namespace.
    pub fn k8s_resource_utilization(&self) -> serde_json::Value {
        let mut groups: BTreeMap<(String, String), (f64, f64, f64, f64, f64, f64, u32)> =
            BTreeMap::new();
        // key: (ns, deployment) -> cpu_use, cpu_req, cpu_lim, mem_use, mem_req, mem_lim, pods
        for c in self.list_containers() {
            let Some(ns) = c.kube_namespace.clone() else {
                continue;
            };
            let dep = c
                .kube_deployment
                .clone()
                .unwrap_or_else(|| c.service.clone().if_empty("workload"));
            let e = groups.entry((ns, dep)).or_insert((0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0));
            e.0 += c.cpu_pct / 100.0; // cores used approx
            e.1 += (c.cpu_limit * 0.5).max(0.1); // request ~ half limit if not stored
            e.2 += c.cpu_limit.max(0.1);
            e.3 += c.mem_usage_mb;
            e.4 += c.mem_limit_mb * 0.5;
            e.5 += c.mem_limit_mb.max(1.0);
            e.6 += 1;
        }
        let rows: Vec<_> = groups
            .into_iter()
            .map(|((ns, dep), (cu, cr, cl, mu, mr, ml, n))| {
                json!({
                    "kube_namespace": ns,
                    "kube_deployment": dep,
                    "pods": n,
                    "cpu_usage": cu,
                    "cpu_requested": cr,
                    "cpu_limit": cl,
                    "cpu_usage_vs_request_pct": if cr > 0.0 { cu / cr * 100.0 } else { 0.0 },
                    "cpu_usage_vs_limit_pct": if cl > 0.0 { cu / cl * 100.0 } else { 0.0 },
                    "memory_usage_mb": mu,
                    "memory_requested_mb": mr,
                    "memory_limit_mb": ml,
                    "memory_usage_vs_request_pct": if mr > 0.0 { mu / mr * 100.0 } else { 0.0 },
                    "memory_usage_vs_limit_pct": if ml > 0.0 { mu / ml * 100.0 } else { 0.0 },
                    "waste_hint": if cu / cl.max(0.01) < 0.35 && mu / ml.max(1.0) < 0.35 {
                        "over-provisioned — tighten requests/limits"
                    } else if cu / cl.max(0.01) > 0.85 || mu / ml.max(1.0) > 0.85 {
                        "near limit — risk of throttle/OOM"
                    } else {
                        "balanced"
                    },
                })
            })
            .collect();
        json!({
            "group_by": ["kube_namespace", "kube_deployment"],
            "rows": rows,
            "significance": "Datadog Kubernetes Resource Utilization — usage/request/limit % for bin packing"
        })
    }

    /// Seed multi-env hosts + docker/k8s containers and emit container.* / docker.* metrics.
    pub fn seed_infra_fleet(&self) {
        let now = Utc::now().timestamp_millis();
        // Dense Host Map fleet across AZs (Datadog-style honeycomb)
        let azs = [
            "us-east-1a",
            "us-east-1b",
            "us-east-1c",
            "us-east-1d",
            "us-west-2a",
            "northcentralus",
            "us-central1-a",
        ];
        let roles: [(&str, bool, &[&str]); 6] = [
            ("nginx", true, &["aws", "nginx", "chef", "fluentd"]),
            ("api", true, &["aws", "docker", "api", "consul"]),
            ("worker", false, &["aws", "docker", "worker", "chef"]),
            ("checkout", true, &["aws", "docker", "checkout", "go_expvar"]),
            ("ingest", false, &["aws", "docker", "ingest"]),
            ("payments", true, &["aws", "docker", "payments", "bifrost"]),
        ];
        let mut n = 0u32;
        for (az_i, az) in azs.iter().enumerate() {
            // Dense honeycomb clusters like Datadog Host Map screenshots
            let per_az = 18 + (az_i % 5) * 4;
            for i in 0..per_az {
                n += 1;
                let (role, is_nginx_tier, base_apps) = roles[i % roles.len()];
                let env = if az_i == 3 && i > 8 { "staging" } else { "prod" };
                let cpu = if role == "payments" && *az == "us-east-1c" && i == 2 {
                    0.91
                } else if role == "nginx" {
                    0.18 + (i as f64 % 5.0) * 0.04
                } else if role == "api" {
                    0.35 + (i as f64 % 6.0) * 0.05
                } else if role == "checkout" {
                    0.42 + (i as f64 % 4.0) * 0.06
                } else {
                    0.22 + (i as f64 % 7.0) * 0.045
                };
                let name = format!("i-0{:010x}", 0x1000_0000u32 + n * 97 + az_i as u32 * 13);
                let alias = format!("ip-172-31-{}-{}", 100 + az_i * 20 + (i % 20), 10 + (n % 200));
                let mut apps: Vec<String> = base_apps.iter().map(|s| (*s).into()).collect();
                if is_nginx_tier && !apps.iter().any(|a| a == "nginx") {
                    apps.insert(1, "nginx".into());
                }
                if i % 4 == 0 {
                    apps.push("go_expvar".into());
                }
                if i % 5 == 0 {
                    apps.push("chefAPI".into());
                }
                let itype = if role == "nginx" {
                    "c6i.large"
                } else if role == "api" {
                    "m6i.xlarge"
                } else {
                    "m6i.large"
                };
                let cores = if itype.contains("xlarge") { 4.0 } else { 2.0 };
                self.hosts.insert(
                    name.clone(),
                    HostInfo {
                        name: name.clone(),
                        service: role.into(),
                        env: env.into(),
                        cpu,
                        memory_mib: 1800.0 + (i as f64) * 220.0,
                        status: "up".into(),
                        az: (*az).into(),
                        instance_type: itype.into(),
                        agent_version: "0.2.0".into(),
                        cores,
                        load_15: cpu * cores * 0.85,
                        disk_pct: 25.0 + (i as f64 % 9.0) * 4.0,
                        container_count: 2 + (i % 3) as u32,
                        tags: BTreeMap::from([
                            ("env".into(), env.into()),
                            ("service".into(), role.into()),
                            ("availability-zone".into(), (*az).into()),
                            ("instance-type".into(), itype.into()),
                            ("cloud_provider".into(), "aws".into()),
                            (
                                "account".into(),
                                if env == "prod" { "demo1" } else { "demo3" }.into(),
                            ),
                        ]),
                        alias,
                        apps,
                    },
                );
            }
        }
        for (name, svc, env, az, cpu) in [
            ("i-api-1", "api", "prod", "us-east-1a", 0.71),
            ("i-api-2", "api", "prod", "us-east-1b", 0.48),
            ("i-worker-1", "worker", "prod", "us-east-1a", 0.55),
            ("i-worker-2", "worker", "prod", "us-east-1c", 0.33),
            ("i-ingest-1", "ingest", "prod", "us-east-1a", 0.62),
            ("k8s-node-1", "kubernetes", "prod", "us-east-1a", 0.58),
            ("k8s-node-2", "kubernetes", "prod", "us-east-1b", 0.41),
            ("i-api-stg-1", "api", "staging", "us-east-1a", 0.28),
            ("i-worker-stg-1", "worker", "staging", "us-east-1a", 0.22),
            ("k8s-stg-1", "kubernetes", "staging", "us-east-1a", 0.31),
            ("i-dev-1", "api", "dev", "us-west-2a", 0.12),
            ("k8s-dev-1", "kubernetes", "dev", "us-west-2a", 0.18),
        ] {
            self.hosts.insert(
                name.into(),
                HostInfo {
                    name: name.into(),
                    service: svc.into(),
                    env: env.into(),
                    cpu,
                    memory_mib: 6400.0,
                    status: "up".into(),
                    az: az.into(),
                    instance_type: "m6i.xlarge".into(),
                    agent_version: "0.2.0".into(),
                    cores: 4.0,
                    load_15: cpu * 3.5,
                    disk_pct: 48.0,
                    container_count: 3,
                    tags: BTreeMap::from([
                        ("env".into(), env.into()),
                        ("service".into(), svc.into()),
                        ("availability-zone".into(), az.into()),
                        ("cloud_provider".into(), "aws".into()),
                        ("account".into(), "demo1".into()),
                    ]),
                    alias: format!("ip-10-0-{}-20", name.len()),
                    apps: vec![
                        "aws".into(),
                        "docker".into(),
                        svc.into(),
                        "consul".into(),
                        "fluentd".into(),
                        "nginx".into(),
                    ],
                },
            );
        }

        let containers = [
            // docker on prod hosts
            ("ctr-api-1", "api", "thine/api:1.4.2", "i-api-1", "prod", "1.4.2", "docker", None, None, None, 68.0, 1.0, 780.0, 1024.0, 2),
            ("ctr-api-2", "api", "thine/api:1.4.2", "i-api-2", "prod", "1.4.2", "docker", None, None, None, 41.0, 1.0, 520.0, 1024.0, 0),
            ("ctr-worker-1", "worker", "thine/worker:2.1.0", "i-worker-1", "prod", "2.1.0", "docker", None, None, None, 55.0, 1.0, 640.0, 1536.0, 1),
            ("ctr-worker-2", "worker", "thine/worker:2.1.0", "i-worker-2", "prod", "2.1.0", "docker", None, None, None, 29.0, 1.0, 410.0, 1536.0, 0),
            ("ctr-ingest-1", "ingest", "thine/ingest:0.9.1", "i-ingest-1", "prod", "0.9.1", "docker", None, None, None, 74.0, 2.0, 1100.0, 2048.0, 3),
            ("ctr-redis-1", "redis", "redis:7.2", "i-api-1", "prod", "7.2", "docker", None, None, None, 8.0, 0.5, 180.0, 512.0, 0),
            // k8s prod
            ("ctr-chk-a", "checkout", "thine/checkout:3.0.1", "k8s-node-1", "prod", "3.0.1", "containerd", Some("payments"), Some("checkout-7f9d8c-a1"), Some("checkout"), 82.0, 1.0, 920.0, 1024.0, 4),
            ("ctr-chk-b", "checkout", "thine/checkout:3.0.1", "k8s-node-2", "prod", "3.0.1", "containerd", Some("payments"), Some("checkout-7f9d8c-b2"), Some("checkout"), 61.0, 1.0, 700.0, 1024.0, 1),
            ("ctr-api-k1", "api", "thine/api:1.4.2", "k8s-node-1", "prod", "1.4.2", "containerd", Some("default"), Some("api-6d4b2-x1"), Some("api"), 47.0, 2.0, 890.0, 2048.0, 0),
            ("ctr-api-k2", "api", "thine/api:1.4.2", "k8s-node-2", "prod", "1.4.2", "containerd", Some("default"), Some("api-6d4b2-x2"), Some("api"), 33.0, 2.0, 760.0, 2048.0, 0),
            ("ctr-pay-1", "payments", "thine/payments:1.2.0", "k8s-node-1", "prod", "1.2.0", "containerd", Some("payments"), Some("payments-5c8a-p1"), Some("payments"), 39.0, 1.0, 540.0, 1024.0, 2),
            ("ctr-nginx", "edge", "nginx:1.25", "k8s-node-2", "prod", "1.25", "containerd", Some("ingress"), Some("nginx-ingress-q1"), Some("nginx-ingress"), 12.0, 0.5, 96.0, 256.0, 0),
            // staging
            ("ctr-api-stg", "api", "thine/api:1.5.0-rc1", "i-api-stg-1", "staging", "1.5.0-rc1", "docker", None, None, None, 18.0, 1.0, 310.0, 1024.0, 0),
            ("ctr-wrk-stg", "worker", "thine/worker:2.2.0-rc1", "i-worker-stg-1", "staging", "2.2.0-rc1", "docker", None, None, None, 14.0, 1.0, 280.0, 1024.0, 0),
            ("ctr-api-kstg", "api", "thine/api:1.5.0-rc1", "k8s-stg-1", "staging", "1.5.0-rc1", "containerd", Some("default"), Some("api-stg-1"), Some("api"), 21.0, 1.0, 400.0, 1024.0, 0),
            ("ctr-chk-stg", "checkout", "thine/checkout:3.1.0-rc", "k8s-stg-1", "staging", "3.1.0-rc", "containerd", Some("payments"), Some("checkout-stg-1"), Some("checkout"), 26.0, 1.0, 450.0, 1024.0, 1),
            // dev
            ("ctr-api-dev", "api", "thine/api:dev", "i-dev-1", "dev", "dev", "docker", None, None, None, 9.0, 1.0, 220.0, 1024.0, 0),
            ("ctr-api-kdev", "api", "thine/api:dev", "k8s-dev-1", "dev", "dev", "containerd", Some("default"), Some("api-dev-1"), Some("api"), 11.0, 0.5, 180.0, 512.0, 0),
            ("ctr-sidekiq", "worker", "thine/worker:dev", "k8s-dev-1", "dev", "dev", "containerd", Some("default"), Some("worker-dev-1"), Some("worker"), 7.0, 0.5, 160.0, 512.0, 0),
            // stopped
            ("ctr-old-api", "api", "thine/api:1.3.0", "i-api-1", "prod", "1.3.0", "docker", None, None, None, 0.0, 1.0, 0.0, 1024.0, 0),
        ];

        let mut host_counts: BTreeMap<String, u32> = BTreeMap::new();
        for (id, svc, image, host, env, ver, runtime, ns, pod, dep, cpu, clim, mem, mlim, restarts) in containers {
            let status = if id == "ctr-old-api" {
                "stopped"
            } else {
                "running"
            };
            host_counts
                .entry(host.into())
                .and_modify(|n| *n += 1)
                .or_insert(1);
            self.upsert_container(ContainerInfo {
                id: id.into(),
                image: image.into(),
                host: host.into(),
                status: status.into(),
                name: svc.into(),
                env: env.into(),
                service: svc.into(),
                version: ver.into(),
                runtime: runtime.into(),
                kube_namespace: ns.map(|s| s.into()),
                pod_name: pod.map(|s| s.into()),
                kube_deployment: dep.map(|s| s.into()),
                cpu_pct: cpu,
                cpu_limit: clim,
                mem_usage_mb: mem,
                mem_limit_mb: mlim,
                mem_rss_mb: mem * 0.92,
                net_rx_bps: 50_000.0 + cpu * 2_000.0,
                net_tx_bps: 40_000.0 + cpu * 1_500.0,
                restarts,
                started_ms: now - 3_600_000 - (restarts as i64 * 60_000),
            });
        }
        for (host, n) in host_counts {
            if let Some(mut h) = self.hosts.get_mut(&host) {
                h.container_count = n;
            }
        }

        // Emit Datadog-style container / docker / system metrics for explorer + hostmap fill
        for c in self.list_containers() {
            let mut tags = Tags::from([
                ("host".into(), c.host.clone()),
                ("env".into(), c.env.clone()),
                ("service".into(), c.service.clone()),
                ("version".into(), c.version.clone()),
                ("container_name".into(), c.name.clone()),
                ("container_id".into(), c.id.clone()),
                ("image_name".into(), c.image.clone()),
            ]);
            if let Some(ns) = &c.kube_namespace {
                tags.insert("kube_namespace".into(), ns.clone());
            }
            if let Some(pod) = &c.pod_name {
                tags.insert("pod_name".into(), pod.clone());
            }
            if let Some(dep) = &c.kube_deployment {
                tags.insert("kube_deployment".into(), dep.clone());
            }
            for i in 0..24 {
                let ts = now - (23 - i) as i64 * 300_000;
                let wobble = ((i as f64) * 0.4).sin() * 4.0;
                let _ = self.metrics.ingest_point(gauge(
                    "container.cpu.usage",
                    &tags,
                    ts,
                    (c.cpu_pct + wobble).max(0.0),
                    "percent",
                ));
                let _ = self.metrics.ingest_point(gauge(
                    "container.memory.usage",
                    &tags,
                    ts,
                    (c.mem_usage_mb + wobble) * 1024.0 * 1024.0,
                    "byte",
                ));
                let _ = self.metrics.ingest_point(gauge(
                    "container.memory.limit",
                    &tags,
                    ts,
                    c.mem_limit_mb * 1024.0 * 1024.0,
                    "byte",
                ));
                let _ = self.metrics.ingest_point(gauge(
                    "docker.cpu.usage",
                    &tags,
                    ts,
                    (c.cpu_pct + wobble).max(0.0),
                    "percent",
                ));
            }
        }
        for h in self.list_hosts() {
            let tags = Tags::from([
                ("host".into(), h.name.clone()),
                ("env".into(), h.env.clone()),
                ("service".into(), h.service.clone()),
                ("availability-zone".into(), h.az.clone()),
            ]);
            let cpu = if h.cpu <= 1.0 { h.cpu * 100.0 } else { h.cpu };
            for i in 0..24 {
                let ts = now - (23 - i) as i64 * 300_000;
                let wobble = ((i as f64) * 0.35).sin() * 3.0;
                let _ = self.metrics.ingest_point(gauge(
                    "system.cpu.user",
                    &tags,
                    ts,
                    (cpu + wobble).max(0.0),
                    "percent",
                ));
                let _ = self.metrics.ingest_point(gauge(
                    "system.mem.used",
                    &tags,
                    ts,
                    h.memory_mib * 1024.0 * 1024.0,
                    "byte",
                ));
                let _ = self.metrics.ingest_point(gauge(
                    "docker.containers.running",
                    &tags,
                    ts,
                    h.container_count as f64,
                    "1",
                ));
            }
        }
    }

    // —— K8s autoscalers ——
    pub fn upsert_autoscaler(&self, a: Autoscaler) -> Autoscaler {
        self.autoscalers.insert(a.name.clone(), a.clone());
        a
    }

    pub fn recommend_autoscalers(&self) -> Vec<Autoscaler> {
        let mut out = Vec::new();
        for e in self.autoscalers.iter() {
            let mut a = e.value().clone();
            // recommend scale-out if host CPU high for matching service
            let hot = self.hosts.iter().any(|h| h.cpu > a.target_cpu);
            if hot && a.current < a.max {
                a.recommended = (a.current + 1).min(a.max);
            } else if !hot && a.current > a.min {
                a.recommended = a.current.saturating_sub(1).max(a.min);
            } else {
                a.recommended = a.current;
            }
            out.push(a);
        }
        if out.is_empty() {
            out.push(Autoscaler {
                name: "api-hpa".into(),
                namespace: "default".into(),
                min: 2,
                max: 20,
                current: 4,
                recommended: 5,
                metric: "cpu".into(),
                target_cpu: 0.65,
            });
        }
        out
    }

    // —— Serverless ——
    pub fn upsert_function(&self, f: ServerlessFunction) -> ServerlessFunction {
        self.serverless
            .insert(f.name.clone(), f.clone());
        f
    }

    pub fn list_serverless_detailed(&self) -> Vec<ServerlessFunction> {
        let mut v: Vec<_> = self.serverless.iter().map(|e| e.value().clone()).collect();
        if v.is_empty() {
            // migrate legacy json map
            for e in self.functions.iter() {
                let val = e.value();
                v.push(ServerlessFunction {
                    name: val
                        .get("name")
                        .and_then(|x| x.as_str())
                        .unwrap_or(e.key())
                        .into(),
                    runtime: val
                        .get("runtime")
                        .and_then(|x| x.as_str())
                        .unwrap_or("unknown")
                        .into(),
                    invocations_24h: 1000,
                    errors_24h: 3,
                    cold_starts_24h: val
                        .get("cold_starts_24h")
                        .and_then(|x| x.as_u64())
                        .unwrap_or(0),
                    avg_duration_ms: 120.0,
                    memory_mb: 512,
                    cloud: "aws".into(),
                    region: "us-east-1".into(),
                    service: e.key().clone(),
                    env: "prod".into(),
                    estimated_cost_24h: 1.2,
                    timeout_errors_24h: 0,
                    oom_errors_24h: 0,
                    memory_used_pct: 45.0,
                    kind: "lambda".into(),
                });
            }
        }
        v.sort_by(|a, b| b.invocations_24h.cmp(&a.invocations_24h));
        v
    }

    /// Serverless Monitoring overview — Lambda / Azure / Cloud Run rollup.
    pub fn serverless_overview(&self) -> serde_json::Value {
        let fns = self.list_serverless_detailed();
        let invocations: u64 = fns.iter().map(|f| f.invocations_24h).sum();
        let errors: u64 = fns.iter().map(|f| f.errors_24h).sum();
        let cold: u64 = fns.iter().map(|f| f.cold_starts_24h).sum();
        let cost: f64 = fns.iter().map(|f| f.estimated_cost_24h).sum();
        let by_cloud = {
            let mut m: BTreeMap<String, u64> = BTreeMap::new();
            for f in &fns {
                *m.entry(f.cloud.clone()).or_insert(0) += 1;
            }
            m
        };
        let by_kind = {
            let mut m: BTreeMap<String, u64> = BTreeMap::new();
            for f in &fns {
                let k = if f.kind.is_empty() {
                    "lambda".into()
                } else {
                    f.kind.clone()
                };
                *m.entry(k).or_insert(0) += 1;
            }
            m
        };
        json!({
            "functions": fns,
            "totals": {
                "functions": fns.len(),
                "invocations_24h": invocations,
                "errors_24h": errors,
                "cold_starts_24h": cold,
                "estimated_cost_24h": cost,
                "error_rate": if invocations > 0 { errors as f64 / invocations as f64 * 100.0 } else { 0.0 },
            },
            "by_cloud": by_cloud,
            "by_kind": by_kind,
            "significance": "Datadog Serverless Monitoring — Lambda, Azure App Service, Cloud Run with enhanced cold start / cost signals"
        })
    }

    // —— Cost ——
    pub fn cost_detail(&self) -> Vec<CostLine> {
        let mut lines = Vec::new();
        for svc in self.catalog.iter() {
            let name = svc.key().clone();
            let compute = 80.0 + name.len() as f64 * 12.0;
            let storage = 25.0 + (name.len() % 5) as f64 * 3.0;
            let network = 15.0 + (name.as_bytes().first().copied().unwrap_or(1) % 7) as f64;
            lines.push(CostLine {
                service: name,
                compute,
                storage,
                network,
                total: compute + storage + network,
            });
        }
        lines.sort_by(|a, b| {
            b.total
                .partial_cmp(&a.total)
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        lines
    }

    // —— Storage / GPU live updates ——
    pub fn upsert_volume(&self, v: VolumeInfo) -> VolumeInfo {
        self.volumes.insert(v.id.clone(), v.clone());
        v
    }

    // —— Profiler ——
    pub fn upload_profile(&self, meta: ProfileMeta, blob_b64: Option<String>) -> ProfileMeta {
        if let Some(b) = blob_b64 {
            self.profile_blobs.insert(meta.id.clone(), b);
        }
        self.profiles.insert(meta.id.clone(), meta.clone());
        meta
    }

    pub fn get_profile_blob(&self, id: &str) -> Option<String> {
        self.profile_blobs.get(id).map(|e| e.value().clone())
    }

    // —— Dynamic instrumentation ——
    pub fn upsert_probe(&self, p: DynProbe) -> DynProbe {
        self.probes.insert(p.id.clone(), p.clone());
        self.audit("upsert", &format!("probe:{}", p.id), "system");
        p
    }

    pub fn list_probes(&self) -> Vec<DynProbe> {
        self.probes.iter().map(|e| e.value().clone()).collect()
    }

    pub fn delete_probe(&self, id: &str) -> bool {
        self.probes.remove(id).is_some()
    }

    // —— DBM ——
    pub fn add_query_sample(&self, sample: DbQuerySample) {
        let mut q = self.db_queries.write();
        q.push_back(sample);
        while q.len() > 1000 {
            q.pop_front();
        }
    }

    pub fn top_queries(&self, limit: usize) -> Vec<DbQuerySample> {
        let mut v: Vec<_> = self.db_queries.read().iter().cloned().collect();
        v.sort_by(|a, b| {
            (b.duration_ms * b.calls as f64)
                .partial_cmp(&(a.duration_ms * a.calls as f64))
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        v.truncate(limit);
        v
    }

    // —— Streams ——
    pub fn upsert_stream(&self, s: StreamInfo) -> StreamInfo {
        self.streams.insert(s.name.clone(), s.clone());
        s
    }

    // —— SDS scan ——
    pub fn upsert_sds_rule(&self, rule: SdsRule) -> SdsRule {
        self.sds_rules.insert(rule.id.clone(), rule.clone());
        rule
    }

    pub fn scan_logs_for_sensitive(&self) -> Vec<SdsFinding> {
        let rules: Vec<_> = self
            .sds_rules
            .iter()
            .filter(|r| r.enabled)
            .map(|r| r.value().clone())
            .collect();
        let mut findings = Vec::new();
        for log in self.logs.read().iter().rev().take(500) {
            for rule in &rules {
                let hit = match rule.id.as_str() {
                    "email" => log.message.contains('@') && log.message.contains('.'),
                    _ => log.message.contains(&rule.pattern)
                        || (!rule.pattern.is_empty()
                            && log.message.to_ascii_lowercase().contains(
                                &rule
                                    .pattern
                                    .chars()
                                    .filter(|c| c.is_alphanumeric())
                                    .take(8)
                                    .collect::<String>()
                                    .to_ascii_lowercase(),
                            )),
                };
                if hit {
                    findings.push(SdsFinding {
                        rule_id: rule.id.clone(),
                        service: log.service.clone(),
                        snippet: log.message.chars().take(120).collect(),
                        timestamp_ms: log.timestamp_ms,
                    });
                }
            }
        }
        findings.truncate(100);
        findings
    }

    // —— Pipelines ——
    pub fn run_pipeline(&self, id: &str, events: Vec<LogEvent>) -> serde_json::Value {
        let Some(pipe) = self.pipelines.get(id) else {
            return json!({ "error": "pipeline not found" });
        };
        let mut processed = 0u64;
        let mut dropped = 0u64;
        for mut e in events {
            for proc in &pipe.processors {
                match proc.as_str() {
                    "grok" => {
                        if e.message.contains('=') {
                            // naive key=value extract
                            for part in e.message.split_whitespace() {
                                if let Some((k, v)) = part.split_once('=') {
                                    e.attrs.insert(k.into(), v.into());
                                }
                            }
                        }
                    }
                    "remap" => {
                        e.service = e.service.to_ascii_lowercase();
                    }
                    "filter_debug" => {
                        if e.level == "debug" {
                            dropped += 1;
                            continue;
                        }
                    }
                    _ => {}
                }
            }
            if e.level != "debug" || !pipe.processors.iter().any(|p| p == "filter_debug") {
                self.ingest_log(e);
                processed += 1;
            }
        }
        json!({
            "pipeline": id,
            "processors": pipe.processors,
            "processed": processed,
            "dropped": dropped
        })
    }

    pub fn upsert_pipeline(&self, p: Pipeline) -> Pipeline {
        self.pipelines.insert(p.id.clone(), p.clone());
        p
    }

    // —— BYOC ——
    pub fn upsert_byoc(&self, sink: ByocSink) -> ByocSink {
        self.byoc_sinks.insert(sink.id.clone(), sink.clone());
        sink
    }

    pub fn list_byoc(&self) -> Vec<ByocSink> {
        self.byoc_sinks.iter().map(|e| e.value().clone()).collect()
    }

    pub fn forward_logs_byoc(&self, sink_id: &str, limit: usize) -> serde_json::Value {
        let Some(mut sink) = self.byoc_sinks.get_mut(sink_id) else {
            return json!({ "error": "sink not found" });
        };
        if !sink.enabled {
            return json!({ "error": "sink disabled" });
        }
        let batch: Vec<_> = self
            .logs
            .read()
            .iter()
            .rev()
            .take(limit)
            .cloned()
            .collect();
        sink.forwarded += batch.len() as u64;
        json!({
            "sink": sink_id,
            "provider": sink.provider,
            "bucket": sink.bucket,
            "prefix": sink.prefix,
            "forwarded_now": batch.len(),
            "forwarded_total": sink.forwarded
        })
    }

    // —— UX / RUM / Synthetics ——
    pub fn ux_overview(&self) -> serde_json::Value {
        let sessions = self.ux_sessions();
        let vitals = self.ux_vitals();
        let synthetics = self.ux_synthetics();
        let errors = sessions.iter().filter(|s| s["has_error"].as_bool() == Some(true)).count();
        let bounce = sessions
            .iter()
            .filter(|s| s["bounced"].as_bool() == Some(true))
            .count() as f64
            / sessions.len().max(1) as f64;
        let syn_alert = synthetics.iter().filter(|s| s["status"].as_str() == Some("alert")).count();
        json!({
            "sessions_24h": 18_420,
            "active_users": 3_842,
            "bounce_rate": (bounce * 1000.0).round() / 1000.0,
            "error_sessions": errors,
            "avg_session_ms": 142_000,
            "views_per_session": 4.6,
            "web_vitals_ok_pct": 91.2,
            "conversion_rate": 0.042,
            "revenue_at_risk_usd": if syn_alert > 0 { 48_200 } else { 0 },
            "significance": if syn_alert > 0 {
                "Checkout synthetic failing + RUM bounce spike — ~$48k/day revenue at risk until path recovers"
            } else {
                "UX healthy — Web Vitals within budget and synthetics green"
            },
            "synthetics_ok": synthetics.iter().filter(|s| s["status"].as_str() == Some("ok")).count(),
            "synthetics_total": synthetics.len(),
            "lcp_p75_ms": vitals.iter().find(|v| v["name"].as_str() == Some("LCP")).and_then(|v| v["p75_ms"].as_f64()).unwrap_or(0.0),
            "cls_p75": vitals.iter().find(|v| v["name"].as_str() == Some("CLS")).and_then(|v| v["p75"].as_f64()).unwrap_or(0.0),
            "inp_p75_ms": vitals.iter().find(|v| v["name"].as_str() == Some("INP")).and_then(|v| v["p75_ms"].as_f64()).unwrap_or(0.0),
        })
    }

    pub fn ux_sessions(&self) -> Vec<serde_json::Value> {
        let now = Utc::now().timestamp_millis();
        vec![
            json!({"id":"sess-a1","user":"anon-4821","view":"/checkout","country":"US","device":"desktop","browser":"Chrome","duration_ms":186000,"bounced":false,"has_error":true,"frustration":"rage_click","impact":"high","started_ms": now - 400_000}),
            json!({"id":"sess-b2","user":"anon-9910","view":"/pricing","country":"DE","device":"mobile","browser":"Safari","duration_ms":22000,"bounced":true,"has_error":false,"frustration":null,"impact":"medium","started_ms": now - 320_000}),
            json!({"id":"sess-c3","user":"alice@acme.io","view":"/app/dashboards","country":"US","device":"desktop","browser":"Firefox","duration_ms":540000,"bounced":false,"has_error":true,"frustration":"dead_click","impact":"medium","started_ms": now - 280_000}),
            json!({"id":"sess-d4","user":"anon-1102","view":"/docs/install","country":"IN","device":"desktop","browser":"Chrome","duration_ms":98000,"bounced":false,"has_error":false,"frustration":null,"impact":"low","started_ms": now - 210_000}),
            json!({"id":"sess-e5","user":"bob@acme.io","view":"/app/monitors","country":"GB","device":"tablet","browser":"Chrome","duration_ms":301000,"bounced":false,"has_error":false,"frustration":null,"impact":"low","started_ms": now - 150_000}),
            json!({"id":"sess-f6","user":"anon-7744","view":"/signup","country":"BR","device":"mobile","browser":"Chrome","duration_ms":41000,"bounced":true,"has_error":true,"frustration":"error_click","impact":"high","started_ms": now - 90_000}),
            json!({"id":"sess-g7","user":"anon-2201","view":"/status","country":"JP","device":"desktop","browser":"Edge","duration_ms":67000,"bounced":false,"has_error":false,"frustration":null,"impact":"low","started_ms": now - 55_000}),
            json!({"id":"sess-h8","user":"carol@acme.io","view":"/app/apm","country":"US","device":"desktop","browser":"Chrome","duration_ms":412000,"bounced":false,"has_error":false,"frustration":null,"impact":"low","started_ms": now - 30_000}),
            json!({"id":"sess-i9","user":"anon-5510","view":"/checkout","country":"US","device":"mobile","browser":"Safari","duration_ms":18000,"bounced":true,"has_error":true,"frustration":"rage_click","impact":"critical","started_ms": now - 25_000}),
            json!({"id":"sess-j10","user":"dana@acme.io","view":"/app/logs","country":"CA","device":"desktop","browser":"Chrome","duration_ms":255000,"bounced":false,"has_error":false,"frustration":null,"impact":"low","started_ms": now - 18_000}),
            json!({"id":"sess-k11","user":"anon-3300","view":"/checkout/pay","country":"US","device":"desktop","browser":"Chrome","duration_ms":92000,"bounced":false,"has_error":true,"frustration":"js_error","impact":"critical","started_ms": now - 12_000}),
            json!({"id":"sess-l12","user":"anon-8801","view":"/pricing","country":"FR","device":"mobile","browser":"Firefox","duration_ms":45000,"bounced":true,"has_error":false,"frustration":null,"impact":"medium","started_ms": now - 6_000}),
        ]
    }

    pub fn ux_vitals(&self) -> Vec<serde_json::Value> {
        vec![
            json!({"name":"LCP","p75_ms":2140.0,"good_pct":88.0,"needs_improvement_pct":9.0,"poor_pct":3.0,"budget_ms":2500.0,"significance":"Near budget — checkout hero image delays mobile LCP"}),
            json!({"name":"INP","p75_ms":168.0,"good_pct":92.0,"needs_improvement_pct":6.0,"poor_pct":2.0,"budget_ms":200.0,"significance":"OK — pay button main-thread still under 200ms"}),
            json!({"name":"CLS","p75":0.08,"good_pct":94.0,"needs_improvement_pct":4.0,"poor_pct":2.0,"budget":0.1,"significance":"OK — banner A/B test caused prior week regressions"}),
            json!({"name":"TTFB","p75_ms":420.0,"good_pct":86.0,"needs_improvement_pct":10.0,"poor_pct":4.0,"budget_ms":800.0,"significance":"Edge cache miss rate up in eu-west during deploy"}),
            json!({"name":"FCP","p75_ms":1180.0,"good_pct":90.0,"needs_improvement_pct":7.0,"poor_pct":3.0,"budget_ms":1800.0,"significance":"Healthy — font subsetting holding FCP"}),
        ]
    }

    pub fn ux_synthetics(&self) -> Vec<serde_json::Value> {
        let now = Utc::now().timestamp_millis();
        vec![
            json!({"id":"syn-home","name":"Homepage load","type":"browser","url":"https://app.thine.dev/","locations":["aws:us-east-1","aws:eu-west-1"],"status":"ok","latency_ms":812,"last_run_ms": now - 60_000,"significance":"Public entry — SEO + first impression"}),
            json!({"id":"syn-api","name":"API health","type":"api","url":"https://app.thine.dev/api/v1/health","locations":["aws:us-east-1"],"status":"ok","latency_ms":94,"last_run_ms": now - 30_000,"significance":"Backend liveness for mobile + web"}),
            json!({"id":"syn-login","name":"Login journey","type":"browser","url":"https://app.thine.dev/login","locations":["aws:us-west-2","gcp:us-central1"],"status":"ok","latency_ms":2400,"last_run_ms": now - 120_000,"significance":"Auth funnel — blocks all product surfaces"}),
            json!({"id":"syn-checkout","name":"Checkout critical path","type":"browser","url":"https://shop.thine.dev/checkout","locations":["aws:us-east-1"],"status":"alert","latency_ms":6100,"last_run_ms": now - 45_000,"significance":"Revenue path failing — correlates with payments.retry_v2 + API latency"}),
            json!({"id":"syn-docs","name":"Docs CDN","type":"api","url":"https://docs.thine.dev/","locations":["cloudflare:global"],"status":"ok","latency_ms":140,"last_run_ms": now - 15_000,"significance":"Install docs — agent onboarding conversion"}),
            json!({"id":"syn-signup","name":"Signup form","type":"browser","url":"https://app.thine.dev/signup","locations":["aws:eu-west-1"],"status":"ok","latency_ms":1900,"last_run_ms": now - 80_000,"significance":"Top-of-funnel acquisition"}),
        ]
    }

    pub fn ux_timeseries(&self) -> Vec<serde_json::Value> {
        let now = Utc::now().timestamp_millis();
        (0..24)
            .map(|i| {
                let t = now - (23 - i) * 3600_000;
                let hour = i as f64;
                let sessions = 600.0 + (hour * 0.7).sin().abs() * 220.0 + if (12..=16).contains(&i) { 180.0 } else { 0.0 };
                let errors = if (13..=15).contains(&i) { 42.0 + (i as f64 - 13.0) * 8.0 } else { 6.0 + (hour % 5.0) };
                let bounce = if (13..=15).contains(&i) { 0.41 } else { 0.28 };
                let lcp = if (13..=15).contains(&i) { 2800.0 } else { 1900.0 + (hour % 3.0) * 40.0 };
                json!({
                    "t": t,
                    "label": format!("{:02}:00", i),
                    "sessions": sessions.round(),
                    "errors": errors.round(),
                    "bounce_pct": ((bounce * 100.0) as f64).round(),
                    "lcp_p75_ms": lcp.round(),
                })
            })
            .collect()
    }

    pub fn ux_pages(&self) -> Vec<serde_json::Value> {
        vec![
            json!({"path":"/checkout","views":8420,"avg_lcp_ms":2680,"error_rate":0.086,"bounce_rate":0.39,"impact":"critical","note":"Synthetic + RUM both red — primary revenue"}),
            json!({"path":"/checkout/pay","views":5102,"avg_lcp_ms":2440,"error_rate":0.112,"bounce_rate":0.22,"impact":"critical","note":"JS error PaymentServiceUnavailable"}),
            json!({"path":"/pricing","views":12110,"avg_lcp_ms":1580,"error_rate":0.012,"bounce_rate":0.44,"impact":"medium","note":"High bounce expected; watch mobile"}),
            json!({"path":"/signup","views":3900,"avg_lcp_ms":1720,"error_rate":0.031,"bounce_rate":0.35,"impact":"high","note":"Acquisition — form validation friction"}),
            json!({"path":"/app/dashboards","views":6200,"avg_lcp_ms":1410,"error_rate":0.008,"bounce_rate":0.11,"impact":"low","note":"Product sticky — healthy"}),
            json!({"path":"/docs/install","views":2800,"avg_lcp_ms":980,"error_rate":0.004,"bounce_rate":0.19,"impact":"medium","note":"Agent install docs — Fleet conversion"}),
        ]
    }

    pub fn ux_funnel(&self) -> Vec<serde_json::Value> {
        vec![
            json!({"step":"Landing","users":10000,"drop_pct":0.0}),
            json!({"step":"Pricing","users":6200,"drop_pct":38.0}),
            json!({"step":"Signup","users":2100,"drop_pct":66.1}),
            json!({"step":"Install agent","users":980,"drop_pct":53.3}),
            json!({"step":"First dashboard","users":640,"drop_pct":34.7}),
            json!({"step":"First monitor","users":410,"drop_pct":35.9}),
        ]
    }

    // —— Mobile / IDE ——
    pub fn mobile_config(&self) -> MobileConfig {
        self.mobile.read().clone()
    }

    pub fn set_mobile_config(&self, cfg: MobileConfig) -> MobileConfig {
        *self.mobile.write() = cfg.clone();
        cfg
    }

    pub fn list_ide_plugins(&self) -> Vec<IdePlugin> {
        self.ide_plugins.iter().map(|e| e.value().clone()).collect()
    }

    // —— Marketplace ——
    pub fn install_app(&self, id: &str) -> Option<MarketplaceApp> {
        let mut app = self.marketplace.get_mut(id)?;
        app.installed = true;
        Some(app.clone())
    }

    pub fn uninstall_app(&self, id: &str) -> Option<MarketplaceApp> {
        let mut app = self.marketplace.get_mut(id)?;
        app.installed = false;
        Some(app.clone())
    }

    // —— MCP tool invoke ——
    pub fn mcp_invoke(&self, tool: &str, args: &serde_json::Value) -> serde_json::Value {
        match tool {
            "query_metrics" => {
                let metric = args.get("metric").and_then(|v| v.as_str()).unwrap_or("");
                let list = self.metrics.list_metrics(Some(metric));
                json!({ "ok": true, "series": list.len(), "sample": list.into_iter().take(5).collect::<Vec<_>>() })
            }
            "list_slos" => json!({ "ok": true, "slos": self.list_slos() }),
            "search_logs" => {
                let service = args.get("service").and_then(|v| v.as_str());
                json!({ "ok": true, "logs": self.search_logs(service, None, 20) })
            }
            "list_incidents" => json!({ "ok": true, "incidents": self.list_incidents() }),
            "run_tutorial" => {
                let r = self.bits_run_tutorial();
                json!({ "ok": true, "result": r })
            }
            "install_agent" => {
                let platform = args.get("platform").and_then(|v| v.as_str());
                json!({ "ok": true, "result": self.bits_install_agent(platform) })
            }
            "create_dashboard" => json!({ "ok": true, "result": self.bits_create_tutorial_dashboard() }),
            "create_monitor" => {
                let hint = args
                    .get("hint")
                    .and_then(|v| v.as_str())
                    .unwrap_or("cpu latency recovery");
                json!({ "ok": true, "result": self.bits_create_tutorial_monitors(hint) })
            }
            "list_fleet" => {
                json!({ "ok": true, "agents": self.list_fleet(), "summary": self.fleet_summary() })
            }
            _ => json!({ "ok": false, "error": "unknown tool" }),
        }
    }

    // —— USM graph ——
    pub fn usm_map(&self) -> serde_json::Value {
        let services = self.list_apm_services();
        let edges: Vec<_> = self
            .network_flows
            .read()
            .iter()
            .filter_map(|f| {
                Some(json!({
                    "from": f.get("src")?,
                    "to": f.get("dst")?,
                    "bytes": f.get("bytes")?,
                    "protocol": f.get("protocol")?
                }))
            })
            .collect();
        json!({ "services": services, "edges": edges, "detected_via": ["network", "apm"] })
    }

    // —— CLI schema ——
    pub fn cli_schema(&self) -> serde_json::Value {
        json!({
            "name": "thine",
            "version": env!("CARGO_PKG_VERSION"),
            "commands": [
                {"name": "status", "path": "/health", "method": "GET"},
                {"name": "query", "path": "/api/v1/query", "method": "GET", "args": ["metric", "tags"]},
                {"name": "logs", "path": "/api/v1/logs/search", "method": "GET", "args": ["service", "level"]},
                {"name": "slo", "path": "/api/v1/slos", "method": "GET"},
                {"name": "catalog", "path": "/api/v1/catalog/services", "method": "GET"},
                {"name": "bits", "path": "/api/v1/bits/chat", "method": "POST", "args": ["message"]},
                {"name": "features", "path": "/api/v1/features", "method": "GET"}
            ]
        })
    }

    pub fn seed_deep(&self) {
        let _ = self.issue_token("alice@thine.dev", vec!["admin".into()]);
        let viewer = self.issue_token("bob@thine.dev", vec!["viewer".into()]);
        let _ = viewer;

        self.upsert_agent(AgentInfo {
            id: "bits-sre".into(),
            name: "SRE Copilot".into(),
            capabilities: vec!["metrics".into(), "logs".into(), "incidents".into(), "slos".into()],
        });
        self.upsert_agent(AgentInfo {
            id: "bits-cost".into(),
            name: "Cost Analyst".into(),
            capabilities: vec!["cost".into(), "catalog".into()],
        });

        self.fleet_heartbeat("agent-1", "0.1.0", "i-api-1", Some("linux"));
        self.fleet_heartbeat("agent-2", "0.1.0", "i-worker-1", Some("linux"));

        let now = Utc::now().timestamp_millis();
        self.ingest_flows(vec![
            NetworkFlow {
                id: "f1".into(),
                src: "api".into(),
                dst: "postgres".into(),
                protocol: "tcp".into(),
                bytes: 1_500_000,
                packets: 12_000,
                timestamp_ms: now,
            },
            NetworkFlow {
                id: "f2".into(),
                src: "api".into(),
                dst: "redis".into(),
                protocol: "tcp".into(),
                bytes: 400_000,
                packets: 8_000,
                timestamp_ms: now,
            },
            NetworkFlow {
                id: "f3".into(),
                src: "worker".into(),
                dst: "kafka".into(),
                protocol: "tcp".into(),
                bytes: 900_000,
                packets: 5_000,
                timestamp_ms: now,
            },
        ]);

        self.upsert_autoscaler(Autoscaler {
            name: "api-hpa".into(),
            namespace: "default".into(),
            min: 2,
            max: 20,
            current: 4,
            recommended: 4,
            metric: "cpu".into(),
            target_cpu: 0.65,
        });
        self.upsert_autoscaler(Autoscaler {
            name: "worker-hpa".into(),
            namespace: "default".into(),
            min: 1,
            max: 10,
            current: 2,
            recommended: 2,
            metric: "cpu".into(),
            target_cpu: 0.7,
        });

        for (name, runtime, inv, err, cold, dur, mem, cloud, region, svc, env, cost, kind, mem_pct) in [
            ("checkout", "provided.al2023", 12400u64, 18u64, 12u64, 145.0, 512u32, "aws", "us-east-1", "checkout", "prod", 4.2, "lambda", 62.0),
            ("notify", "nodejs20.x", 50000, 2, 40, 32.0, 256, "aws", "us-east-1", "notify", "prod", 2.8, "lambda", 41.0),
            ("ingest-events", "python3.12", 88000, 45, 120, 210.0, 1024, "aws", "us-east-1", "ingest", "prod", 9.5, "lambda", 78.0),
            ("order-workflow", "nodejs20.x", 3200, 5, 8, 890.0, 512, "aws", "us-east-1", "orders", "prod", 3.1, "step_function", 55.0),
            ("payments-webhook", "java21", 15000, 22, 30, 180.0, 1024, "aws", "us-west-2", "payments", "prod", 5.4, "lambda", 70.0),
            ("image-resize", "nodejs20.x", 6200, 1, 90, 420.0, 1536, "aws", "us-east-1", "media", "prod", 6.8, "lambda", 85.0),
            ("web-storefront", "dotnet8", 0, 0, 0, 0.0, 0, "azure", "eastus", "storefront", "prod", 12.0, "azure_app", 48.0),
            ("billing-api", "python3.11", 0, 0, 0, 0.0, 0, "azure", "eastus", "billing", "prod", 8.5, "azure_app", 52.0),
            ("edge-renderer", "go", 22000, 8, 15, 95.0, 512, "gcp", "us-central1", "edge", "prod", 3.9, "cloud_run", 44.0),
            ("ml-score", "python3.12", 4100, 12, 55, 680.0, 2048, "gcp", "us-central1", "ml", "prod", 11.2, "cloud_run", 88.0),
            ("checkout-stg", "provided.al2023", 800, 3, 25, 160.0, 512, "aws", "us-east-1", "checkout", "staging", 0.4, "lambda", 35.0),
            ("notify-dev", "nodejs20.x", 120, 0, 40, 28.0, 256, "aws", "us-east-1", "notify", "dev", 0.1, "lambda", 22.0),
        ] {
            self.upsert_function(ServerlessFunction {
                name: name.into(),
                runtime: runtime.into(),
                invocations_24h: inv,
                errors_24h: err,
                cold_starts_24h: cold,
                avg_duration_ms: dur,
                memory_mb: mem,
                cloud: cloud.into(),
                region: region.into(),
                service: svc.into(),
                env: env.into(),
                estimated_cost_24h: cost,
                timeout_errors_24h: if err > 10 { err / 4 } else { 0 },
                oom_errors_24h: if mem_pct > 80.0 { 2 } else { 0 },
                memory_used_pct: mem_pct,
                kind: kind.into(),
            });
        }

        self.upload_profile(
            ProfileMeta {
                id: "api-cpu".into(),
                service: "api".into(),
                profile_type: "cpu".into(),
                duration_ms: 60_000,
            },
            Some(base64_encode(b"fake-pprof-bytes")),
        );

        self.upsert_probe(DynProbe {
            id: "probe-1".into(),
            service: "api".into(),
            language: "go".into(),
            method: "checkout.Handler.Pay".into(),
            enabled: true,
            capture: vec!["req.Amount".into(), "err".into()],
        });

        self.add_query_sample(DbQuerySample {
            sql: "SELECT * FROM orders WHERE id = $1".into(),
            duration_ms: 12.5,
            calls: 40_000,
            db: "pg-primary".into(),
        });
        self.add_query_sample(DbQuerySample {
            sql: "UPDATE inventory SET qty = qty - 1 WHERE sku = $1".into(),
            duration_ms: 8.2,
            calls: 22_000,
            db: "pg-primary".into(),
        });

        self.upsert_stream(StreamInfo {
            name: "orders".into(),
            lag: 42,
            throughput: 1500.0,
        });
        self.upsert_stream(StreamInfo {
            name: "payments".into(),
            lag: 3,
            throughput: 800.0,
        });

        self.upsert_pipeline(Pipeline {
            id: "default-logs".into(),
            name: "Default logs pipeline".into(),
            processors: vec!["grok".into(), "remap".into(), "filter_debug".into()],
        });

        self.upsert_byoc(ByocSink {
            id: "s3-archive".into(),
            provider: "s3".into(),
            bucket: "thine-logs-archive".into(),
            prefix: "prod/".into(),
            enabled: true,
            forwarded: 0,
        });

        *self.mobile.write() = MobileConfig {
            app_name: "Thine Mobile".into(),
            min_ios: "16.0".into(),
            min_android: "12".into(),
            push_enabled: true,
            deep_links: vec![
                "thine://incident/{id}".into(),
                "thine://dashboard/{id}".into(),
            ],
        };

        self.ide_plugins.insert(
            "vscode".into(),
            IdePlugin {
                id: "vscode".into(),
                ide: "VS Code".into(),
                version: "0.1.0".into(),
                download_url: "https://github.com/Jubayer1228/thine-metrics/releases/vscode".into(),
                features: vec!["metrics hover".into(), "slo gutter".into(), "log search".into()],
            },
        );
        self.ide_plugins.insert(
            "jetbrains".into(),
            IdePlugin {
                id: "jetbrains".into(),
                ide: "JetBrains".into(),
                version: "0.1.0".into(),
                download_url: "https://github.com/Jubayer1228/thine-metrics/releases/jetbrains"
                    .into(),
                features: vec!["APM links".into(), "error insights".into()],
            },
        );

        // seed a log with email for SDS
        self.ingest_log(LogEvent {
            timestamp_ms: now,
            level: "warn".into(),
            service: "api".into(),
            message: "password reset requested for user alice@example.com".into(),
            attrs: BTreeMap::new(),
        });
    }
}

fn base64_encode(bytes: &[u8]) -> String {
    const T: &[u8] =
        b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::new();
    for chunk in bytes.chunks(3) {
        let a = chunk[0] as u32;
        let b = chunk.get(1).copied().unwrap_or(0) as u32;
        let c = chunk.get(2).copied().unwrap_or(0) as u32;
        let n = (a << 16) | (b << 8) | c;
        out.push(T[((n >> 18) & 63) as usize] as char);
        out.push(T[((n >> 12) & 63) as usize] as char);
        out.push(if chunk.len() > 1 {
            T[((n >> 6) & 63) as usize] as char
        } else {
            '='
        });
        out.push(if chunk.len() > 2 {
            T[(n & 63) as usize] as char
        } else {
            '='
        });
    }
    out
}

trait IfEmpty {
    fn if_empty(self, fallback: &str) -> String;
}
impl IfEmpty for String {
    fn if_empty(self, fallback: &str) -> String {
        if self.is_empty() {
            fallback.into()
        } else {
            self
        }
    }
}

fn hostmap_group_key(h: &HostInfo, group_by: &str) -> String {
    match group_by {
        "az" | "availability-zone" => h.az.clone().if_empty("unknown"),
        "service" => h.service.clone().if_empty("untagged"),
        "instance_type" | "instance-type" => h.instance_type.clone().if_empty("unknown"),
        "env" => h.env.clone().if_empty("untagged"),
        "cloud_provider" => h
            .tags
            .get("cloud_provider")
            .cloned()
            .unwrap_or_else(|| "unknown".into()),
        "kube_namespace" | "kube_cluster_name" => h
            .tags
            .get("kube_namespace")
            .cloned()
            .unwrap_or_else(|| "none".into()),
        "kube_deployment" => h
            .tags
            .get("kube_deployment")
            .cloned()
            .unwrap_or_else(|| "none".into()),
        _ => h
            .tags
            .get(group_by)
            .cloned()
            .unwrap_or_else(|| h.env.clone().if_empty("untagged")),
    }
}

/// Datadog Containers Explorer-style search: AND (default), OR, NOT / !.
fn container_query_match(blob: &str, query: &str) -> bool {
    let q = query.trim();
    if q.is_empty() {
        return true;
    }
    if q.contains(" or ") {
        return q
            .split(" or ")
            .any(|part| container_query_match(blob, part.trim()));
    }
    let tokens: Vec<&str> = q.split_whitespace().collect();
    let mut i = 0;
    while i < tokens.len() {
        let mut term = tokens[i];
        let mut negate = false;
        if term.eq_ignore_ascii_case("and") {
            i += 1;
            continue;
        }
        if term.eq_ignore_ascii_case("not") {
            negate = true;
            i += 1;
            if i >= tokens.len() {
                break;
            }
            term = tokens[i];
        } else if let Some(rest) = term.strip_prefix('!') {
            negate = true;
            term = rest;
        }
        if term.is_empty() {
            i += 1;
            continue;
        }
        let hit = blob.contains(&term.to_ascii_lowercase());
        if negate && hit {
            return false;
        }
        if !negate && !hit {
            return false;
        }
        i += 1;
    }
    true
}

fn facet_count<I>(iter: I) -> Vec<serde_json::Value>
where
    I: IntoIterator<Item = String>,
{
    let mut map: BTreeMap<String, u64> = BTreeMap::new();
    for k in iter {
        *map.entry(k).or_insert(0) += 1;
    }
    let mut v: Vec<_> = map
        .into_iter()
        .map(|(k, n)| json!({"value": k, "count": n}))
        .collect();
    v.sort_by(|a, b| {
        b["count"]
            .as_u64()
            .unwrap_or(0)
            .cmp(&a["count"].as_u64().unwrap_or(0))
    });
    v
}

fn gauge(name: &str, tags: &Tags, ts: i64, value: f64, unit: &str) -> MetricPoint {
    MetricPoint {
        name: name.into(),
        metric_type: MetricType::Gauge,
        tags: tags.clone(),
        sample: Sample {
            timestamp_ms: ts,
            value,
        },
        unit: Some(unit.into()),
        description: None,
    }
}
