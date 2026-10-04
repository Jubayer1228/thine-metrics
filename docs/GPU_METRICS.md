# GPU Monitoring — granular metrics

Thine collects Datadog GPU Monitoring / NVIDIA DCGM–aligned telemetry under the `gpu.*` namespace (**49** catalog metrics).

Live: `GET /api/v1/gpu/metrics` · Dashboard: **Observability → GPU** (studio charts + live values for every metric)

## Groups

| Group | Metrics |
|-------|---------|
| **Utilization** | `gpu.utilization`, `gpu.sm_active`, `gpu.sm_occupancy`, `gpu.gr_engine_active`, `gpu.enc_utilization`, `gpu.dec_utilization` |
| **Memory** | `gpu.memory.used/free/total/reserved/used_percent`, `gpu.memory.copy_utilization`, `gpu.dram_active` |
| **Thermal** | `gpu.temperature`, `gpu.memory.temperature`, `gpu.fan_speed` |
| **Power** | `gpu.power.usage`, `gpu.power.management_limit`, `gpu.energy.consumption` |
| **Clock** | `gpu.clock.sm/memory/graphics`, `gpu.pstate`, `gpu.clock.throttle_reasons.*` |
| **Pipeline** | `gpu.pipe.fp16/fp32/fp64/tensor/integer_active` |
| **Interconnect** | `gpu.pcie.replay`, `gpu.nvlink.bandwidth`, `gpu.nvlink.replay_errors` |
| **Health** | `gpu.errors.xid.total`, `gpu.remapped_rows.*`, `gpu.ecc.sbe/dbe` |
| **Process** | `gpu.process.sm_active`, `gpu.process.memory.usage`, `gpu.process.utilization` |

## APIs

| Endpoint | Purpose |
|----------|---------|
| `GET /api/v1/gpu/devices` | Full device inventory + latest snapshot |
| `GET /api/v1/gpu/summary` | Fleet KPIs |
| `GET /api/v1/gpu/fleet` | By model / host / health |
| `GET /api/v1/gpu/health` | Xid, ECC, throttle, zombie findings |
| `GET /api/v1/gpu/processes` | Pod/process attribution |
| `GET /api/v1/gpu/samples` | Timeseries samples |
| `GET /api/v1/gpu/metrics` | Catalog (name ↔ DCGM field) |
| `POST /api/v1/gpu/emit` | Emit all gauges/counters into metric store |

Tags on series: `gpu_id`, `host`, `model`, `uuid`, `pci_bus_id`, plus process `pid` / `pod` / `workload`.
