//! Granular GPU monitoring — Datadog/DCGM-parity device, process, and pipeline metrics.
//!
//! Metric namespace mirrors Datadog GPU Monitoring + NVIDIA DCGM exporter fields:
//! `gpu.*` gauges/counters tagged by `gpu_id`, `host`, `model`, `uuid`, `pci_bus_id`.

use chrono::Utc;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::BTreeMap;
use thine_common::{MetricPoint, MetricType, Sample, Tags};

use crate::state::PlatformState;

const MAX_GPU_SAMPLES: usize = 2_000;

/// Full GPU device inventory + latest snapshot (NVML/DCGM-aligned).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GpuDevice {
    pub id: String,
    #[serde(default)]
    pub uuid: String,
    pub model: String,
    #[serde(default)]
    pub host: String,
    #[serde(default)]
    pub pci_bus_id: String,
    #[serde(default)]
    pub driver_version: String,
    #[serde(default)]
    pub cuda_version: String,
    /// Legacy alias — GPU util % (0–100). Prefer `sm_active` / `gpu_util`.
    #[serde(default)]
    pub util: f64,
    /// Frame-buffer / HBM used (MiB). Prefer memory_used_mb under memory block.
    #[serde(default)]
    pub memory_used_mb: f64,

    // —— Utilization ——
    /// DCGM_FI_DEV_GPU_UTIL / gpu utilization %
    #[serde(default)]
    pub gpu_util: f64,
    /// DCGM_FI_PROF_SM_ACTIVE — % of cycles an SM has ≥1 warp
    #[serde(default)]
    pub sm_active: f64,
    /// DCGM_FI_PROF_SM_OCCUPANCY — warp residency / max warps
    #[serde(default)]
    pub sm_occupancy: f64,
    /// DCGM_FI_PROF_GR_ENGINE_ACTIVE
    #[serde(default)]
    pub gr_engine_active: f64,
    /// DCGM_FI_DEV_MEM_COPY_UTIL
    #[serde(default)]
    pub mem_copy_util: f64,
    #[serde(default)]
    pub enc_utilization: f64,
    #[serde(default)]
    pub dec_utilization: f64,

    // —— Memory ——
    #[serde(default)]
    pub memory_total_mb: f64,
    #[serde(default)]
    pub memory_free_mb: f64,
    #[serde(default)]
    pub memory_reserved_mb: f64,
    #[serde(default)]
    pub memory_used_percent: f64,
    /// DCGM_FI_PROF_DRAM_ACTIVE
    #[serde(default)]
    pub dram_active: f64,

    // —— Clocks ——
    #[serde(default)]
    pub sm_clock_mhz: f64,
    #[serde(default)]
    pub mem_clock_mhz: f64,
    #[serde(default)]
    pub graphics_clock_mhz: f64,
    #[serde(default)]
    pub pstate: i32,

    // —— Power / energy / thermals ——
    #[serde(default)]
    pub power_usage_w: f64,
    #[serde(default)]
    pub power_management_limit_w: f64,
    #[serde(default)]
    pub total_energy_consumption_mj: f64,
    #[serde(default)]
    pub temperature_c: f64,
    #[serde(default)]
    pub memory_temperature_c: f64,
    #[serde(default)]
    pub fan_speed_pct: f64,

    // —— Pipeline / tensor (Hopper+) ——
    #[serde(default)]
    pub pipe_fp16_active: f64,
    #[serde(default)]
    pub pipe_fp32_active: f64,
    #[serde(default)]
    pub pipe_fp64_active: f64,
    #[serde(default)]
    pub pipe_tensor_active: f64,
    #[serde(default)]
    pub pipe_integer_active: f64,

    // —— Interconnect / PCIe ——
    #[serde(default)]
    pub pcie_replay_total: u64,
    #[serde(default)]
    pub nvlink_bandwidth_total: f64,
    #[serde(default)]
    pub nvlink_replay_errors: u64,

    // —— Health ——
    #[serde(default)]
    pub xid_errors_total: u64,
    #[serde(default)]
    pub remapped_rows_pending: u64,
    #[serde(default)]
    pub remapped_rows_failed: u64,
    #[serde(default)]
    pub remapped_rows_correctable: u64,
    #[serde(default)]
    pub remapped_rows_uncorrectable: u64,
    /// Bitfield-style reasons as named flags (Datadog gpu.clock.throttle_reasons.*)
    #[serde(default)]
    pub throttle_reasons: GpuThrottleReasons,
    #[serde(default)]
    pub ecc_sbe_volatile_total: u64,
    #[serde(default)]
    pub ecc_dbe_volatile_total: u64,
    #[serde(default)]
    pub health_status: String,
    #[serde(default)]
    pub tags: BTreeMap<String, String>,
    #[serde(default)]
    pub last_seen_ms: i64,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct GpuThrottleReasons {
    #[serde(default)]
    pub gpu_idle: bool,
    #[serde(default)]
    pub applications_clocks: bool,
    #[serde(default)]
    pub sw_power_cap: bool,
    #[serde(default)]
    pub hw_slowdown: bool,
    #[serde(default)]
    pub sync_boost: bool,
    #[serde(default)]
    pub sw_thermal_slowdown: bool,
    #[serde(default)]
    pub hw_thermal_slowdown: bool,
    #[serde(default)]
    pub hw_power_brake: bool,
}

/// Process / pod attribution on a GPU (Datadog connected entities).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GpuProcess {
    pub pid: u32,
    pub gpu_id: String,
    pub host: String,
    pub process_name: String,
    #[serde(default)]
    pub cmdline: String,
    #[serde(default)]
    pub container_id: Option<String>,
    #[serde(default)]
    pub pod: Option<String>,
    #[serde(default)]
    pub namespace: Option<String>,
    #[serde(default)]
    pub sm_active: f64,
    #[serde(default)]
    pub memory_usage_mb: f64,
    #[serde(default)]
    pub gpu_util: f64,
    #[serde(default)]
    pub team: Option<String>,
    #[serde(default)]
    pub workload: Option<String>,
}

/// Point-in-time sample retained for timeseries / effectiveness demos.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GpuSample {
    pub timestamp_ms: i64,
    pub gpu_id: String,
    pub sm_active: f64,
    pub sm_occupancy: f64,
    pub gpu_util: f64,
    pub memory_used_percent: f64,
    pub power_usage_w: f64,
    pub temperature_c: f64,
    pub pipe_tensor_active: f64,
    pub gr_engine_active: f64,
}

/// Catalog entry describing a collected metric.
#[derive(Debug, Clone, Serialize)]
pub struct GpuMetricDef {
    pub name: &'static str,
    pub dcgm_field: &'static str,
    pub metric_type: &'static str,
    pub unit: &'static str,
    pub description: &'static str,
    pub group: &'static str,
}

pub fn gpu_metric_catalog() -> Vec<GpuMetricDef> {
    vec![
        GpuMetricDef {
            name: "gpu.utilization",
            dcgm_field: "DCGM_FI_DEV_GPU_UTIL",
            metric_type: "gauge",
            unit: "percent",
            description: "GPU utilization (%)",
            group: "utilization",
        },
        GpuMetricDef {
            name: "gpu.sm_active",
            dcgm_field: "DCGM_FI_PROF_SM_ACTIVE",
            metric_type: "gauge",
            unit: "percent",
            description: "Fraction of cycles an SM has at least one warp assigned",
            group: "utilization",
        },
        GpuMetricDef {
            name: "gpu.sm_occupancy",
            dcgm_field: "DCGM_FI_PROF_SM_OCCUPANCY",
            metric_type: "gauge",
            unit: "percent",
            description: "Ratio of resident warps to max warps per SM (saturation)",
            group: "utilization",
        },
        GpuMetricDef {
            name: "gpu.gr_engine_active",
            dcgm_field: "DCGM_FI_PROF_GR_ENGINE_ACTIVE",
            metric_type: "gauge",
            unit: "percent",
            description: "Graphics/compute engine activity",
            group: "utilization",
        },
        GpuMetricDef {
            name: "gpu.memory.copy_utilization",
            dcgm_field: "DCGM_FI_DEV_MEM_COPY_UTIL",
            metric_type: "gauge",
            unit: "percent",
            description: "Memory copy engine utilization",
            group: "memory",
        },
        GpuMetricDef {
            name: "gpu.memory.used",
            dcgm_field: "DCGM_FI_DEV_FB_USED",
            metric_type: "gauge",
            unit: "mebibyte",
            description: "Frame buffer / HBM used",
            group: "memory",
        },
        GpuMetricDef {
            name: "gpu.memory.free",
            dcgm_field: "DCGM_FI_DEV_FB_FREE",
            metric_type: "gauge",
            unit: "mebibyte",
            description: "Frame buffer free",
            group: "memory",
        },
        GpuMetricDef {
            name: "gpu.memory.total",
            dcgm_field: "DCGM_FI_DEV_FB_TOTAL",
            metric_type: "gauge",
            unit: "mebibyte",
            description: "Frame buffer total",
            group: "memory",
        },
        GpuMetricDef {
            name: "gpu.memory.reserved",
            dcgm_field: "DCGM_FI_DEV_FB_RESERVED",
            metric_type: "gauge",
            unit: "mebibyte",
            description: "Frame buffer reserved",
            group: "memory",
        },
        GpuMetricDef {
            name: "gpu.memory.used_percent",
            dcgm_field: "DCGM_FI_DEV_FB_USED_PERCENT",
            metric_type: "gauge",
            unit: "percent",
            description: "Memory used as percent of total",
            group: "memory",
        },
        GpuMetricDef {
            name: "gpu.dram_active",
            dcgm_field: "DCGM_FI_PROF_DRAM_ACTIVE",
            metric_type: "gauge",
            unit: "percent",
            description: "DRAM bandwidth activity",
            group: "memory",
        },
        GpuMetricDef {
            name: "gpu.enc_utilization",
            dcgm_field: "DCGM_FI_DEV_ENC_UTIL",
            metric_type: "gauge",
            unit: "percent",
            description: "Video encoder utilization",
            group: "utilization",
        },
        GpuMetricDef {
            name: "gpu.dec_utilization",
            dcgm_field: "DCGM_FI_DEV_DEC_UTIL",
            metric_type: "gauge",
            unit: "percent",
            description: "Video decoder utilization",
            group: "utilization",
        },
        GpuMetricDef {
            name: "gpu.temperature",
            dcgm_field: "DCGM_FI_DEV_GPU_TEMP",
            metric_type: "gauge",
            unit: "celsius",
            description: "GPU die temperature",
            group: "thermal",
        },
        GpuMetricDef {
            name: "gpu.memory.temperature",
            dcgm_field: "DCGM_FI_DEV_MEMORY_TEMP",
            metric_type: "gauge",
            unit: "celsius",
            description: "Memory temperature",
            group: "thermal",
        },
        GpuMetricDef {
            name: "gpu.fan_speed",
            dcgm_field: "NVML_FAN",
            metric_type: "gauge",
            unit: "percent",
            description: "Fan speed percent",
            group: "thermal",
        },
        GpuMetricDef {
            name: "gpu.power.usage",
            dcgm_field: "DCGM_FI_DEV_POWER_USAGE",
            metric_type: "gauge",
            unit: "watt",
            description: "Instantaneous / 1s average board power",
            group: "power",
        },
        GpuMetricDef {
            name: "gpu.power.management_limit",
            dcgm_field: "DCGM_FI_DEV_POWER_MGMT_LIMIT",
            metric_type: "gauge",
            unit: "watt",
            description: "Configured power management limit",
            group: "power",
        },
        GpuMetricDef {
            name: "gpu.energy.consumption",
            dcgm_field: "DCGM_FI_DEV_TOTAL_ENERGY_CONSUMPTION",
            metric_type: "counter",
            unit: "millijoule",
            description: "Total energy since boot",
            group: "power",
        },
        GpuMetricDef {
            name: "gpu.clock.sm",
            dcgm_field: "DCGM_FI_DEV_SM_CLOCK",
            metric_type: "gauge",
            unit: "megahertz",
            description: "SM clock",
            group: "clock",
        },
        GpuMetricDef {
            name: "gpu.clock.memory",
            dcgm_field: "DCGM_FI_DEV_MEM_CLOCK",
            metric_type: "gauge",
            unit: "megahertz",
            description: "Memory clock",
            group: "clock",
        },
        GpuMetricDef {
            name: "gpu.clock.graphics",
            dcgm_field: "NVML_GRAPHICS_CLOCK",
            metric_type: "gauge",
            unit: "megahertz",
            description: "Graphics engine clock",
            group: "clock",
        },
        GpuMetricDef {
            name: "gpu.pstate",
            dcgm_field: "DCGM_FI_DEV_PSTATE",
            metric_type: "gauge",
            unit: "level",
            description: "Performance state (P0–Pn)",
            group: "clock",
        },
        GpuMetricDef {
            name: "gpu.pipe.fp16_active",
            dcgm_field: "DCGM_FI_PROF_PIPE_FP16_ACTIVE",
            metric_type: "gauge",
            unit: "percent",
            description: "FP16 pipe activity (Hopper+)",
            group: "pipeline",
        },
        GpuMetricDef {
            name: "gpu.pipe.fp32_active",
            dcgm_field: "DCGM_FI_PROF_PIPE_FP32_ACTIVE",
            metric_type: "gauge",
            unit: "percent",
            description: "FP32 pipe activity",
            group: "pipeline",
        },
        GpuMetricDef {
            name: "gpu.pipe.fp64_active",
            dcgm_field: "DCGM_FI_PROF_PIPE_FP64_ACTIVE",
            metric_type: "gauge",
            unit: "percent",
            description: "FP64 pipe activity",
            group: "pipeline",
        },
        GpuMetricDef {
            name: "gpu.pipe.tensor_active",
            dcgm_field: "DCGM_FI_PROF_PIPE_TENSOR_ACTIVE",
            metric_type: "gauge",
            unit: "percent",
            description: "Tensor core activity",
            group: "pipeline",
        },
        GpuMetricDef {
            name: "gpu.pipe.integer_active",
            dcgm_field: "DCGM_FI_PROF_PIPE_TENSOR_ACTIVE",
            metric_type: "gauge",
            unit: "percent",
            description: "Integer pipe activity",
            group: "pipeline",
        },
        GpuMetricDef {
            name: "gpu.pcie.replay",
            dcgm_field: "DCGM_FI_DEV_PCIE_REPLAY_COUNTER",
            metric_type: "counter",
            unit: "error",
            description: "PCIe replay count",
            group: "interconnect",
        },
        GpuMetricDef {
            name: "gpu.nvlink.bandwidth",
            dcgm_field: "DCGM_FI_DEV_NVLINK_BANDWIDTH_TOTAL",
            metric_type: "gauge",
            unit: "byte",
            description: "NVLink bandwidth total",
            group: "interconnect",
        },
        GpuMetricDef {
            name: "gpu.nvlink.replay_errors",
            dcgm_field: "DCGM_FI_DEV_NVLINK_REPLAY_ERROR_COUNT",
            metric_type: "counter",
            unit: "error",
            description: "NVLink replay errors",
            group: "interconnect",
        },
        GpuMetricDef {
            name: "gpu.errors.xid.total",
            dcgm_field: "DCGM_FI_DEV_XID_ERRORS",
            metric_type: "counter",
            unit: "error",
            description: "Xid error count",
            group: "health",
        },
        GpuMetricDef {
            name: "gpu.remapped_rows.pending",
            dcgm_field: "DCGM_FI_DEV_ROW_REMAP_PENDING",
            metric_type: "gauge",
            unit: "row",
            description: "Pending remapped rows",
            group: "health",
        },
        GpuMetricDef {
            name: "gpu.remapped_rows.failed",
            dcgm_field: "DCGM_FI_DEV_ROW_REMAP_FAILURE",
            metric_type: "gauge",
            unit: "row",
            description: "Failed remapped rows",
            group: "health",
        },
        GpuMetricDef {
            name: "gpu.remapped_rows.correctable",
            dcgm_field: "DCGM_FI_DEV_ROW_REMAP_CORRECTABLE",
            metric_type: "gauge",
            unit: "row",
            description: "Correctable remapped rows",
            group: "health",
        },
        GpuMetricDef {
            name: "gpu.remapped_rows.uncorrectable",
            dcgm_field: "DCGM_FI_DEV_ROW_REMAP_UNCORRECTABLE",
            metric_type: "gauge",
            unit: "row",
            description: "Uncorrectable remapped rows",
            group: "health",
        },
        GpuMetricDef {
            name: "gpu.ecc.sbe",
            dcgm_field: "DCGM_FI_DEV_ECC_SBE_VOL_TOTAL",
            metric_type: "counter",
            unit: "error",
            description: "Single-bit ECC errors (volatile)",
            group: "health",
        },
        GpuMetricDef {
            name: "gpu.ecc.dbe",
            dcgm_field: "DCGM_FI_DEV_ECC_DBE_VOL_TOTAL",
            metric_type: "counter",
            unit: "error",
            description: "Double-bit ECC errors (volatile)",
            group: "health",
        },
        GpuMetricDef {
            name: "gpu.clock.throttle_reasons.gpu_idle",
            dcgm_field: "DCGM_FI_DEV_CLOCK_THROTTLE_REASONS",
            metric_type: "gauge",
            unit: "bool",
            description: "Throttled / clocks reduced because GPU idle",
            group: "health",
        },
        GpuMetricDef {
            name: "gpu.clock.throttle_reasons.applications_clocks",
            dcgm_field: "DCGM_FI_DEV_CLOCK_THROTTLE_REASONS",
            metric_type: "gauge",
            unit: "bool",
            description: "Throttled due to application clocks setting",
            group: "health",
        },
        GpuMetricDef {
            name: "gpu.clock.throttle_reasons.sw_power_cap",
            dcgm_field: "DCGM_FI_DEV_CLOCK_THROTTLE_REASONS",
            metric_type: "gauge",
            unit: "bool",
            description: "Throttled due to software power cap",
            group: "health",
        },
        GpuMetricDef {
            name: "gpu.clock.throttle_reasons.hw_slowdown",
            dcgm_field: "DCGM_FI_DEV_CLOCK_THROTTLE_REASONS",
            metric_type: "gauge",
            unit: "bool",
            description: "Throttled due to HW slowdown",
            group: "health",
        },
        GpuMetricDef {
            name: "gpu.clock.throttle_reasons.sync_boost",
            dcgm_field: "DCGM_FI_DEV_CLOCK_THROTTLE_REASONS",
            metric_type: "gauge",
            unit: "bool",
            description: "Throttled due to sync boost",
            group: "health",
        },
        GpuMetricDef {
            name: "gpu.clock.throttle_reasons.sw_thermal_slowdown",
            dcgm_field: "DCGM_FI_DEV_CLOCK_THROTTLE_REASONS",
            metric_type: "gauge",
            unit: "bool",
            description: "Throttled due to SW thermal slowdown",
            group: "health",
        },
        GpuMetricDef {
            name: "gpu.clock.throttle_reasons.hw_thermal_slowdown",
            dcgm_field: "DCGM_FI_DEV_CLOCK_THROTTLE_REASONS",
            metric_type: "gauge",
            unit: "bool",
            description: "Throttled due to hardware thermal slowdown",
            group: "health",
        },
        GpuMetricDef {
            name: "gpu.clock.throttle_reasons.hw_power_brake",
            dcgm_field: "DCGM_FI_DEV_CLOCK_THROTTLE_REASONS",
            metric_type: "gauge",
            unit: "bool",
            description: "Throttled due to HW power brake",
            group: "health",
        },
        GpuMetricDef {
            name: "gpu.process.sm_active",
            dcgm_field: "NVML_PROCESS_UTIL",
            metric_type: "gauge",
            unit: "percent",
            description: "Per-process SM activity",
            group: "process",
        },
        GpuMetricDef {
            name: "gpu.process.memory.usage",
            dcgm_field: "NVML_PROCESS_MEMORY",
            metric_type: "gauge",
            unit: "mebibyte",
            description: "Per-process GPU memory usage",
            group: "process",
        },
        GpuMetricDef {
            name: "gpu.process.utilization",
            dcgm_field: "NVML_PROCESS_UTIL",
            metric_type: "gauge",
            unit: "percent",
            description: "Per-process GPU utilization",
            group: "process",
        },
    ]
}

fn device_tags(g: &GpuDevice) -> Tags {
    let mut tags = Tags::new();
    tags.insert("gpu_id".into(), g.id.clone());
    tags.insert("host".into(), g.host.clone());
    tags.insert("model".into(), g.model.clone());
    if !g.uuid.is_empty() {
        tags.insert("uuid".into(), g.uuid.clone());
    }
    if !g.pci_bus_id.is_empty() {
        tags.insert("pci_bus_id".into(), g.pci_bus_id.clone());
    }
    for (k, v) in &g.tags {
        tags.insert(k.clone(), v.clone());
    }
    tags
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

fn counter(name: &str, tags: &Tags, ts: i64, value: f64, unit: &str) -> MetricPoint {
    MetricPoint {
        name: name.into(),
        metric_type: MetricType::Counter,
        tags: tags.clone(),
        sample: Sample {
            timestamp_ms: ts,
            value,
        },
        unit: Some(unit.into()),
        description: None,
    }
}

impl PlatformState {
    pub fn upsert_gpu(&self, mut g: GpuDevice) -> GpuDevice {
        // Keep legacy fields in sync
        if g.gpu_util == 0.0 && g.util > 0.0 {
            g.gpu_util = if g.util <= 1.0 { g.util * 100.0 } else { g.util };
        }
        if g.util == 0.0 && g.gpu_util > 0.0 {
            g.util = g.gpu_util;
        }
        if g.sm_active == 0.0 {
            g.sm_active = g.gpu_util;
        }
        if g.memory_total_mb > 0.0 {
            g.memory_free_mb = (g.memory_total_mb - g.memory_used_mb).max(0.0);
            g.memory_used_percent = (g.memory_used_mb / g.memory_total_mb) * 100.0;
        }
        if g.last_seen_ms == 0 {
            g.last_seen_ms = Utc::now().timestamp_millis();
        }
        if g.health_status.is_empty() {
            g.health_status = if g.xid_errors_total > 0 || g.remapped_rows_failed > 0 {
                "degraded".into()
            } else if g.throttle_reasons.hw_thermal_slowdown || g.throttle_reasons.hw_slowdown {
                "throttled".into()
            } else {
                "healthy".into()
            };
        }
        self.gpus.insert(g.id.clone(), g.clone());
        g
    }

    pub fn upsert_gpu_process(&self, p: GpuProcess) -> GpuProcess {
        let key = format!("{}:{}:{}", p.host, p.gpu_id, p.pid);
        self.gpu_processes.insert(key, p.clone());
        p
    }

    pub fn list_gpu_processes(&self, gpu_id: Option<&str>) -> Vec<GpuProcess> {
        let mut v: Vec<_> = self
            .gpu_processes
            .iter()
            .map(|e| e.value().clone())
            .filter(|p| gpu_id.map(|id| p.gpu_id == id).unwrap_or(true))
            .collect();
        v.sort_by(|a, b| {
            b.memory_usage_mb
                .partial_cmp(&a.memory_usage_mb)
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        v
    }

    pub fn record_gpu_sample(&self, sample: GpuSample) {
        let mut q = self.gpu_samples.write();
        q.push_back(sample);
        while q.len() > MAX_GPU_SAMPLES {
            q.pop_front();
        }
    }

    pub fn list_gpu_samples(&self, gpu_id: Option<&str>, limit: usize) -> Vec<GpuSample> {
        let q = self.gpu_samples.read();
        let mut v: Vec<_> = q
            .iter()
            .cloned()
            .filter(|s| gpu_id.map(|id| s.gpu_id == id).unwrap_or(true))
            .collect();
        v.sort_by_key(|s| std::cmp::Reverse(s.timestamp_ms));
        v.truncate(limit);
        v
    }

    /// Emit all granular gauges/counters into the metric store for Explorer queries.
    pub fn emit_gpu_metrics(&self, ts: Option<i64>) -> usize {
        let ts = ts.unwrap_or_else(|| Utc::now().timestamp_millis());
        let mut points = Vec::new();
        for g in self.list_gpus() {
            let tags = device_tags(&g);
            let util = if g.gpu_util > 0.0 {
                g.gpu_util
            } else if g.util <= 1.0 {
                g.util * 100.0
            } else {
                g.util
            };
            points.push(gauge("gpu.utilization", &tags, ts, util, "percent"));
            points.push(gauge("gpu.sm_active", &tags, ts, g.sm_active, "percent"));
            points.push(gauge(
                "gpu.sm_occupancy",
                &tags,
                ts,
                g.sm_occupancy,
                "percent",
            ));
            points.push(gauge(
                "gpu.gr_engine_active",
                &tags,
                ts,
                g.gr_engine_active,
                "percent",
            ));
            points.push(gauge(
                "gpu.memory.copy_utilization",
                &tags,
                ts,
                g.mem_copy_util,
                "percent",
            ));
            points.push(gauge(
                "gpu.memory.used",
                &tags,
                ts,
                g.memory_used_mb,
                "mebibyte",
            ));
            points.push(gauge(
                "gpu.memory.free",
                &tags,
                ts,
                g.memory_free_mb,
                "mebibyte",
            ));
            points.push(gauge(
                "gpu.memory.total",
                &tags,
                ts,
                g.memory_total_mb,
                "mebibyte",
            ));
            points.push(gauge(
                "gpu.memory.reserved",
                &tags,
                ts,
                g.memory_reserved_mb,
                "mebibyte",
            ));
            points.push(gauge(
                "gpu.memory.used_percent",
                &tags,
                ts,
                g.memory_used_percent,
                "percent",
            ));
            points.push(gauge("gpu.dram_active", &tags, ts, g.dram_active, "percent"));
            points.push(gauge(
                "gpu.enc_utilization",
                &tags,
                ts,
                g.enc_utilization,
                "percent",
            ));
            points.push(gauge(
                "gpu.dec_utilization",
                &tags,
                ts,
                g.dec_utilization,
                "percent",
            ));
            points.push(gauge(
                "gpu.temperature",
                &tags,
                ts,
                g.temperature_c,
                "celsius",
            ));
            points.push(gauge(
                "gpu.memory.temperature",
                &tags,
                ts,
                g.memory_temperature_c,
                "celsius",
            ));
            points.push(gauge(
                "gpu.fan_speed",
                &tags,
                ts,
                g.fan_speed_pct,
                "percent",
            ));
            points.push(gauge(
                "gpu.power.usage",
                &tags,
                ts,
                g.power_usage_w,
                "watt",
            ));
            points.push(gauge(
                "gpu.power.management_limit",
                &tags,
                ts,
                g.power_management_limit_w,
                "watt",
            ));
            points.push(counter(
                "gpu.energy.consumption",
                &tags,
                ts,
                g.total_energy_consumption_mj,
                "millijoule",
            ));
            points.push(gauge("gpu.clock.sm", &tags, ts, g.sm_clock_mhz, "megahertz"));
            points.push(gauge(
                "gpu.clock.memory",
                &tags,
                ts,
                g.mem_clock_mhz,
                "megahertz",
            ));
            points.push(gauge(
                "gpu.clock.graphics",
                &tags,
                ts,
                g.graphics_clock_mhz,
                "megahertz",
            ));
            points.push(gauge("gpu.pstate", &tags, ts, g.pstate as f64, "level"));
            points.push(gauge(
                "gpu.pipe.fp16_active",
                &tags,
                ts,
                g.pipe_fp16_active,
                "percent",
            ));
            points.push(gauge(
                "gpu.pipe.fp32_active",
                &tags,
                ts,
                g.pipe_fp32_active,
                "percent",
            ));
            points.push(gauge(
                "gpu.pipe.fp64_active",
                &tags,
                ts,
                g.pipe_fp64_active,
                "percent",
            ));
            points.push(gauge(
                "gpu.pipe.tensor_active",
                &tags,
                ts,
                g.pipe_tensor_active,
                "percent",
            ));
            points.push(gauge(
                "gpu.pipe.integer_active",
                &tags,
                ts,
                g.pipe_integer_active,
                "percent",
            ));
            points.push(counter(
                "gpu.pcie.replay",
                &tags,
                ts,
                g.pcie_replay_total as f64,
                "error",
            ));
            points.push(gauge(
                "gpu.nvlink.bandwidth",
                &tags,
                ts,
                g.nvlink_bandwidth_total,
                "byte",
            ));
            points.push(counter(
                "gpu.nvlink.replay_errors",
                &tags,
                ts,
                g.nvlink_replay_errors as f64,
                "error",
            ));
            points.push(counter(
                "gpu.errors.xid.total",
                &tags,
                ts,
                g.xid_errors_total as f64,
                "error",
            ));
            points.push(gauge(
                "gpu.remapped_rows.pending",
                &tags,
                ts,
                g.remapped_rows_pending as f64,
                "row",
            ));
            points.push(gauge(
                "gpu.remapped_rows.failed",
                &tags,
                ts,
                g.remapped_rows_failed as f64,
                "row",
            ));
            points.push(gauge(
                "gpu.remapped_rows.correctable",
                &tags,
                ts,
                g.remapped_rows_correctable as f64,
                "row",
            ));
            points.push(gauge(
                "gpu.remapped_rows.uncorrectable",
                &tags,
                ts,
                g.remapped_rows_uncorrectable as f64,
                "row",
            ));
            points.push(counter(
                "gpu.ecc.sbe",
                &tags,
                ts,
                g.ecc_sbe_volatile_total as f64,
                "error",
            ));
            points.push(counter(
                "gpu.ecc.dbe",
                &tags,
                ts,
                g.ecc_dbe_volatile_total as f64,
                "error",
            ));
            let tr = &g.throttle_reasons;
            for (name, on) in [
                ("gpu.clock.throttle_reasons.gpu_idle", tr.gpu_idle),
                (
                    "gpu.clock.throttle_reasons.applications_clocks",
                    tr.applications_clocks,
                ),
                ("gpu.clock.throttle_reasons.sw_power_cap", tr.sw_power_cap),
                ("gpu.clock.throttle_reasons.hw_slowdown", tr.hw_slowdown),
                ("gpu.clock.throttle_reasons.sync_boost", tr.sync_boost),
                (
                    "gpu.clock.throttle_reasons.sw_thermal_slowdown",
                    tr.sw_thermal_slowdown,
                ),
                (
                    "gpu.clock.throttle_reasons.hw_thermal_slowdown",
                    tr.hw_thermal_slowdown,
                ),
                ("gpu.clock.throttle_reasons.hw_power_brake", tr.hw_power_brake),
            ] {
                points.push(gauge(name, &tags, ts, if on { 1.0 } else { 0.0 }, "bool"));
            }

            self.record_gpu_sample(GpuSample {
                timestamp_ms: ts,
                gpu_id: g.id.clone(),
                sm_active: g.sm_active,
                sm_occupancy: g.sm_occupancy,
                gpu_util: util,
                memory_used_percent: g.memory_used_percent,
                power_usage_w: g.power_usage_w,
                temperature_c: g.temperature_c,
                pipe_tensor_active: g.pipe_tensor_active,
                gr_engine_active: g.gr_engine_active,
            });
        }

        for p in self.list_gpu_processes(None) {
            let mut tags = Tags::new();
            tags.insert("gpu_id".into(), p.gpu_id.clone());
            tags.insert("host".into(), p.host.clone());
            tags.insert("pid".into(), p.pid.to_string());
            tags.insert("process".into(), p.process_name.clone());
            if let Some(pod) = &p.pod {
                tags.insert("pod".into(), pod.clone());
            }
            if let Some(ns) = &p.namespace {
                tags.insert("kube_namespace".into(), ns.clone());
            }
            if let Some(team) = &p.team {
                tags.insert("team".into(), team.clone());
            }
            if let Some(wl) = &p.workload {
                tags.insert("workload".into(), wl.clone());
            }
            points.push(gauge(
                "gpu.process.sm_active",
                &tags,
                ts,
                p.sm_active,
                "percent",
            ));
            points.push(gauge(
                "gpu.process.memory.usage",
                &tags,
                ts,
                p.memory_usage_mb,
                "mebibyte",
            ));
            points.push(gauge(
                "gpu.process.utilization",
                &tags,
                ts,
                p.gpu_util,
                "percent",
            ));
        }

        let n = points.len();
        let _ = self.metrics.ingest_batch(points);
        n
    }

    pub fn gpu_summary(&self) -> Value {
        let gpus = self.list_gpus();
        let procs = self.list_gpu_processes(None);
        let avg = |f: fn(&GpuDevice) -> f64| {
            if gpus.is_empty() {
                0.0
            } else {
                gpus.iter().map(f).sum::<f64>() / gpus.len() as f64
            }
        };
        let throttled = gpus
            .iter()
            .filter(|g| {
                g.throttle_reasons.hw_thermal_slowdown
                    || g.throttle_reasons.sw_power_cap
                    || g.throttle_reasons.hw_slowdown
            })
            .count();
        let unhealthy = gpus
            .iter()
            .filter(|g| g.health_status != "healthy")
            .count();
        json!({
            "devices": gpus.len(),
            "processes": procs.len(),
            "avg_util": avg(|g| if g.gpu_util > 0.0 { g.gpu_util } else { g.util }),
            "avg_sm_active": avg(|g| g.sm_active),
            "avg_sm_occupancy": avg(|g| g.sm_occupancy),
            "avg_memory_used_percent": avg(|g| g.memory_used_percent),
            "total_memory_mb": gpus.iter().map(|g| g.memory_used_mb).sum::<f64>(),
            "total_memory_capacity_mb": gpus.iter().map(|g| g.memory_total_mb).sum::<f64>(),
            "total_power_w": gpus.iter().map(|g| g.power_usage_w).sum::<f64>(),
            "avg_temperature_c": avg(|g| g.temperature_c),
            "throttled_devices": throttled,
            "unhealthy_devices": unhealthy,
            "xid_errors_total": gpus.iter().map(|g| g.xid_errors_total).sum::<u64>(),
            "metrics_catalog_count": gpu_metric_catalog().len(),
            "devices_detail": gpus,
        })
    }

    pub fn gpu_fleet(&self) -> Value {
        let gpus = self.list_gpus();
        let mut by_model: BTreeMap<String, usize> = BTreeMap::new();
        let mut by_host: BTreeMap<String, usize> = BTreeMap::new();
        let mut by_health: BTreeMap<String, usize> = BTreeMap::new();
        for g in &gpus {
            *by_model.entry(g.model.clone()).or_default() += 1;
            *by_host.entry(g.host.clone()).or_default() += 1;
            *by_health.entry(g.health_status.clone()).or_default() += 1;
        }
        json!({
            "total": gpus.len(),
            "by_model": by_model,
            "by_host": by_host,
            "by_health": by_health,
            "power_draw_w": gpus.iter().map(|g| g.power_usage_w).sum::<f64>(),
            "power_cap_w": gpus.iter().map(|g| g.power_management_limit_w).sum::<f64>(),
            "memory_used_pct_fleet": if gpus.is_empty() { 0.0 } else {
                gpus.iter().map(|g| g.memory_used_percent).sum::<f64>() / gpus.len() as f64
            },
            "sm_active_fleet": if gpus.is_empty() { 0.0 } else {
                gpus.iter().map(|g| g.sm_active).sum::<f64>() / gpus.len() as f64
            },
        })
    }

    pub fn gpu_health(&self) -> Value {
        let gpus = self.list_gpus();
        let findings: Vec<_> = gpus
            .iter()
            .flat_map(|g| {
                let mut out = Vec::new();
                if g.xid_errors_total > 0 {
                    out.push(json!({
                        "gpu_id": g.id, "severity": "high",
                        "finding": "xid_errors", "value": g.xid_errors_total
                    }));
                }
                if g.remapped_rows_failed > 0 {
                    out.push(json!({
                        "gpu_id": g.id, "severity": "high",
                        "finding": "remapped_rows_failed", "value": g.remapped_rows_failed
                    }));
                }
                if g.throttle_reasons.hw_thermal_slowdown {
                    out.push(json!({
                        "gpu_id": g.id, "severity": "medium",
                        "finding": "hw_thermal_slowdown", "temperature_c": g.temperature_c
                    }));
                }
                if g.throttle_reasons.sw_power_cap {
                    out.push(json!({
                        "gpu_id": g.id, "severity": "low",
                        "finding": "sw_power_cap",
                        "power_w": g.power_usage_w, "limit_w": g.power_management_limit_w
                    }));
                }
                if g.ecc_dbe_volatile_total > 0 {
                    out.push(json!({
                        "gpu_id": g.id, "severity": "high",
                        "finding": "ecc_dbe", "value": g.ecc_dbe_volatile_total
                    }));
                }
                if g.sm_active < 5.0 && g.memory_used_percent > 80.0 {
                    out.push(json!({
                        "gpu_id": g.id, "severity": "medium",
                        "finding": "zombie_allocation",
                        "sm_active": g.sm_active, "memory_used_percent": g.memory_used_percent
                    }));
                }
                out
            })
            .collect();
        json!({
            "devices_checked": gpus.len(),
            "findings": findings,
            "healthy": gpus.iter().filter(|g| g.health_status == "healthy").count(),
            "degraded": gpus.iter().filter(|g| g.health_status != "healthy").count(),
        })
    }

    pub fn gpu_metrics_catalog_json(&self) -> Value {
        json!({
            "count": gpu_metric_catalog().len(),
            "groups": ["utilization","memory","thermal","power","clock","pipeline","interconnect","health","process"],
            "metrics": gpu_metric_catalog(),
            "docs_ref": "https://docs.datadoghq.com/gpu_monitoring/",
            "dcgm_ref": "https://docs.nvidia.com/datacenter/dcgm/latest/reference/dcgm-exporter-metrics.html"
        })
    }

    pub fn seed_gpu_fleet(&self) {
        let now = Utc::now().timestamp_millis();
        let devices = [
            GpuDevice {
                id: "gpu-0".into(),
                uuid: "GPU-a100-0".into(),
                model: "NVIDIA A100-SXM4-80GB".into(),
                host: "gpu-node-1".into(),
                pci_bus_id: "0000:00:1e.0".into(),
                driver_version: "550.54.15".into(),
                cuda_version: "12.4".into(),
                util: 72.0,
                memory_used_mb: 62_000.0,
                gpu_util: 72.0,
                sm_active: 68.5,
                sm_occupancy: 54.2,
                gr_engine_active: 71.0,
                mem_copy_util: 22.0,
                enc_utilization: 0.0,
                dec_utilization: 0.0,
                memory_total_mb: 81_920.0,
                memory_free_mb: 19_920.0,
                memory_reserved_mb: 512.0,
                memory_used_percent: 75.7,
                dram_active: 41.0,
                sm_clock_mhz: 1410.0,
                mem_clock_mhz: 1593.0,
                graphics_clock_mhz: 1410.0,
                pstate: 0,
                power_usage_w: 312.0,
                power_management_limit_w: 400.0,
                total_energy_consumption_mj: 48_200_000.0,
                temperature_c: 64.0,
                memory_temperature_c: 58.0,
                fan_speed_pct: 55.0,
                pipe_fp16_active: 18.0,
                pipe_fp32_active: 12.0,
                pipe_fp64_active: 2.0,
                pipe_tensor_active: 61.0,
                pipe_integer_active: 8.0,
                pcie_replay_total: 0,
                nvlink_bandwidth_total: 420_000_000.0,
                nvlink_replay_errors: 0,
                xid_errors_total: 0,
                remapped_rows_pending: 0,
                remapped_rows_failed: 0,
                remapped_rows_correctable: 1,
                remapped_rows_uncorrectable: 0,
                throttle_reasons: GpuThrottleReasons::default(),
                ecc_sbe_volatile_total: 2,
                ecc_dbe_volatile_total: 0,
                health_status: "healthy".into(),
                tags: BTreeMap::from([
                    ("env".into(), "prod".into()),
                    ("cluster".into(), "ml-train".into()),
                    ("team".into(), "foundation".into()),
                ]),
                last_seen_ms: now,
            },
            GpuDevice {
                id: "gpu-1".into(),
                uuid: "GPU-h100-1".into(),
                model: "NVIDIA H100-80GB-HBM3".into(),
                host: "gpu-node-1".into(),
                pci_bus_id: "0000:00:1f.0".into(),
                driver_version: "550.54.15".into(),
                cuda_version: "12.4".into(),
                util: 91.0,
                memory_used_mb: 74_500.0,
                gpu_util: 91.0,
                sm_active: 88.0,
                sm_occupancy: 72.5,
                gr_engine_active: 90.0,
                mem_copy_util: 35.0,
                enc_utilization: 0.0,
                dec_utilization: 0.0,
                memory_total_mb: 81_920.0,
                memory_free_mb: 7_420.0,
                memory_reserved_mb: 768.0,
                memory_used_percent: 90.9,
                dram_active: 58.0,
                sm_clock_mhz: 1785.0,
                mem_clock_mhz: 2619.0,
                graphics_clock_mhz: 1785.0,
                pstate: 0,
                power_usage_w: 668.0,
                power_management_limit_w: 700.0,
                total_energy_consumption_mj: 91_000_000.0,
                temperature_c: 78.0,
                memory_temperature_c: 71.0,
                fan_speed_pct: 72.0,
                pipe_fp16_active: 22.0,
                pipe_fp32_active: 9.0,
                pipe_fp64_active: 1.0,
                pipe_tensor_active: 79.0,
                pipe_integer_active: 11.0,
                pcie_replay_total: 1,
                nvlink_bandwidth_total: 890_000_000.0,
                nvlink_replay_errors: 0,
                xid_errors_total: 0,
                remapped_rows_pending: 0,
                remapped_rows_failed: 0,
                remapped_rows_correctable: 0,
                remapped_rows_uncorrectable: 0,
                throttle_reasons: GpuThrottleReasons {
                    sw_power_cap: true,
                    ..Default::default()
                },
                ecc_sbe_volatile_total: 0,
                ecc_dbe_volatile_total: 0,
                health_status: "throttled".into(),
                tags: BTreeMap::from([
                    ("env".into(), "prod".into()),
                    ("cluster".into(), "ml-train".into()),
                    ("team".into(), "foundation".into()),
                ]),
                last_seen_ms: now,
            },
            GpuDevice {
                id: "gpu-2".into(),
                uuid: "GPU-a10g-2".into(),
                model: "NVIDIA A10G".into(),
                host: "gpu-node-2".into(),
                pci_bus_id: "0000:00:1d.0".into(),
                driver_version: "535.161.08".into(),
                cuda_version: "12.2".into(),
                util: 12.0,
                memory_used_mb: 20_480.0,
                gpu_util: 12.0,
                sm_active: 3.2,
                sm_occupancy: 2.1,
                gr_engine_active: 8.0,
                mem_copy_util: 4.0,
                enc_utilization: 15.0,
                dec_utilization: 5.0,
                memory_total_mb: 24_576.0,
                memory_free_mb: 4_096.0,
                memory_reserved_mb: 256.0,
                memory_used_percent: 83.3,
                dram_active: 9.0,
                sm_clock_mhz: 1695.0,
                mem_clock_mhz: 6251.0,
                graphics_clock_mhz: 1695.0,
                pstate: 2,
                power_usage_w: 88.0,
                power_management_limit_w: 300.0,
                total_energy_consumption_mj: 12_400_000.0,
                temperature_c: 48.0,
                memory_temperature_c: 44.0,
                fan_speed_pct: 35.0,
                pipe_fp16_active: 4.0,
                pipe_fp32_active: 6.0,
                pipe_fp64_active: 0.0,
                pipe_tensor_active: 2.0,
                pipe_integer_active: 3.0,
                pcie_replay_total: 0,
                nvlink_bandwidth_total: 0.0,
                nvlink_replay_errors: 0,
                xid_errors_total: 0,
                remapped_rows_pending: 0,
                remapped_rows_failed: 0,
                remapped_rows_correctable: 0,
                remapped_rows_uncorrectable: 0,
                throttle_reasons: GpuThrottleReasons {
                    gpu_idle: true,
                    ..Default::default()
                },
                ecc_sbe_volatile_total: 0,
                ecc_dbe_volatile_total: 0,
                health_status: "healthy".into(),
                tags: BTreeMap::from([
                    ("env".into(), "prod".into()),
                    ("cluster".into(), "inference".into()),
                    ("team".into(), "ml-platform".into()),
                ]),
                last_seen_ms: now,
            },
            GpuDevice {
                id: "gpu-3".into(),
                uuid: "GPU-l4-3".into(),
                model: "NVIDIA L4".into(),
                host: "gpu-node-2".into(),
                pci_bus_id: "0000:00:1c.0".into(),
                driver_version: "535.161.08".into(),
                cuda_version: "12.2".into(),
                util: 44.0,
                memory_used_mb: 14_200.0,
                gpu_util: 44.0,
                sm_active: 41.0,
                sm_occupancy: 33.0,
                gr_engine_active: 42.0,
                mem_copy_util: 18.0,
                enc_utilization: 40.0,
                dec_utilization: 22.0,
                memory_total_mb: 24_576.0,
                memory_free_mb: 10_376.0,
                memory_reserved_mb: 128.0,
                memory_used_percent: 57.8,
                dram_active: 28.0,
                sm_clock_mhz: 2040.0,
                mem_clock_mhz: 6251.0,
                graphics_clock_mhz: 2040.0,
                pstate: 0,
                power_usage_w: 55.0,
                power_management_limit_w: 72.0,
                total_energy_consumption_mj: 6_800_000.0,
                temperature_c: 86.0,
                memory_temperature_c: 79.0,
                fan_speed_pct: 90.0,
                pipe_fp16_active: 14.0,
                pipe_fp32_active: 20.0,
                pipe_fp64_active: 0.0,
                pipe_tensor_active: 28.0,
                pipe_integer_active: 10.0,
                pcie_replay_total: 4,
                nvlink_bandwidth_total: 0.0,
                nvlink_replay_errors: 0,
                xid_errors_total: 2,
                remapped_rows_pending: 1,
                remapped_rows_failed: 0,
                remapped_rows_correctable: 2,
                remapped_rows_uncorrectable: 0,
                throttle_reasons: GpuThrottleReasons {
                    hw_thermal_slowdown: true,
                    sw_thermal_slowdown: true,
                    ..Default::default()
                },
                ecc_sbe_volatile_total: 5,
                ecc_dbe_volatile_total: 1,
                health_status: "degraded".into(),
                tags: BTreeMap::from([
                    ("env".into(), "prod".into()),
                    ("cluster".into(), "inference".into()),
                    ("team".into(), "ml-platform".into()),
                ]),
                last_seen_ms: now,
            },
        ];

        for d in devices {
            self.upsert_gpu(d);
        }

        for p in [
            GpuProcess {
                pid: 44102,
                gpu_id: "gpu-0".into(),
                host: "gpu-node-1".into(),
                process_name: "torchrun".into(),
                cmdline: "python -m torch.distributed.run train.py --model llama-70b".into(),
                container_id: Some("ctr-train-1".into()),
                pod: Some("train-llama-0".into()),
                namespace: Some("ml-train".into()),
                sm_active: 65.0,
                memory_usage_mb: 58_000.0,
                gpu_util: 70.0,
                team: Some("foundation".into()),
                workload: Some("pretrain".into()),
            },
            GpuProcess {
                pid: 44188,
                gpu_id: "gpu-1".into(),
                host: "gpu-node-1".into(),
                process_name: "torchrun".into(),
                cmdline: "python -m torch.distributed.run train.py --model llama-70b".into(),
                container_id: Some("ctr-train-1".into()),
                pod: Some("train-llama-0".into()),
                namespace: Some("ml-train".into()),
                sm_active: 86.0,
                memory_usage_mb: 72_000.0,
                gpu_util: 90.0,
                team: Some("foundation".into()),
                workload: Some("pretrain".into()),
            },
            GpuProcess {
                pid: 22011,
                gpu_id: "gpu-2".into(),
                host: "gpu-node-2".into(),
                process_name: "python".into(),
                cmdline: "uvicorn serve:app --port 8080".into(),
                container_id: Some("ctr-inf-2".into()),
                pod: Some("infer-embeddings-7d9f".into()),
                namespace: Some("inference".into()),
                sm_active: 2.5,
                memory_usage_mb: 19_200.0,
                gpu_util: 8.0,
                team: Some("ml-platform".into()),
                workload: Some("embeddings".into()),
            },
            GpuProcess {
                pid: 33044,
                gpu_id: "gpu-3".into(),
                host: "gpu-node-2".into(),
                process_name: "ffmpeg".into(),
                cmdline: "ffmpeg -hwaccel cuda -i in.mp4 out.mp4".into(),
                container_id: Some("ctr-media-3".into()),
                pod: Some("transcode-batch-2".into()),
                namespace: Some("media".into()),
                sm_active: 38.0,
                memory_usage_mb: 12_800.0,
                gpu_util: 42.0,
                team: Some("media".into()),
                workload: Some("transcode".into()),
            },
        ] {
            self.upsert_gpu_process(p);
        }

        // Historical samples for sparklines / effectiveness
        for i in 0..30 {
            let ts = now - (30 - i) as i64 * 60_000;
            for (gid, base_sm, base_mem, base_pwr, base_temp) in [
                ("gpu-0", 65.0, 74.0, 300.0, 62.0),
                ("gpu-1", 85.0, 88.0, 650.0, 76.0),
                ("gpu-2", 4.0, 82.0, 90.0, 47.0),
                ("gpu-3", 40.0, 55.0, 52.0, 82.0),
            ] {
                let wobble = ((i % 7) as f64 - 3.0) * 1.5;
                self.record_gpu_sample(GpuSample {
                    timestamp_ms: ts,
                    gpu_id: gid.into(),
                    sm_active: (base_sm + wobble).clamp(0.0, 100.0),
                    sm_occupancy: (base_sm * 0.75 + wobble).clamp(0.0, 100.0),
                    gpu_util: (base_sm + 2.0 + wobble).clamp(0.0, 100.0),
                    memory_used_percent: (base_mem + wobble * 0.3).clamp(0.0, 100.0),
                    power_usage_w: base_pwr + wobble * 2.0,
                    temperature_c: base_temp + wobble * 0.4,
                    pipe_tensor_active: (base_sm * 0.9).clamp(0.0, 100.0),
                    gr_engine_active: (base_sm + 1.0).clamp(0.0, 100.0),
                });
            }
        }

        // Emit current snapshot into metric store (Explorer can query gpu.*)
        for back in 0..12 {
            let ts = now - back as i64 * 300_000;
            // temporarily bump clocks via re-emit with historical feel
            let _ = self.emit_gpu_metrics(Some(ts));
        }
    }
}
