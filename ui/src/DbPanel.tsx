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
import type {
  DbActivity,
  DbBlocking,
  DbHostSample,
  DbInstance,
  DbMetricDef,
  DbQuery,
  DbQueryMetric,
  DbSchema,
  DbWaitEvent,
  ExplainPlan,
} from "./api";
import { fmt } from "./widgets";
import { chartTip } from "./theme";

const tip = { ...chartTip, borderRadius: 8, padding: "10px 12px" };

const COLORS = ["#5B91EB", "#2EC4B6", "#A371E3", "#F4A261", "#F25F5C", "#4CC9F0"];

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

/** Live value for every catalog metric from instance (+ query rollups). */
function instanceValue(
  d: DbInstance,
  metricName: string,
  queryMetrics: DbQueryMetric[],
): number | null {
  const qm = queryMetrics.filter((q) => q.db === d.name);
  const sumQm = (fn: (q: DbQueryMetric) => number) =>
    qm.length ? qm.reduce((a, q) => a + fn(q), 0) : null;

  const map: Record<string, number | undefined | null> = {
    // connections
    "postgresql.connections": d.connections,
    "postgresql.active_connections": d.active_connections,
    "postgresql.idle_connections": d.idle_connections,
    "postgresql.waiting_connections": d.waiting_connections,
    "postgresql.max_connections": d.max_connections,
    "postgresql.percent_usage_connections": d.connections_pct,
    "mysql.performance.threads_connected": d.connections,
    "mysql.performance.threads_running": d.active_connections,
    "mysql.net.max_connections": d.max_connections,
    "mysql.net.connections": d.connections * 0.08,
    "mysql.net.aborted_connects": d.connection_errors_per_sec,
    // throughput
    "postgresql.queries.count": d.qps,
    "postgresql.commits": d.tps,
    "postgresql.rollbacks": d.rollbacks_per_sec,
    "postgresql.rows_returned": d.rows_returned_per_sec,
    "postgresql.rows_fetched": d.rows_fetched_per_sec,
    "postgresql.rows_inserted": d.rows_inserted_per_sec,
    "postgresql.rows_updated": d.rows_updated_per_sec,
    "postgresql.rows_deleted": d.rows_deleted_per_sec,
    "mysql.performance.queries": d.qps,
    "mysql.performance.questions": d.qps * 0.92,
    "mysql.performance.com_select": d.qps * 0.7,
    "mysql.performance.com_insert": d.rows_inserted_per_sec,
    "mysql.performance.com_update": d.rows_updated_per_sec,
    "mysql.performance.com_delete": d.rows_deleted_per_sec,
    "mysql.performance.slow_queries": d.slow_queries / 60,
    // latency
    "postgresql.queries.time": d.avg_query_ms,
    "postgresql.queries.p95_time": d.p95_query_ms,
    "postgresql.queries.max_time": d.p99_query_ms,
    "mysql.queries.time": d.avg_query_ms,
    "mysql.queries.p95_time": d.p95_query_ms,
    // buffer
    "postgresql.buffer_hit_ratio": d.buffer_hit_ratio,
    "postgresql.blocks_read": d.blocks_read_per_sec,
    "postgresql.blocks_hit": d.blocks_hit_per_sec,
    "postgresql.temp_bytes": d.temp_bytes_per_sec,
    "mysql.innodb.buffer_pool_reads": d.blocks_read_per_sec,
    "mysql.innodb.buffer_pool_read_requests": d.blocks_hit_per_sec + d.blocks_read_per_sec,
    "mysql.innodb.buffer_pool_utilization": d.buffer_pool_utilization,
    "mysql.innodb.buffer_pool_bytes_data": d.buffer_pool_bytes,
    "mysql.innodb.buffer_pool_bytes_dirty": d.buffer_pool_dirty_bytes,
    // locks
    "postgresql.locks.deadlocks": d.deadlocks_per_sec,
    "postgresql.locks.waiting": d.locks_waiting,
    "postgresql.locks.relations": d.locks_waiting * 3 + d.active_connections * 0.4,
    "mysql.innodb.deadlocks": d.deadlocks_per_sec,
    "mysql.innodb.row_lock_waits": d.locks_waiting,
    "mysql.innodb.row_lock_time": d.avg_lock_wait_ms,
    "mysql.performance.table_locks_waited": d.locks_waiting * 0.3,
    // replication
    "postgresql.replication_delay": d.replication_lag_ms,
    "postgresql.replication.slots.count": d.role === "primary" ? 2 : 0,
    "mysql.replication.seconds_behind_source": d.replication_lag_ms / 1000,
    "mysql.replication.slaves_connected": d.role === "primary" ? 1 : 0,
    // maintenance
    "postgresql.vacuum.running": d.autovacuum_workers,
    "postgresql.dead_rows": d.dead_rows,
    "postgresql.live_rows": d.live_rows,
    "postgresql.index_bloat": d.index_bloat_pct,
    "postgresql.table_bloat": d.table_bloat_pct,
    // io / resources
    "postgresql.disk_read_ops": d.disk_read_ops,
    "postgresql.disk_write_ops": d.disk_write_ops,
    "postgresql.cpu.user": d.cpu_pct * 0.72,
    "postgresql.cpu.system": d.cpu_pct * 0.28,
    "postgresql.mem.used_pct": d.mem_used_pct,
    "mysql.performance.created_tmp_tables": d.tmp_tables_per_sec,
    "mysql.performance.created_tmp_disk_tables": d.tmp_disk_tables_per_sec,
    "mysql.performance.open_files": d.open_files,
    "mysql.innodb.data_reads": d.disk_read_ops,
    "mysql.innodb.data_writes": d.disk_write_ops,
    "mysql.innodb.os_log_fsyncs": d.disk_write_ops * 0.35,
    // query-level rollups
    "postgresql.queries.rows": sumQm((q) => q.rows_per_sec) ?? undefined,
    "postgresql.queries.shared_blks_hit": sumQm((q) => q.shared_blks_hit) ?? undefined,
    "postgresql.queries.shared_blks_read": sumQm((q) => q.shared_blks_read) ?? undefined,
    "postgresql.queries.temp_blks_written": sumQm((q) => q.temp_blks_written) ?? undefined,
    "mysql.queries.rows": sumQm((q) => q.rows_examined_per_sec) ?? undefined,
    "mysql.queries.errors": d.connection_errors_per_sec * 0.4,
  };
  const v = map[metricName];
  return v == null ? null : v;
}

function formatLive(unit: string, v: number | null): string {
  if (v == null) return "—";
  if (unit === "%" || unit === "ms" || unit === "s") return `${fmt(v, v >= 100 ? 0 : 2)}${unit === "%" ? "%" : unit === "ms" ? "ms" : "s"}`;
  if (unit.includes("byte")) {
    if (v >= 1e9) return `${fmt(v / 1e9, 2)} GB`;
    if (v >= 1e6) return `${fmt(v / 1e6, 1)} MB`;
    if (v >= 1e3) return `${fmt(v / 1e3, 1)} KB`;
    return `${fmt(v, 0)} B`;
  }
  return fmt(v, v >= 100 ? 0 : 2);
}

type Props = {
  summary: Record<string, unknown> | null;
  instances: DbInstance[];
  samples: DbHostSample[];
  queryMetrics: DbQueryMetric[];
  queries: DbQuery[];
  explain: ExplainPlan[];
  schemas: DbSchema[];
  waits: DbWaitEvent[];
  blocking: DbBlocking[];
  activity: DbActivity[];
  health: { findings?: { db: string; severity: string; finding: string }[]; blocking?: number } | null;
  catalog: DbMetricDef[];
  apm: Record<string, unknown> | null;
};

export function DbPanel({
  summary,
  instances,
  samples,
  queryMetrics,
  queries,
  explain,
  schemas,
  waits,
  blocking,
  activity,
  health,
  catalog,
  apm,
}: Props) {
  const [dbFilter, setDbFilter] = useState<string>("all");
  const [metricGroup, setMetricGroup] = useState<string>("all");

  const dbs = useMemo(() => instances.map((d) => d.name), [instances]);
  const active = dbFilter === "all" ? instances : instances.filter((d) => d.name === dbFilter);
  const focus = dbFilter === "all" ? instances[0] : instances.find((d) => d.name === dbFilter);

  const timeSeries = useMemo(() => {
    const byTs = new Map<number, Record<string, number | string>>();
    for (const s of [...samples].sort((a, b) => a.timestamp_ms - b.timestamp_ms)) {
      if (dbFilter !== "all" && s.db !== dbFilter) continue;
      const row =
        byTs.get(s.timestamp_ms) ??
        ({
          t: new Date(s.timestamp_ms).toLocaleTimeString([], { hour: "2-digit", minute: "2-digit" }),
        } as Record<string, number | string>);
      row[`${s.db}_qps`] = s.qps;
      row[`${s.db}_p95`] = s.p95_query_ms;
      row[`${s.db}_lag`] = s.replication_lag_ms;
      row[`${s.db}_hit`] = s.buffer_hit_ratio;
      row[`${s.db}_conn`] = s.connections;
      row[`${s.db}_read`] = s.disk_read_ops;
      row[`${s.db}_write`] = s.disk_write_ops;
      row[`${s.db}_cpu`] = s.cpu_pct;
      row[`${s.db}_mem`] = s.mem_used_pct;
      row[`${s.db}_locks`] = s.locks_waiting;
      row[`${s.db}_dead`] = s.deadlocks;
      byTs.set(s.timestamp_ms, row);
    }
    return Array.from(byTs.entries())
      .sort((a, b) => a[0] - b[0])
      .map(([, v]) => v)
      .slice(-48);
  }, [samples, dbFilter]);

  const seriesDbs = dbFilter === "all" ? dbs : [dbFilter];

  const connBars = useMemo(
    () =>
      active.map((d) => ({
        name: d.name,
        active: d.active_connections,
        idle: d.idle_connections,
        waiting: d.waiting_connections,
      })),
    [active],
  );

  const throughputBars = useMemo(
    () =>
      active.map((d) => ({
        name: d.name,
        qps: d.qps,
        tps: d.tps,
        rollbacks: d.rollbacks_per_sec,
      })),
    [active],
  );

  const rowOps = useMemo(
    () =>
      active.map((d) => ({
        name: d.name,
        returned: d.rows_returned_per_sec,
        inserted: d.rows_inserted_per_sec,
        updated: d.rows_updated_per_sec,
        deleted: d.rows_deleted_per_sec,
      })),
    [active],
  );

  const resourceBars = useMemo(
    () =>
      active.map((d) => ({
        name: d.name,
        cpu: d.cpu_pct,
        mem: d.mem_used_pct,
        hit: d.buffer_hit_ratio,
        connPct: d.connections_pct,
      })),
    [active],
  );

  const waitBars = useMemo(() => {
    const filtered = waits.filter((w) => dbFilter === "all" || w.db === dbFilter);
    return [...filtered]
      .sort((a, b) => b.pct_wait_time - a.pct_wait_time)
      .slice(0, 10)
      .map((w) => ({
        name: `${w.event}`,
        pct: w.pct_wait_time,
        waits: w.waits,
        type: w.event_type,
      }));
  }, [waits, dbFilter]);

  const groups = useMemo(() => {
    const g = new Map<string, DbMetricDef[]>();
    for (const m of catalog) {
      if (metricGroup !== "all" && m.group !== metricGroup) continue;
      const list = g.get(m.group) ?? [];
      list.push(m);
      g.set(m.group, list);
    }
    return Array.from(g.entries());
  }, [catalog, metricGroup]);

  const filteredQm = useMemo(
    () => (dbFilter === "all" ? queryMetrics : queryMetrics.filter((q) => q.db === dbFilter)),
    [queryMetrics, dbFilter],
  );

  const filteredExplain = useMemo(
    () => (dbFilter === "all" ? explain : explain.filter((e) => e.db === dbFilter)),
    [explain, dbFilter],
  );

  const filteredSchemas = useMemo(
    () => (dbFilter === "all" ? schemas : schemas.filter((s) => s.db === dbFilter)),
    [schemas, dbFilter],
  );

  const apmSamples = useMemo(() => {
    const raw = apm?.query_samples;
    if (!Array.isArray(raw)) return [];
    return raw as { sql: string; db: string; duration_ms: number; calls: number; linked_service?: string }[];
  }, [apm]);

  const engines = (summary?.engines as Record<string, number> | undefined) ?? {};

  return (
    <div className="obs-section gpu-studio">
      <div className="obs-kpi-row">
        <Kpi label="Instances" value={String(summary?.instances ?? instances.length)} hint={`${engines.postgres ?? 0} pg · ${engines.mysql ?? 0} mysql`} />
        <Kpi label="Total QPS" value={fmt(Number(summary?.total_qps ?? 0), 0)} />
        <Kpi label="Slow queries" value={String(summary?.slow_queries ?? "—")} />
        <Kpi label="Query samples" value={String(summary?.query_samples ?? queries.length)} />
        <Kpi label="Buffer hit" value={`${fmt(Number(summary?.avg_buffer_hit_ratio ?? 0), 1)}%`} />
        <Kpi label="Max lag" value={`${fmt(Number(summary?.max_replication_lag_ms ?? 0), 0)}ms`} />
        <Kpi label="Explain plans" value={String(summary?.explain_plans ?? explain.length)} hint={`costliest ${fmt(Number(summary?.costliest_plan ?? 0), 1)}`} />
        <Kpi label="Tables" value={String(summary?.tables_tracked ?? schemas.length)} />
        <Kpi label="Metrics" value={String(summary?.metrics_catalog_count ?? catalog.length)} hint="postgresql.* / mysql.*" />
      </div>

      <div className="gpu-device-picker">
        <span>Database</span>
        <button className={dbFilter === "all" ? "on" : ""} onClick={() => setDbFilter("all")}>
          All
        </button>
        {dbs.map((name) => {
          const inst = instances.find((d) => d.name === name);
          return (
            <button
              key={name}
              className={dbFilter === name ? "on" : ""}
              onClick={() => setDbFilter(name)}
            >
              {name}
              <small>
                {inst?.engine} · {inst?.role}
              </small>
            </button>
          );
        })}
      </div>

      {(health?.findings?.length ?? 0) > 0 && (
        <div className="obs-card" style={{ marginBottom: "0.75rem" }}>
          <h3>Health findings</h3>
          <ul className="dbm-findings">
            {health!.findings!.map((f, i) => (
              <li key={i}>
                <span className={`sev ${f.severity}`}>{f.severity}</span>
                <code>{f.db}</code> {f.finding}
              </li>
            ))}
          </ul>
        </div>
      )}

      <div className="gpu-chart-grid">
        <ChartCard title="Query rate (QPS)" subtitle="postgresql.queries.count / mysql.performance.queries" tall>
          <ResponsiveContainer width="100%" height="100%">
            <AreaChart data={timeSeries} margin={{ top: 12, right: 16, left: 0, bottom: 8 }}>
              <CartesianGrid stroke="rgba(255,255,255,0.05)" vertical={false} />
              <XAxis dataKey="t" tick={{ fill: "#9AA6B8", fontSize: 10 }} minTickGap={28} axisLine={false} tickLine={false} />
              <YAxis tick={{ fill: "#9AA6B8", fontSize: 11 }} axisLine={false} tickLine={false} width={40} />
              <Tooltip contentStyle={tip} />
              <Legend />
              {seriesDbs.map((db, i) => (
                <Area
                  key={db}
                  type="monotone"
                  dataKey={`${db}_qps`}
                  name={db}
                  stroke={COLORS[i % COLORS.length]}
                  fill={COLORS[i % COLORS.length]}
                  fillOpacity={0.12}
                  strokeWidth={2}
                  dot={false}
                  isAnimationActive={false}
                />
              ))}
            </AreaChart>
          </ResponsiveContainer>
        </ChartCard>

        <ChartCard title="p95 query latency" subtitle="ms — spike windows highlight regressions" tall>
          <ResponsiveContainer width="100%" height="100%">
            <LineChart data={timeSeries} margin={{ top: 12, right: 16, left: 0, bottom: 8 }}>
              <CartesianGrid stroke="rgba(255,255,255,0.05)" vertical={false} />
              <XAxis dataKey="t" tick={{ fill: "#9AA6B8", fontSize: 10 }} minTickGap={28} axisLine={false} tickLine={false} />
              <YAxis tick={{ fill: "#9AA6B8", fontSize: 11 }} axisLine={false} tickLine={false} width={40} />
              <Tooltip contentStyle={tip} />
              <Legend />
              {seriesDbs.map((db, i) => (
                <Line
                  key={db}
                  type="monotone"
                  dataKey={`${db}_p95`}
                  name={db}
                  stroke={COLORS[i % COLORS.length]}
                  strokeWidth={2}
                  dot={false}
                  isAnimationActive={false}
                />
              ))}
            </LineChart>
          </ResponsiveContainer>
        </ChartCard>

        <ChartCard title="Connections" subtitle="Active / idle / waiting">
          <ResponsiveContainer width="100%" height="100%">
            <BarChart data={connBars} margin={{ top: 12, right: 8, left: 0, bottom: 8 }}>
              <CartesianGrid stroke="rgba(255,255,255,0.05)" vertical={false} />
              <XAxis dataKey="name" tick={{ fill: "#9AA6B8", fontSize: 10 }} axisLine={false} tickLine={false} />
              <YAxis tick={{ fill: "#9AA6B8", fontSize: 11 }} axisLine={false} tickLine={false} width={36} />
              <Tooltip contentStyle={tip} />
              <Legend />
              <Bar dataKey="active" stackId="c" fill="#5B91EB" />
              <Bar dataKey="idle" stackId="c" fill="#2EC4B6" />
              <Bar dataKey="waiting" stackId="c" fill="#F25F5C" />
            </BarChart>
          </ResponsiveContainer>
        </ChartCard>

        <ChartCard title="Throughput" subtitle="QPS vs TPS (commits) · rollbacks">
          <ResponsiveContainer width="100%" height="100%">
            <ComposedChart data={throughputBars} margin={{ top: 12, right: 8, left: 0, bottom: 8 }}>
              <CartesianGrid stroke="rgba(255,255,255,0.05)" vertical={false} />
              <XAxis dataKey="name" tick={{ fill: "#9AA6B8", fontSize: 10 }} axisLine={false} tickLine={false} />
              <YAxis tick={{ fill: "#9AA6B8", fontSize: 11 }} axisLine={false} tickLine={false} width={36} />
              <Tooltip contentStyle={tip} />
              <Legend />
              <Bar dataKey="qps" fill="#A371E3" name="QPS" />
              <Bar dataKey="tps" fill="#F4A261" name="TPS" />
              <Line type="monotone" dataKey="rollbacks" stroke="#F25F5C" strokeWidth={2} name="Rollbacks/s" dot={{ r: 3 }} />
            </ComposedChart>
          </ResponsiveContainer>
        </ChartCard>

        <ChartCard title="Disk I/O" subtitle="disk_read_ops / disk_write_ops over time" tall>
          <ResponsiveContainer width="100%" height="100%">
            <AreaChart data={timeSeries} margin={{ top: 12, right: 16, left: 0, bottom: 8 }}>
              <CartesianGrid stroke="rgba(255,255,255,0.05)" vertical={false} />
              <XAxis dataKey="t" tick={{ fill: "#9AA6B8", fontSize: 10 }} minTickGap={28} axisLine={false} tickLine={false} />
              <YAxis tick={{ fill: "#9AA6B8", fontSize: 11 }} axisLine={false} tickLine={false} width={40} />
              <Tooltip contentStyle={tip} />
              <Legend />
              {seriesDbs.map((db, i) => (
                <Area
                  key={`${db}-r`}
                  type="monotone"
                  dataKey={`${db}_read`}
                  name={`${db} read`}
                  stroke={COLORS[i % COLORS.length]}
                  fill={COLORS[i % COLORS.length]}
                  fillOpacity={0.1}
                  strokeWidth={2}
                  dot={false}
                  isAnimationActive={false}
                />
              ))}
              {seriesDbs.map((db, i) => (
                <Line
                  key={`${db}-w`}
                  type="monotone"
                  dataKey={`${db}_write`}
                  name={`${db} write`}
                  stroke={COLORS[i % COLORS.length]}
                  strokeWidth={1.5}
                  strokeDasharray="4 3"
                  dot={false}
                  isAnimationActive={false}
                />
              ))}
            </AreaChart>
          </ResponsiveContainer>
        </ChartCard>

        <ChartCard title="CPU & memory" subtitle="Host saturation for DB processes" tall>
          <ResponsiveContainer width="100%" height="100%">
            <LineChart data={timeSeries} margin={{ top: 12, right: 16, left: 0, bottom: 8 }}>
              <CartesianGrid stroke="rgba(255,255,255,0.05)" vertical={false} />
              <XAxis dataKey="t" tick={{ fill: "#9AA6B8", fontSize: 10 }} minTickGap={28} axisLine={false} tickLine={false} />
              <YAxis domain={[0, 100]} tick={{ fill: "#9AA6B8", fontSize: 11 }} axisLine={false} tickLine={false} width={36} unit="%" />
              <Tooltip contentStyle={tip} />
              <Legend />
              {seriesDbs.map((db, i) => (
                <Line
                  key={`${db}-cpu`}
                  type="monotone"
                  dataKey={`${db}_cpu`}
                  name={`${db} CPU`}
                  stroke={COLORS[i % COLORS.length]}
                  strokeWidth={2}
                  dot={false}
                  isAnimationActive={false}
                />
              ))}
              {seriesDbs.map((db, i) => (
                <Line
                  key={`${db}-mem`}
                  type="monotone"
                  dataKey={`${db}_mem`}
                  name={`${db} mem`}
                  stroke={COLORS[i % COLORS.length]}
                  strokeWidth={1.5}
                  strokeDasharray="3 3"
                  dot={false}
                  isAnimationActive={false}
                />
              ))}
            </LineChart>
          </ResponsiveContainer>
        </ChartCard>

        <ChartCard title="Replication lag" subtitle="ms behind primary / source" tall>
          <ResponsiveContainer width="100%" height="100%">
            <AreaChart data={timeSeries} margin={{ top: 12, right: 16, left: 0, bottom: 8 }}>
              <CartesianGrid stroke="rgba(255,255,255,0.05)" vertical={false} />
              <XAxis dataKey="t" tick={{ fill: "#9AA6B8", fontSize: 10 }} minTickGap={28} axisLine={false} tickLine={false} />
              <YAxis tick={{ fill: "#9AA6B8", fontSize: 11 }} axisLine={false} tickLine={false} width={40} />
              <Tooltip contentStyle={tip} />
              <Legend />
              {seriesDbs.map((db, i) => (
                <Area
                  key={db}
                  type="monotone"
                  dataKey={`${db}_lag`}
                  name={db}
                  stroke={COLORS[i % COLORS.length]}
                  fill={COLORS[i % COLORS.length]}
                  fillOpacity={0.15}
                  strokeWidth={2}
                  dot={false}
                  isAnimationActive={false}
                />
              ))}
            </AreaChart>
          </ResponsiveContainer>
        </ChartCard>

        <ChartCard title="Buffer hit ratio" subtitle="% — cache effectiveness">
          <ResponsiveContainer width="100%" height="100%">
            <LineChart data={timeSeries} margin={{ top: 12, right: 16, left: 0, bottom: 8 }}>
              <CartesianGrid stroke="rgba(255,255,255,0.05)" vertical={false} />
              <XAxis dataKey="t" tick={{ fill: "#9AA6B8", fontSize: 10 }} minTickGap={28} axisLine={false} tickLine={false} />
              <YAxis domain={[90, 100]} tick={{ fill: "#9AA6B8", fontSize: 11 }} axisLine={false} tickLine={false} width={36} />
              <Tooltip contentStyle={tip} />
              {seriesDbs.map((db, i) => (
                <Line
                  key={db}
                  type="monotone"
                  dataKey={`${db}_hit`}
                  name={db}
                  stroke={COLORS[i % COLORS.length]}
                  strokeWidth={2}
                  dot={false}
                  isAnimationActive={false}
                />
              ))}
            </LineChart>
          </ResponsiveContainer>
        </ChartCard>

        <ChartCard title="Row operations" subtitle="returned · inserted · updated · deleted /s">
          <ResponsiveContainer width="100%" height="100%">
            <BarChart data={rowOps} margin={{ top: 12, right: 8, left: 0, bottom: 8 }}>
              <CartesianGrid stroke="rgba(255,255,255,0.05)" vertical={false} />
              <XAxis dataKey="name" tick={{ fill: "#9AA6B8", fontSize: 10 }} axisLine={false} tickLine={false} />
              <YAxis tick={{ fill: "#9AA6B8", fontSize: 11 }} axisLine={false} tickLine={false} width={42} />
              <Tooltip contentStyle={tip} />
              <Legend />
              <Bar dataKey="returned" fill="#5B91EB" name="Returned" />
              <Bar dataKey="inserted" fill="#2EC4B6" name="Inserted" />
              <Bar dataKey="updated" fill="#F4A261" name="Updated" />
              <Bar dataKey="deleted" fill="#F25F5C" name="Deleted" />
            </BarChart>
          </ResponsiveContainer>
        </ChartCard>

        <ChartCard title="Resource saturation" subtitle="CPU · mem · hit% · conn%">
          <ResponsiveContainer width="100%" height="100%">
            <BarChart data={resourceBars} margin={{ top: 12, right: 8, left: 0, bottom: 8 }}>
              <CartesianGrid stroke="rgba(255,255,255,0.05)" vertical={false} />
              <XAxis dataKey="name" tick={{ fill: "#9AA6B8", fontSize: 10 }} axisLine={false} tickLine={false} />
              <YAxis domain={[0, 100]} tick={{ fill: "#9AA6B8", fontSize: 11 }} axisLine={false} tickLine={false} width={36} />
              <Tooltip contentStyle={tip} />
              <Legend />
              <Bar dataKey="cpu" fill="#F25F5C" name="CPU%" radius={[3, 3, 0, 0]} />
              <Bar dataKey="mem" fill="#A371E3" name="Mem%" radius={[3, 3, 0, 0]} />
              <Bar dataKey="hit" fill="#2EC4B6" name="Hit%" radius={[3, 3, 0, 0]} />
              <Bar dataKey="connPct" fill="#5B91EB" name="Conn%" radius={[3, 3, 0, 0]} />
            </BarChart>
          </ResponsiveContainer>
        </ChartCard>

        <ChartCard title="Wait events" subtitle="% wait time — Lock / IO / LWLock / Client" tall>
          <ResponsiveContainer width="100%" height="100%">
            <BarChart data={waitBars} layout="vertical" margin={{ top: 8, right: 16, left: 8, bottom: 8 }}>
              <CartesianGrid stroke="rgba(255,255,255,0.05)" horizontal={false} />
              <XAxis type="number" tick={{ fill: "#9AA6B8", fontSize: 11 }} axisLine={false} tickLine={false} unit="%" />
              <YAxis type="category" dataKey="name" width={120} tick={{ fill: "#9AA6B8", fontSize: 10 }} axisLine={false} tickLine={false} />
              <Tooltip contentStyle={tip} />
              <Bar dataKey="pct" fill="#F4A261" name="% wait" radius={[0, 4, 4, 0]} barSize={12} />
            </BarChart>
          </ResponsiveContainer>
        </ChartCard>

        <ChartCard title="Lock pressure" subtitle="locks_waiting · deadlocks over time" tall>
          <ResponsiveContainer width="100%" height="100%">
            <ComposedChart data={timeSeries} margin={{ top: 12, right: 16, left: 0, bottom: 8 }}>
              <CartesianGrid stroke="rgba(255,255,255,0.05)" vertical={false} />
              <XAxis dataKey="t" tick={{ fill: "#9AA6B8", fontSize: 10 }} minTickGap={28} axisLine={false} tickLine={false} />
              <YAxis tick={{ fill: "#9AA6B8", fontSize: 11 }} axisLine={false} tickLine={false} width={36} />
              <Tooltip contentStyle={tip} />
              <Legend />
              {seriesDbs.map((db, i) => (
                <Area
                  key={`${db}-l`}
                  type="monotone"
                  dataKey={`${db}_locks`}
                  name={`${db} waiting`}
                  stroke={COLORS[i % COLORS.length]}
                  fill={COLORS[i % COLORS.length]}
                  fillOpacity={0.12}
                  strokeWidth={2}
                  dot={false}
                  isAnimationActive={false}
                />
              ))}
              {seriesDbs.map((db, i) => (
                <Line
                  key={`${db}-d`}
                  type="monotone"
                  dataKey={`${db}_dead`}
                  name={`${db} deadlocks`}
                  stroke={COLORS[(i + 3) % COLORS.length]}
                  strokeWidth={2}
                  dot={false}
                  isAnimationActive={false}
                />
              ))}
            </ComposedChart>
          </ResponsiveContainer>
        </ChartCard>
      </div>

      <div className="obs-card" style={{ marginTop: "0.75rem" }}>
        <h3>Instances</h3>
        <p className="muted">Host metrics — connections, throughput, cache, replication, resources</p>
        <table className="dd-table compact">
          <thead>
            <tr>
              <th>Name</th>
              <th>Engine</th>
              <th>Role</th>
              <th>QPS</th>
              <th>TPS</th>
              <th>Avg / p95 / p99</th>
              <th>Conns</th>
              <th>Hit %</th>
              <th>Lag</th>
              <th>CPU</th>
              <th>Mem</th>
              <th>I/O r/w</th>
              <th>Size</th>
            </tr>
          </thead>
          <tbody>
            {active.map((d) => (
              <tr key={d.name} className={d.name === focus?.name ? "row-active" : ""}>
                <td>
                  <code>{d.name}</code>
                  <div className="muted">
                    {d.host}:{d.port}
                  </div>
                </td>
                <td>
                  {d.engine} {d.version}
                </td>
                <td>{d.role}</td>
                <td>{fmt(d.qps, 0)}</td>
                <td>{fmt(d.tps, 0)}</td>
                <td>
                  {fmt(d.avg_query_ms, 0)} / {fmt(d.p95_query_ms, 0)} / {fmt(d.p99_query_ms, 0)}ms
                </td>
                <td>
                  {fmt(d.connections, 0)}/{fmt(d.max_connections, 0)}
                  <div className="muted">{fmt(d.connections_pct, 0)}%</div>
                </td>
                <td>{fmt(d.buffer_hit_ratio, 1)}%</td>
                <td>{fmt(d.replication_lag_ms, 0)}ms</td>
                <td>{fmt(d.cpu_pct, 0)}%</td>
                <td>{fmt(d.mem_used_pct, 0)}%</td>
                <td>
                  {fmt(d.disk_read_ops, 0)}/{fmt(d.disk_write_ops, 0)}
                </td>
                <td>{fmt(d.size_gb, 0)} GB</td>
              </tr>
            ))}
          </tbody>
        </table>
      </div>

      <div className="obs-card" style={{ marginTop: "0.75rem" }}>
        <h3>Query metrics</h3>
        <p className="muted">Normalized fingerprints — latency, % time, rows, buffer I/O, APM service</p>
        <table className="dd-table">
          <thead>
            <tr>
              <th>Fingerprint</th>
              <th>DB</th>
              <th>Calls</th>
              <th>RPS</th>
              <th>Avg</th>
              <th>p95</th>
              <th>Max</th>
              <th>% time</th>
              <th>Rows/s</th>
              <th>Examined/s</th>
              <th>Blks hit</th>
              <th>Blks read</th>
              <th>Temp</th>
              <th>Service</th>
            </tr>
          </thead>
          <tbody>
            {filteredQm.map((q) => (
              <tr key={`${q.db}:${q.fingerprint}`}>
                <td>
                  <code>{q.fingerprint}</code>
                  <div className="sql-cell muted">{q.sql}</div>
                </td>
                <td>{q.db}</td>
                <td>{q.calls.toLocaleString()}</td>
                <td>{fmt(q.requests_per_sec, 2)}</td>
                <td>{fmt(q.avg_latency_ms, 1)}ms</td>
                <td>{fmt(q.p95_latency_ms, 0)}ms</td>
                <td>{fmt(q.max_latency_ms, 0)}ms</td>
                <td>{fmt(q.pct_time * 100, 0)}%</td>
                <td>{fmt(q.rows_per_sec, 0)}</td>
                <td>{fmt(q.rows_examined_per_sec, 1)}</td>
                <td>{fmt(q.shared_blks_hit, 0)}</td>
                <td>{fmt(q.shared_blks_read, 0)}</td>
                <td>{fmt(q.temp_blks_written, 0)}</td>
                <td>{q.apm_service || "—"}</td>
              </tr>
            ))}
          </tbody>
        </table>
      </div>

      <div className="obs-grid-2" style={{ marginTop: "0.75rem" }}>
        <div className="obs-card">
          <h3>EXPLAIN plans</h3>
          <table className="dd-table compact">
            <thead>
              <tr>
                <th>Fingerprint</th>
                <th>DB</th>
                <th>Cost</th>
                <th>Rows</th>
                <th>Root</th>
              </tr>
            </thead>
            <tbody>
              {filteredExplain.map((e) => (
                <tr key={`${e.db}:${e.query_fingerprint}`}>
                  <td>
                    <code>{e.query_fingerprint}</code>
                  </td>
                  <td>{e.db}</td>
                  <td>{fmt(e.total_cost, 1)}</td>
                  <td>{e.estimated_rows.toLocaleString()}</td>
                  <td className="muted">
                    {String(
                      (e.plan as { "Node Type"?: string; query_block?: unknown })["Node Type"] ||
                        (e.plan.query_block ? "query_block" : "—"),
                    )}
                  </td>
                </tr>
              ))}
            </tbody>
          </table>
        </div>
        <div className="obs-card">
          <h3>Schema inventory</h3>
          <table className="dd-table compact">
            <thead>
              <tr>
                <th>Table</th>
                <th>Rows</th>
                <th>Indexes</th>
                <th>Columns</th>
              </tr>
            </thead>
            <tbody>
              {filteredSchemas.map((s) => (
                <tr key={`${s.db}.${s.schema}.${s.table}`}>
                  <td>
                    <code>
                      {s.db}/{s.schema}.{s.table}
                    </code>
                  </td>
                  <td>{s.approx_rows.toLocaleString()}</td>
                  <td className="tags">{s.indexes.join(", ")}</td>
                  <td className="muted">{s.columns.join(", ")}</td>
                </tr>
              ))}
            </tbody>
          </table>
        </div>
      </div>

      <div className="obs-grid-2" style={{ marginTop: "0.75rem" }}>
        <div className="obs-card">
          <h3>Wait events</h3>
          <table className="dd-table compact">
            <thead>
              <tr>
                <th>DB</th>
                <th>Type</th>
                <th>Event</th>
                <th>Waits</th>
                <th>Wait ms</th>
                <th>% time</th>
              </tr>
            </thead>
            <tbody>
              {waits
                .filter((w) => dbFilter === "all" || w.db === dbFilter)
                .map((w, i) => (
                  <tr key={i}>
                    <td>{w.db}</td>
                    <td>{w.event_type}</td>
                    <td>
                      <code>{w.event}</code>
                    </td>
                    <td>{w.waits.toLocaleString()}</td>
                    <td>{fmt(w.wait_time_ms, 0)}</td>
                    <td>{fmt(w.pct_wait_time, 1)}%</td>
                  </tr>
                ))}
            </tbody>
          </table>
        </div>
        <div className="obs-card">
          <h3>Blocking queries</h3>
          {blocking.length === 0 ? (
            <p className="muted">No blockers</p>
          ) : (
            <table className="dd-table compact">
              <thead>
                <tr>
                  <th>Blocked</th>
                  <th>Blocking</th>
                  <th>Wait</th>
                  <th>Duration</th>
                </tr>
              </thead>
              <tbody>
                {blocking.map((b, i) => (
                  <tr key={i}>
                    <td>
                      pid {b.blocked_pid}
                      <div className="sql-cell muted">{b.blocked_sql}</div>
                    </td>
                    <td>
                      pid {b.blocking_pid}
                      <div className="sql-cell muted">{b.blocking_sql}</div>
                    </td>
                    <td>{b.wait_event}</td>
                    <td>{fmt(b.duration_ms, 0)}ms</td>
                  </tr>
                ))}
              </tbody>
            </table>
          )}
        </div>
      </div>

      <div className="obs-card" style={{ marginTop: "0.75rem" }}>
        <h3>Live activity</h3>
        <table className="dd-table compact">
          <thead>
            <tr>
              <th>DB</th>
              <th>PID</th>
              <th>User / App</th>
              <th>Client</th>
              <th>State</th>
              <th>Wait</th>
              <th>Duration</th>
              <th>Query</th>
            </tr>
          </thead>
          <tbody>
            {activity
              .filter((a) => dbFilter === "all" || a.db === dbFilter)
              .map((a) => (
                <tr key={`${a.db}:${a.pid}`}>
                  <td>{a.db}</td>
                  <td>{a.pid}</td>
                  <td>
                    {a.user}
                    <div className="muted">{a.application}</div>
                  </td>
                  <td className="muted">{a.client_addr || "—"}</td>
                  <td>{a.state}</td>
                  <td>
                    {a.wait_event_type ? `${a.wait_event_type}: ` : ""}
                    {a.wait_event || "—"}
                  </td>
                  <td>{fmt(a.duration_ms, 1)}ms</td>
                  <td className="sql-cell">
                    <code>{a.query}</code>
                  </td>
                </tr>
              ))}
          </tbody>
        </table>
      </div>

      <div className="obs-card" style={{ marginTop: "0.75rem" }}>
        <h3>Query samples</h3>
        <p className="muted">Recent executions with call volume — DBM sample collection</p>
        <table className="dd-table">
          <thead>
            <tr>
              <th>SQL</th>
              <th>DB</th>
              <th>Duration</th>
              <th>Calls</th>
            </tr>
          </thead>
          <tbody>
            {(dbFilter === "all" ? queries : queries.filter((q) => q.db === dbFilter)).map((q, i) => (
              <tr key={i}>
                <td className="sql-cell">
                  <code>{q.sql}</code>
                </td>
                <td>{q.db}</td>
                <td>{fmt(q.duration_ms, 0)}ms</td>
                <td>{q.calls.toLocaleString()}</td>
              </tr>
            ))}
          </tbody>
        </table>
      </div>

      {apm && (
        <div className="obs-card" style={{ marginTop: "0.75rem" }}>
          <h3>DBM ↔ APM correlation</h3>
          <p className="muted">
            Mode {String(apm.mode ?? "—")} · services:{" "}
            {Array.isArray(apm.calling_services) ? (apm.calling_services as string[]).join(", ") : "—"}
          </p>
          <table className="dd-table compact">
            <thead>
              <tr>
                <th>SQL</th>
                <th>DB</th>
                <th>Duration</th>
                <th>Calls</th>
                <th>Linked service</th>
              </tr>
            </thead>
            <tbody>
              {apmSamples.map((s, i) => (
                <tr key={i}>
                  <td className="sql-cell">
                    <code>{s.sql}</code>
                  </td>
                  <td>{s.db}</td>
                  <td>{fmt(s.duration_ms, 0)}ms</td>
                  <td>{s.calls.toLocaleString()}</td>
                  <td>
                    <code>{s.linked_service ?? "—"}</code>
                  </td>
                </tr>
              ))}
            </tbody>
          </table>
        </div>
      )}

      <div className="gpu-chart-card" style={{ marginTop: "0.75rem" }}>
        <header>
          <h3>
            All metrics live — {focus?.name ?? "—"} ({catalog.length} supported)
          </h3>
          <p>
            Every postgresql.* / mysql.* field with current value
            {focus ? ` · ${focus.engine} ${focus.version} · ${focus.role}` : ""}
          </p>
        </header>
        <div className="dd-toolbar" style={{ marginBottom: "0.5rem" }}>
          <select value={metricGroup} onChange={(e) => setMetricGroup(e.target.value)}>
            <option value="all">All groups</option>
            {[...new Set(catalog.map((m) => m.group))].map((g) => (
              <option key={g} value={g}>
                {g}
              </option>
            ))}
          </select>
          <span className="muted">{catalog.length} metrics · filter engine-aware</span>
        </div>
        {groups.map(([group, metrics]) => (
          <div key={group} className="gpu-metric-group">
            <h4>{group}</h4>
            <table className="dd-table compact">
              <thead>
                <tr>
                  <th>Metric</th>
                  <th>Live value</th>
                  <th>Unit</th>
                  <th>Engines</th>
                  <th>Description</th>
                </tr>
              </thead>
              <tbody>
                {metrics.map((m) => {
                  const targets =
                    dbFilter === "all"
                      ? active.filter((d) => m.engines.includes(d.engine))
                      : active.filter((d) => m.engines.includes(d.engine));
                  const primary = targets[0] ?? focus;
                  const live = primary ? instanceValue(primary, m.name, queryMetrics) : null;
                  return (
                    <tr key={m.name}>
                      <td>
                        <code>{m.name}</code>
                      </td>
                      <td className="gpu-live-val">{formatLive(m.unit, live)}</td>
                      <td>{m.unit}</td>
                      <td>{m.engines.join(", ")}</td>
                      <td className="muted">{m.description}</td>
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
