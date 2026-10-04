//! Extended models for deep Datadog-parity features.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApiToken {
    pub token: String,
    pub user_email: String,
    pub roles: Vec<String>,
    pub created_at_ms: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NetworkFlow {
    pub id: String,
    pub src: String,
    pub dst: String,
    pub protocol: String,
    pub bytes: u64,
    pub packets: u64,
    pub timestamp_ms: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Autoscaler {
    pub name: String,
    pub namespace: String,
    pub min: u32,
    pub max: u32,
    pub current: u32,
    pub recommended: u32,
    pub metric: String,
    pub target_cpu: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServerlessFunction {
    pub name: String,
    pub runtime: String,
    pub invocations_24h: u64,
    pub errors_24h: u64,
    pub cold_starts_24h: u64,
    pub avg_duration_ms: f64,
    pub memory_mb: u32,
    #[serde(default = "default_cloud")]
    pub cloud: String,
    #[serde(default = "default_region")]
    pub region: String,
    #[serde(default)]
    pub service: String,
    #[serde(default)]
    pub env: String,
    #[serde(default)]
    pub estimated_cost_24h: f64,
    #[serde(default)]
    pub timeout_errors_24h: u64,
    #[serde(default)]
    pub oom_errors_24h: u64,
    #[serde(default)]
    pub memory_used_pct: f64,
    #[serde(default)]
    pub kind: String, // lambda | step_function | azure_app | cloud_run | container_app
}

fn default_cloud() -> String {
    "aws".into()
}
fn default_region() -> String {
    "us-east-1".into()
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DynProbe {
    pub id: String,
    pub service: String,
    pub language: String,
    pub method: String,
    pub enabled: bool,
    pub capture: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ByocSink {
    pub id: String,
    pub provider: String,
    pub bucket: String,
    pub prefix: String,
    pub enabled: bool,
    pub forwarded: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SdsFinding {
    pub rule_id: String,
    pub service: String,
    pub snippet: String,
    pub timestamp_ms: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MobileConfig {
    pub app_name: String,
    pub min_ios: String,
    pub min_android: String,
    pub push_enabled: bool,
    pub deep_links: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IdePlugin {
    pub id: String,
    pub ide: String,
    pub version: String,
    pub download_url: String,
    pub features: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DbQuerySample {
    pub sql: String,
    pub duration_ms: f64,
    pub calls: u64,
    pub db: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CostLine {
    pub service: String,
    pub compute: f64,
    pub storage: f64,
    pub network: f64,
    pub total: f64,
}
