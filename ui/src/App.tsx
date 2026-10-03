import { useEffect, useMemo, useState } from "react";
import {
  Area,
  AreaChart,
  CartesianGrid,
  ResponsiveContainer,
  Tooltip,
  XAxis,
  YAxis,
} from "recharts";
import { api, type AlertRule, type Dashboard, type MetricMeta, type QueryResult } from "./api";
import "./App.css";

const AGGS = ["avg", "sum", "min", "max", "last"] as const;

function fmt(n: number | undefined | null, digits = 2) {
  if (n == null || Number.isNaN(n)) return "—";
  if (Math.abs(n) >= 1000) return n.toLocaleString(undefined, { maximumFractionDigits: 1 });
  return n.toFixed(digits);
}

function tagLine(tags: Record<string, string>) {
  return Object.entries(tags)
    .filter(([k]) => !k.startsWith("telemetry."))
    .map(([k, v]) => `${k}:${v}`)
    .join(" · ");
}

function timeLabel(ms: number) {
  return new Date(ms).toLocaleTimeString([], { hour: "2-digit", minute: "2-digit", second: "2-digit" });
}

export default function App() {
  const [dash, setDash] = useState<Dashboard | null>(null);
  const [metrics, setMetrics] = useState<MetricMeta[]>([]);
  const [selected, setSelected] = useState("http.server.duration");
  const [service, setService] = useState("api");
  const [env, setEnv] = useState("prod");
  const [agg, setAgg] = useState<(typeof AGGS)[number]>("avg");
  const [series, setSeries] = useState<QueryResult[]>([]);
  const [alerts, setAlerts] = useState<AlertRule[]>([]);
  const [filter, setFilter] = useState("");
  const [error, setError] = useState<string | null>(null);
  const [alertName, setAlertName] = useState("Latency spike");
  const [alertThreshold, setAlertThreshold] = useState("120");

  const refresh = async () => {
    try {
      const [d, m, a] = await Promise.all([api.dashboard(), api.metrics(), api.alerts()]);
      setDash(d);
      setMetrics(m);
      setAlerts(a);
      setError(null);
    } catch (e) {
      setError(e instanceof Error ? e.message : "Failed to load");
    }
  };

  const refreshQuery = async () => {
    try {
      const end = Date.now();
      const results = await api.query({
        metric: selected,
        service,
        env,
        aggregation: agg,
        start_ms: end - 15 * 60 * 1000,
        end_ms: end,
        step_ms: 15_000,
      });
      setSeries(results);
    } catch (e) {
      setError(e instanceof Error ? e.message : "Query failed");
    }
  };

  useEffect(() => {
    refresh();
    const id = setInterval(refresh, 4000);
    return () => clearInterval(id);
  }, []);

  useEffect(() => {
    refreshQuery();
    const id = setInterval(refreshQuery, 4000);
    return () => clearInterval(id);
  }, [selected, service, env, agg]);

  const chartData = useMemo(() => {
    const primary = series[0];
    if (!primary) return [];
    return primary.points.map((p) => ({
      t: p.timestamp_ms,
      label: timeLabel(p.timestamp_ms),
      value: p.value,
    }));
  }, [series]);

  const filtered = metrics.filter((m) => {
    const hay = `${m.name} ${tagLine(m.tags)}`.toLowerCase();
    return hay.includes(filter.toLowerCase());
  });

  const uniqueNames = useMemo(
    () => Array.from(new Set(metrics.map((m) => m.name))).sort(),
    [metrics],
  );

  const createAlert = async () => {
    await api.createAlert({
      name: alertName,
      metric: selected,
      threshold: Number(alertThreshold),
      comparator: "gt",
      window_ms: 60_000,
    });
    await refresh();
  };

  const ribbonIntensity = Math.min(1, (dash?.ingest_rate_per_sec ?? 0) / 8);

  return (
    <div className="app">
      <div
        className="signal-ribbon"
        style={{ ["--pulse" as string]: String(0.35 + ribbonIntensity * 0.65) }}
        aria-hidden
      />

      <header className="topbar">
        <div className="brand-block">
          <div className="brand">Thine</div>
          <p className="brand-sub">Metrics that stay under your roof</p>
        </div>
        <div className="status-pills">
          <span className="pill live">
            <i /> Live ingest {fmt(dash?.ingest_rate_per_sec, 1)}/s
          </span>
          <span className="pill">{dash?.series_count ?? 0} series</span>
          <span className="pill">{dash?.active_alerts ?? 0} alerts</span>
        </div>
      </header>

      <main className="layout">
        <aside className="rail">
          <label className="field">
            <span>Find metrics</span>
            <input
              value={filter}
              onChange={(e) => setFilter(e.target.value)}
              placeholder="cpu, http, memory…"
            />
          </label>
          <div className="metric-list">
            {filtered.slice(0, 80).map((m) => {
              const key = `${m.name}|${tagLine(m.tags)}`;
              const active = m.name === selected;
              return (
                <button
                  key={key}
                  className={`metric-row ${active ? "active" : ""}`}
                  onClick={() => {
                    setSelected(m.name);
                    if (m.tags.service) setService(m.tags.service);
                    if (m.tags.env) setEnv(m.tags.env);
                  }}
                >
                  <strong>{m.name}</strong>
                  <small>{tagLine(m.tags) || "untagged"}</small>
                  <em>{fmt(m.last_value)}</em>
                </button>
              );
            })}
          </div>
        </aside>

        <section className="stage">
          <div className="stage-head">
            <div>
              <h1>{selected}</h1>
              <p>
                {service}/{env} · {agg} over last 15 minutes
              </p>
            </div>
            <div className="controls">
              <select value={selected} onChange={(e) => setSelected(e.target.value)}>
                {uniqueNames.map((n) => (
                  <option key={n} value={n}>
                    {n}
                  </option>
                ))}
              </select>
              <select value={service} onChange={(e) => setService(e.target.value)}>
                {["api", "worker", "ingest"].map((s) => (
                  <option key={s} value={s}>
                    {s}
                  </option>
                ))}
              </select>
              <select value={env} onChange={(e) => setEnv(e.target.value)}>
                {["prod", "staging"].map((s) => (
                  <option key={s} value={s}>
                    {s}
                  </option>
                ))}
              </select>
              <select value={agg} onChange={(e) => setAgg(e.target.value as (typeof AGGS)[number])}>
                {AGGS.map((a) => (
                  <option key={a} value={a}>
                    {a}
                  </option>
                ))}
              </select>
            </div>
          </div>

          <div className="chart-panel">
            <ResponsiveContainer width="100%" height={340}>
              <AreaChart data={chartData} margin={{ top: 10, right: 12, left: 0, bottom: 0 }}>
                <defs>
                  <linearGradient id="signalFill" x1="0" y1="0" x2="0" y2="1">
                    <stop offset="0%" stopColor="#0F766E" stopOpacity={0.35} />
                    <stop offset="100%" stopColor="#0F766E" stopOpacity={0.02} />
                  </linearGradient>
                </defs>
                <CartesianGrid stroke="rgba(20,32,51,0.08)" vertical={false} />
                <XAxis dataKey="label" tick={{ fill: "#64748B", fontSize: 11 }} minTickGap={40} />
                <YAxis tick={{ fill: "#64748B", fontSize: 11 }} width={56} />
                <Tooltip
                  contentStyle={{
                    background: "#142033",
                    border: "none",
                    borderRadius: 8,
                    color: "#F2F5F8",
                    fontFamily: "IBM Plex Mono, monospace",
                  }}
                  formatter={(v) => [fmt(Number(v)), agg]}
                />
                <Area
                  type="monotone"
                  dataKey="value"
                  stroke="#0F766E"
                  strokeWidth={2.25}
                  fill="url(#signalFill)"
                  isAnimationActive={false}
                />
              </AreaChart>
            </ResponsiveContainer>
            {chartData.length === 0 && <div className="empty">No points in this window yet.</div>}
          </div>

          <div className="stat-strip">
            <article>
              <span>Samples</span>
              <strong>{fmt(dash?.sample_count, 0)}</strong>
            </article>
            <article>
              <span>Uptime</span>
              <strong>{fmt(dash?.uptime_secs, 0)}s</strong>
            </article>
            <article>
              <span>Series matching</span>
              <strong>{series.filter((s) => s.points.length).length}</strong>
            </article>
            <article>
              <span>Latest</span>
              <strong>{fmt(chartData.at(-1)?.value)}</strong>
            </article>
          </div>

          <div className="lower">
            <div className="cardish">
              <h2>Threshold alert</h2>
              <p>Fire when the window average crosses your line.</p>
              <div className="alert-form">
                <input value={alertName} onChange={(e) => setAlertName(e.target.value)} />
                <input
                  value={alertThreshold}
                  onChange={(e) => setAlertThreshold(e.target.value)}
                  inputMode="decimal"
                />
                <button onClick={createAlert}>Create alert</button>
              </div>
              <ul className="alert-list">
                {alerts.length === 0 && <li className="muted">No rules yet.</li>}
                {alerts.map((a) => (
                  <li key={a.id}>
                    <strong>{a.name}</strong>
                    <span>
                      {a.metric} {a.comparator} {a.threshold}
                    </span>
                  </li>
                ))}
              </ul>
            </div>

            <div className="cardish plug">
              <h2>Plug in OpenTelemetry</h2>
              <p>Point any OTLP exporter at this host — Go, Python, Collector, or agent.</p>
              <pre>{`OTEL_EXPORTER_OTLP_ENDPOINT=http://localhost:4318
OTEL_EXPORTER_OTLP_PROTOCOL=http/json
OTEL_EXPORTER_OTLP_METRICS_ENDPOINT=http://localhost:4318/v1/metrics`}</pre>
              <div className="sdk-links">
                <code>sdks/python</code>
                <code>sdks/go</code>
                <code>examples/</code>
              </div>
            </div>
          </div>

          {error && <div className="error-banner">{error}</div>}
        </section>
      </main>
    </div>
  );
}
