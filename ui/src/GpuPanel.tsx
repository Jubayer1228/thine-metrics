import { useMemo, useState, type ReactNode } from "react";
import {
  Area,
  AreaChart,
  Bar,
  BarChart,
  CartesianGrid,
  ComposedChart,
  Legend,
  Line,
  LineChart,
  ResponsiveContainer,
  Tooltip,
  XAxis,
  YAxis,
} from "recharts";
import type { GpuDevice, GpuProcess, GpuSample, GpuSummary } from "./api";
import { fmt } from "./widgets";

const tip = {
  background: "#151c28",
  border: "1px solid rgba(255,255,255,0.12)",
  borderRadius: 8,
  color: "#E8EDF5",
  fontSize: 12,
  padding: "10px 12px",
};

const GPU_COLORS = ["#5B91EB", "#2EC4B6", "#A371E3", "#F4A261", "#F25F5C", "#4CC9F0"];

type CatalogRow = {
  name: string;
  dcgm_field: string;
  unit: string;
  description: string;
  group: string;
};

function Kpi({ label, value, hint }: { label: string; value: string; hint?: string }) {
  return (
    <div className="obs-kpi">
      <span>{label}</span>
      <strong>{value}</strong>
      {hint && <small>{hint}</small>}
    </div>
  );
}

function ChartCard({
  title,
  subtitle,
  children,
  tall,
}: {
  title: string;
  subtitle?: string;
  children: ReactNode;
  tall?: boolean;
}) {
  return (
    <div className="gpu-chart-card">
      <header>
        <h3>{title}</h3>
        {subtitle && <p>{subtitle}</p>}
      </header>
      <div className={tall ? "gpu-chart tall" : "gpu-chart"}>{children}</div>
    </div>
  );
}

/** Resolve live value for a catalog metric from device (+ process rollups). */
function metricValue(
  m: CatalogRow,
  d: GpuDevice | undefined,
  procs: GpuProcess[],
): number | null {
  if (!d) return null;
  const tr = d.throttle_reasons ?? {};
  const map: Record<string, number | undefined> = {
    "gpu.utilization": d.gpu_util,
    "gpu.sm_active": d.sm_active,
    "gpu.sm_occupancy": d.sm_occupancy,
    "gpu.gr_engine_active": d.gr_engine_active,
    "gpu.memory.copy_utilization": d.mem_copy_util,
    "gpu.memory.used": d.memory_used_mb,
    "gpu.memory.free": d.memory_free_mb,
    "gpu.memory.total": d.memory_total_mb,
    "gpu.memory.reserved": d.memory_reserved_mb,
    "gpu.memory.used_percent": d.memory_used_percent,
    "gpu.dram_active": d.dram_active,
    "gpu.enc_utilization": d.enc_utilization,
    "gpu.dec_utilization": d.dec_utilization,
    "gpu.temperature": d.temperature_c,
    "gpu.memory.temperature": d.memory_temperature_c,
    "gpu.fan_speed": d.fan_speed_pct,
    "gpu.power.usage": d.power_usage_w,
    "gpu.power.management_limit": d.power_management_limit_w,
    "gpu.energy.consumption": d.total_energy_consumption_mj,
    "gpu.clock.sm": d.sm_clock_mhz,
    "gpu.clock.memory": d.mem_clock_mhz,
    "gpu.clock.graphics": d.graphics_clock_mhz,
    "gpu.pstate": d.pstate,
    "gpu.pipe.fp16_active": d.pipe_fp16_active,
    "gpu.pipe.fp32_active": d.pipe_fp32_active,
    "gpu.pipe.fp64_active": d.pipe_fp64_active,
    "gpu.pipe.tensor_active": d.pipe_tensor_active,
    "gpu.pipe.integer_active": d.pipe_integer_active,
    "gpu.pcie.replay": d.pcie_replay_total,
    "gpu.nvlink.bandwidth": d.nvlink_bandwidth_total,
    "gpu.nvlink.replay_errors": d.nvlink_replay_errors,
    "gpu.errors.xid.total": d.xid_errors_total,
    "gpu.remapped_rows.pending": d.remapped_rows_pending,
    "gpu.remapped_rows.failed": d.remapped_rows_failed,
    "gpu.remapped_rows.correctable": d.remapped_rows_correctable,
    "gpu.remapped_rows.uncorrectable": d.remapped_rows_uncorrectable,
    "gpu.ecc.sbe": d.ecc_sbe_volatile_total,
    "gpu.ecc.dbe": d.ecc_dbe_volatile_total,
    "gpu.clock.throttle_reasons.gpu_idle": tr.gpu_idle ? 1 : 0,
    "gpu.clock.throttle_reasons.applications_clocks": tr.applications_clocks ? 1 : 0,
    "gpu.clock.throttle_reasons.sw_power_cap": tr.sw_power_cap ? 1 : 0,
    "gpu.clock.throttle_reasons.hw_slowdown": tr.hw_slowdown ? 1 : 0,
    "gpu.clock.throttle_reasons.sync_boost": tr.sync_boost ? 1 : 0,
    "gpu.clock.throttle_reasons.sw_thermal_slowdown": tr.sw_thermal_slowdown ? 1 : 0,
    "gpu.clock.throttle_reasons.hw_thermal_slowdown": tr.hw_thermal_slowdown ? 1 : 0,
    "gpu.clock.throttle_reasons.hw_power_brake": tr.hw_power_brake ? 1 : 0,
  };
  if (m.name.startsWith("gpu.process.")) {
    const mine = procs.filter((p) => p.gpu_id === d.id);
    if (m.name === "gpu.process.sm_active") return mine.reduce((a, p) => a + p.sm_active, 0);
    if (m.name === "gpu.process.memory.usage") return mine.reduce((a, p) => a + p.memory_usage_mb, 0);
    if (m.name === "gpu.process.utilization") return mine.reduce((a, p) => a + p.gpu_util, 0);
  }
  const v = map[m.name];
  return v === undefined ? null : v;
}

function formatMetric(unit: string, v: number | null) {
  if (v === null || Number.isNaN(v)) return "—";
  if (unit === "bool") return v >= 1 ? "true" : "false";
  if (unit === "percent") return `${fmt(v, 1)}%`;
  if (unit === "mebibyte") return `${fmt(v, 0)} MiB`;
  if (unit === "watt") return `${fmt(v, 1)} W`;
  if (unit === "celsius") return `${fmt(v, 0)}°C`;
  if (unit === "megahertz") return `${fmt(v, 0)} MHz`;
  if (unit === "millijoule") return `${fmt(v / 1e6, 1)} MJ`;
  if (unit === "byte") return v > 1e6 ? `${fmt(v / 1e6, 1)} MB` : fmt(v, 0);
  return fmt(v, 2);
}

export function GpuPanel({
  summary,
  procs,
  samples,
  health,
  catalog,
}: {
  summary: GpuSummary | null;
  procs: GpuProcess[];
  samples: GpuSample[];
  health: {
    devices_checked: number;
    healthy: number;
    degraded: number;
    findings: { gpu_id: string; severity: string; finding: string }[];
  } | null;
  catalog: CatalogRow[];
}) {
  const devices = summary?.devices_detail ?? [];
  const [selected, setSelected] = useState<string>("");
  const activeId = selected || devices[0]?.id || "";
  const active = devices.find((d) => d.id === activeId) ?? devices[0];

  const memStack = useMemo(
    () =>
      devices.map((d) => ({
        name: d.id,
        used: d.memory_used_mb,
        reserved: d.memory_reserved_mb ?? 0,
        free: Math.max(0, d.memory_free_mb ?? d.memory_total_mb - d.memory_used_mb),
        pct: d.memory_used_percent,
      })),
    [devices],
  );

  const utilSeries = useMemo(
    () =>
      devices.map((d) => ({
        name: d.id,
        sm_active: d.sm_active,
        occupancy: d.sm_occupancy,
        util: d.gpu_util,
        dram: d.dram_active,
        gr: d.gr_engine_active,
        tensor: d.pipe_tensor_active,
      })),
    [devices],
  );

  const powerTemp = useMemo(
    () =>
      devices.map((d) => ({
        name: d.id,
        power: d.power_usage_w,
        limit: d.power_management_limit_w,
        temp: d.temperature_c,
        memTemp: d.memory_temperature_c,
      })),
    [devices],
  );

  const timeSeries = useMemo(() => {
    const byTs = new Map<number, Record<string, number | string>>();
    const sorted = [...samples].sort((a, b) => a.timestamp_ms - b.timestamp_ms);
    for (const s of sorted) {
      const row =
        byTs.get(s.timestamp_ms) ??
        ({
          t: new Date(s.timestamp_ms).toLocaleTimeString([], { hour: "2-digit", minute: "2-digit" }),
        } as Record<string, number | string>);
      row[`${s.gpu_id}_sm`] = s.sm_active;
      row[`${s.gpu_id}_mem`] = s.memory_used_percent;
      row[`${s.gpu_id}_pwr`] = s.power_usage_w;
      byTs.set(s.timestamp_ms, row);
    }
    return Array.from(byTs.entries())
      .sort((a, b) => a[0] - b[0])
      .map(([, v]) => v)
      .slice(-36);
  }, [samples]);

  const memTime = useMemo(() => {
    const byTs = new Map<number, Record<string, number | string>>();
    for (const s of [...samples].sort((a, b) => a.timestamp_ms - b.timestamp_ms)) {
      const row =
        byTs.get(s.timestamp_ms) ??
        ({
          t: new Date(s.timestamp_ms).toLocaleTimeString([], { hour: "2-digit", minute: "2-digit" }),
        } as Record<string, number | string>);
      row[s.gpu_id] = s.memory_used_percent;
      byTs.set(s.timestamp_ms, row);
    }
    return Array.from(byTs.entries())
      .sort((a, b) => a[0] - b[0])
      .map(([, v]) => v)
      .slice(-36);
  }, [samples]);

  const groups = useMemo(() => {
    const g = new Map<string, CatalogRow[]>();
    for (const m of catalog) {
      const list = g.get(m.group) ?? [];
      list.push(m);
      g.set(m.group, list);
    }
    return Array.from(g.entries());
  }, [catalog]);

  const processMem = useMemo(
    () =>
      procs.map((p) => ({
        name: `${p.process_name}@${p.gpu_id}`,
        mem: p.memory_usage_mb,
        sm: p.sm_active,
      })),
    [procs],
  );

  return (
    <div className="obs-section gpu-studio">
      <div className="obs-kpi-row">
        <Kpi label="Devices" value={String(summary?.devices ?? "—")} />
        <Kpi label="SM active" value={`${fmt(summary?.avg_sm_active, 1)}%`} hint="fleet avg" />
        <Kpi label="Saturation" value={`${fmt(summary?.avg_sm_occupancy, 1)}%`} hint="sm_occupancy" />
        <Kpi
          label="Memory used"
          value={`${fmt(summary?.avg_memory_used_percent, 1)}%`}
          hint={`${fmt((summary?.total_memory_mb ?? 0) / 1024, 1)} / ${fmt((summary?.total_memory_capacity_mb ?? 0) / 1024, 1)} GiB`}
        />
        <Kpi label="Power" value={`${fmt(summary?.total_power_w, 0)} W`} />
        <Kpi label="Temp" value={`${fmt(summary?.avg_temperature_c, 0)}°C`} />
        <Kpi label="Metrics" value={String(summary?.metrics_catalog_count ?? catalog.length)} hint="all supported" />
        <Kpi
          label="Health"
          value={health ? `${health.healthy} ok / ${health.degraded} bad` : "—"}
        />
      </div>

      <div className="gpu-device-picker">
        <span>Focus device</span>
        {devices.map((d) => (
          <button
            key={d.id}
            className={d.id === activeId ? "on" : ""}
            onClick={() => setSelected(d.id)}
          >
            {d.id}
            <small>{d.model.replace("NVIDIA ", "")}</small>
          </button>
        ))}
      </div>

      <div className="gpu-chart-grid">
        <ChartCard
          title="GPU memory usage"
          subtitle="Frame buffer used · reserved · free (MiB) — stacked composition"
          tall
        >
          <ResponsiveContainer width="100%" height="100%">
            <BarChart data={memStack} margin={{ top: 12, right: 16, left: 8, bottom: 8 }} barCategoryGap="28%">
              <CartesianGrid stroke="rgba(255,255,255,0.05)" vertical={false} />
              <XAxis dataKey="name" tick={{ fill: "#9AA6B8", fontSize: 11 }} axisLine={false} tickLine={false} />
              <YAxis
                tick={{ fill: "#9AA6B8", fontSize: 11 }}
                axisLine={false}
                tickLine={false}
                tickFormatter={(v) => `${Math.round(v / 1024)}G`}
                width={42}
              />
              <Tooltip
                contentStyle={tip}
                formatter={(value, name) => [`${fmt(Number(value), 0)} MiB`, String(name)]}
              />
              <Legend wrapperStyle={{ fontSize: 12, color: "#9AA6B8" }} />
              <Bar dataKey="used" stackId="m" fill="#5B91EB" name="Used" radius={[0, 0, 0, 0]} />
              <Bar dataKey="reserved" stackId="m" fill="#A371E3" name="Reserved" />
              <Bar dataKey="free" stackId="m" fill="rgba(46,196,182,0.45)" name="Free" radius={[4, 4, 0, 0]} />
            </BarChart>
          </ResponsiveContainer>
        </ChartCard>

        <ChartCard
          title="Memory used % over time"
          subtitle="gpu.memory.used_percent by device — continuous series"
          tall
        >
          <ResponsiveContainer width="100%" height="100%">
            <AreaChart data={memTime} margin={{ top: 12, right: 16, left: 0, bottom: 8 }}>
              <defs>
                {devices.map((d, i) => (
                  <linearGradient key={d.id} id={`memFill-${d.id}`} x1="0" y1="0" x2="0" y2="1">
                    <stop offset="0%" stopColor={GPU_COLORS[i % GPU_COLORS.length]} stopOpacity={0.35} />
                    <stop offset="100%" stopColor={GPU_COLORS[i % GPU_COLORS.length]} stopOpacity={0.02} />
                  </linearGradient>
                ))}
              </defs>
              <CartesianGrid stroke="rgba(255,255,255,0.05)" vertical={false} />
              <XAxis dataKey="t" tick={{ fill: "#9AA6B8", fontSize: 10 }} minTickGap={28} axisLine={false} tickLine={false} />
              <YAxis domain={[0, 100]} tick={{ fill: "#9AA6B8", fontSize: 11 }} axisLine={false} tickLine={false} width={36} unit="%" />
              <Tooltip contentStyle={tip} />
              <Legend wrapperStyle={{ fontSize: 12, color: "#9AA6B8" }} />
              {devices.map((d, i) => (
                <Area
                  key={d.id}
                  type="monotone"
                  dataKey={d.id}
                  name={d.id}
                  stroke={GPU_COLORS[i % GPU_COLORS.length]}
                  fill={`url(#memFill-${d.id})`}
                  strokeWidth={2.25}
                  dot={false}
                  isAnimationActive={false}
                />
              ))}
            </AreaChart>
          </ResponsiveContainer>
        </ChartCard>

        <ChartCard
          title="Compute efficiency"
          subtitle="SM active · occupancy · util · DRAM · graphics engine · tensor %"
          tall
        >
          <ResponsiveContainer width="100%" height="100%">
            <BarChart data={utilSeries} margin={{ top: 12, right: 12, left: 0, bottom: 8 }} barGap={2}>
              <CartesianGrid stroke="rgba(255,255,255,0.05)" vertical={false} />
              <XAxis dataKey="name" tick={{ fill: "#9AA6B8", fontSize: 11 }} axisLine={false} tickLine={false} />
              <YAxis domain={[0, 100]} tick={{ fill: "#9AA6B8", fontSize: 11 }} axisLine={false} tickLine={false} width={36} />
              <Tooltip contentStyle={tip} />
              <Legend wrapperStyle={{ fontSize: 11, color: "#9AA6B8" }} />
              <Bar dataKey="sm_active" fill="#5B91EB" name="SM active" radius={[3, 3, 0, 0]} />
              <Bar dataKey="occupancy" fill="#A371E3" name="Occupancy" radius={[3, 3, 0, 0]} />
              <Bar dataKey="util" fill="#2EC4B6" name="Util" radius={[3, 3, 0, 0]} />
              <Bar dataKey="dram" fill="#4CC9F0" name="DRAM" radius={[3, 3, 0, 0]} />
              <Bar dataKey="tensor" fill="#F4A261" name="Tensor" radius={[3, 3, 0, 0]} />
            </BarChart>
          </ResponsiveContainer>
        </ChartCard>

        <ChartCard
          title="Power & temperature"
          subtitle="Board watts vs power limit · die / memory °C (dual axis)"
          tall
        >
          <ResponsiveContainer width="100%" height="100%">
            <ComposedChart data={powerTemp} margin={{ top: 12, right: 12, left: 0, bottom: 8 }}>
              <CartesianGrid stroke="rgba(255,255,255,0.05)" vertical={false} />
              <XAxis dataKey="name" tick={{ fill: "#9AA6B8", fontSize: 11 }} axisLine={false} tickLine={false} />
              <YAxis
                yAxisId="w"
                tick={{ fill: "#9AA6B8", fontSize: 11 }}
                axisLine={false}
                tickLine={false}
                width={40}
                label={{ value: "W", position: "insideTopLeft", fill: "#9AA6B8", fontSize: 10 }}
              />
              <YAxis
                yAxisId="c"
                orientation="right"
                tick={{ fill: "#9AA6B8", fontSize: 11 }}
                axisLine={false}
                tickLine={false}
                width={36}
                domain={[0, 100]}
                label={{ value: "°C", position: "insideTopRight", fill: "#9AA6B8", fontSize: 10 }}
              />
              <Tooltip contentStyle={tip} />
              <Legend wrapperStyle={{ fontSize: 11, color: "#9AA6B8" }} />
              <Bar yAxisId="w" dataKey="power" fill="#F4A261" name="Power W" radius={[4, 4, 0, 0]} barSize={22} />
              <Bar yAxisId="w" dataKey="limit" fill="rgba(139,151,168,0.35)" name="Limit W" radius={[4, 4, 0, 0]} barSize={22} />
              <Line
                yAxisId="c"
                type="monotone"
                dataKey="temp"
                stroke="#F25F5C"
                strokeWidth={2.5}
                dot={{ r: 4, fill: "#F25F5C", strokeWidth: 0 }}
                name="Die °C"
                isAnimationActive={false}
              />
              <Line
                yAxisId="c"
                type="monotone"
                dataKey="memTemp"
                stroke="#E9C46A"
                strokeWidth={2}
                strokeDasharray="4 3"
                dot={false}
                name="Mem °C"
                isAnimationActive={false}
              />
            </ComposedChart>
          </ResponsiveContainer>
        </ChartCard>

        <ChartCard
          title="SM active timeline"
          subtitle="gpu.sm_active — multi-series continuous monitoring"
          tall
        >
          <ResponsiveContainer width="100%" height="100%">
            <LineChart data={timeSeries} margin={{ top: 12, right: 16, left: 0, bottom: 8 }}>
              <CartesianGrid stroke="rgba(255,255,255,0.05)" vertical={false} />
              <XAxis dataKey="t" tick={{ fill: "#9AA6B8", fontSize: 10 }} minTickGap={28} axisLine={false} tickLine={false} />
              <YAxis domain={[0, 100]} tick={{ fill: "#9AA6B8", fontSize: 11 }} axisLine={false} tickLine={false} width={36} />
              <Tooltip contentStyle={tip} />
              <Legend wrapperStyle={{ fontSize: 12, color: "#9AA6B8" }} />
              {devices.map((d, i) => (
                <Line
                  key={d.id}
                  type="monotone"
                  dataKey={`${d.id}_sm`}
                  name={d.id}
                  stroke={GPU_COLORS[i % GPU_COLORS.length]}
                  strokeWidth={2.25}
                  dot={false}
                  isAnimationActive={false}
                />
              ))}
            </LineChart>
          </ResponsiveContainer>
        </ChartCard>

        <ChartCard
          title="Process memory attribution"
          subtitle="gpu.process.memory.usage — who holds HBM"
          tall
        >
          <ResponsiveContainer width="100%" height="100%">
            <BarChart data={processMem} layout="vertical" margin={{ top: 8, right: 16, left: 8, bottom: 8 }}>
              <CartesianGrid stroke="rgba(255,255,255,0.05)" horizontal={false} />
              <XAxis
                type="number"
                tick={{ fill: "#9AA6B8", fontSize: 11 }}
                axisLine={false}
                tickLine={false}
                tickFormatter={(v) => `${Math.round(v / 1024)}G`}
              />
              <YAxis
                type="category"
                dataKey="name"
                width={128}
                tick={{ fill: "#9AA6B8", fontSize: 10 }}
                axisLine={false}
                tickLine={false}
              />
              <Tooltip
                contentStyle={tip}
                formatter={(value, name) =>
                  name === "mem" ? [`${fmt(Number(value), 0)} MiB`, "Memory"] : [fmt(Number(value), 1), "SM %"]
                }
              />
              <Bar dataKey="mem" fill="#5B91EB" name="mem" radius={[0, 4, 4, 0]} barSize={14} />
            </BarChart>
          </ResponsiveContainer>
        </ChartCard>
      </div>

      <div className="gpu-chart-card">
        <header>
          <h3>Device inventory</h3>
          <p>Live snapshot — memory, SM, tensor, power, health</p>
        </header>
        <table className="dd-table">
          <thead>
            <tr>
              <th>GPU</th>
              <th>Model</th>
              <th>Host</th>
              <th>Mem used</th>
              <th>Mem free</th>
              <th>Mem %</th>
              <th>SM%</th>
              <th>Occ%</th>
              <th>Tensor%</th>
              <th>Power</th>
              <th>Temp</th>
              <th>Health</th>
            </tr>
          </thead>
          <tbody>
            {devices.map((d) => (
              <tr key={d.id} className={d.id === activeId ? "row-active" : ""} onClick={() => setSelected(d.id)}>
                <td>
                  <code>{d.id}</code>
                </td>
                <td className="tags">{d.model}</td>
                <td>{d.host}</td>
                <td>{fmt(d.memory_used_mb, 0)} MiB</td>
                <td>{fmt(d.memory_free_mb, 0)} MiB</td>
                <td>
                  <div className="mem-bar">
                    <i style={{ width: `${Math.min(100, d.memory_used_percent)}%` }} />
                    <span>{fmt(d.memory_used_percent, 1)}%</span>
                  </div>
                </td>
                <td>{fmt(d.sm_active, 1)}</td>
                <td>{fmt(d.sm_occupancy, 1)}</td>
                <td>{fmt(d.pipe_tensor_active, 1)}</td>
                <td>
                  {fmt(d.power_usage_w, 0)}/{fmt(d.power_management_limit_w, 0)}W
                </td>
                <td>{fmt(d.temperature_c, 0)}°C</td>
                <td>
                  <span
                    className="pill"
                    style={{
                      background:
                        d.health_status === "healthy"
                          ? "#2ec4b6"
                          : d.health_status === "throttled"
                            ? "#f4a261"
                            : "#f25f5c",
                    }}
                  >
                    {d.health_status}
                  </span>
                </td>
              </tr>
            ))}
          </tbody>
        </table>
      </div>

      <div className="obs-grid-2">
        <div className="gpu-chart-card">
          <header>
            <h3>Process / pod attribution</h3>
            <p>Connected entities — SM + memory per workload</p>
          </header>
          <table className="dd-table compact">
            <thead>
              <tr>
                <th>PID</th>
                <th>GPU</th>
                <th>Process</th>
                <th>Pod</th>
                <th>SM%</th>
                <th>Mem</th>
              </tr>
            </thead>
            <tbody>
              {procs.map((p) => (
                <tr key={`${p.gpu_id}-${p.pid}`}>
                  <td>{p.pid}</td>
                  <td>
                    <code>{p.gpu_id}</code>
                  </td>
                  <td>{p.process_name}</td>
                  <td className="tags">{p.pod ?? "—"}</td>
                  <td>{fmt(p.sm_active, 1)}</td>
                  <td>{fmt(p.memory_usage_mb, 0)} MiB</td>
                </tr>
              ))}
            </tbody>
          </table>
        </div>
        <div className="gpu-chart-card">
          <header>
            <h3>Health findings</h3>
            <p>Xid · ECC · throttle · zombie allocations</p>
          </header>
          <table className="dd-table compact">
            <thead>
              <tr>
                <th>GPU</th>
                <th>Severity</th>
                <th>Finding</th>
              </tr>
            </thead>
            <tbody>
              {(health?.findings ?? []).map((f, i) => (
                <tr key={i}>
                  <td>
                    <code>{f.gpu_id}</code>
                  </td>
                  <td>{f.severity}</td>
                  <td>{f.finding}</td>
                </tr>
              ))}
            </tbody>
          </table>
        </div>
      </div>

      <div className="gpu-chart-card">
        <header>
          <h3>
            All metrics live — {active?.id ?? "—"} ({catalog.length} supported)
          </h3>
          <p>
            Every DCGM/Datadog gpu.* field with current value on the focused device
            {active ? ` · ${active.model}` : ""}
          </p>
        </header>
        {groups.map(([group, rows]) => (
          <div key={group} className="gpu-metric-group">
            <h4>{group}</h4>
            <table className="dd-table compact">
              <thead>
                <tr>
                  <th>Metric</th>
                  <th>Live value</th>
                  <th>DCGM field</th>
                  <th>Unit</th>
                  <th>Description</th>
                </tr>
              </thead>
              <tbody>
                {rows.map((m) => {
                  const v = metricValue(m, active, procs);
                  return (
                    <tr key={m.name}>
                      <td>
                        <code>{m.name}</code>
                      </td>
                      <td className="gpu-live-val">{formatMetric(m.unit, v)}</td>
                      <td className="tags">{m.dcgm_field}</td>
                      <td>{m.unit}</td>
                      <td>{m.description}</td>
                    </tr>
                  );
                })}
              </tbody>
            </table>
          </div>
        ))}
      </div>
    </div>
  );
}
