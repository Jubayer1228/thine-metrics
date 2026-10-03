import { useCallback, useEffect, useMemo, useState } from "react";
import {
  Area,
  AreaChart,
  Bar,
  BarChart,
  CartesianGrid,
  Legend,
  Line,
  LineChart,
  ResponsiveContainer,
  Tooltip,
  XAxis,
  YAxis,
} from "recharts";
import {
  api,
  type AlertRule,
  type BoardMeta,
  type Dashboard,
  type MetricMeta,
  type MetricSummaryRow,
  type QueryResult,
  type RenderedBoard,
} from "./api";
import { fmt, Sparkline, WidgetCard } from "./widgets";
import "./App.css";

const SERIES_COLORS = ["#5B91EB", "#A371E3", "#FF6B6B", "#F4A261", "#2EC4B6", "#E9C46A", "#F77FBE", "#4CC9F0"];
const TIME_RANGES = [
  { id: "15m", label: "Past 15 Minutes", ms: 15 * 60_000 },
  { id: "1h", label: "Past 1 Hour", ms: 60 * 60_000 },
  { id: "4h", label: "Past 4 Hours", ms: 4 * 60 * 60_000 },
  { id: "1d", label: "Past 1 Day", ms: 24 * 60 * 60_000 },
] as const;

type Page = "explorer" | "summary" | "dashboards" | "monitors";
type Viz = "line" | "area" | "bars";
type LayoutMode = "timeseries" | "split";

function tagStr(tags: Record<string, string>) {
  const pairs = Object.entries(tags).filter(([k]) => !k.startsWith("telemetry."));
  return pairs.length ? pairs.map(([k, v]) => `${k}:${v}`).join(",") : "*";
}

function stepForRange(ms: number) {
  if (ms <= 15 * 60_000) return 10_000;
  if (ms <= 60 * 60_000) return 30_000;
  if (ms <= 4 * 60 * 60_000) return 60_000;
  return 5 * 60_000;
}

function timeLabel(ms: number, rangeMs: number) {
  const d = new Date(ms);
  if (rangeMs >= 24 * 60 * 60_000) {
    return d.toLocaleString([], { month: "short", day: "numeric", hour: "2-digit", minute: "2-digit" });
  }
  return d.toLocaleTimeString([], { hour: "2-digit", minute: "2-digit", second: "2-digit" });
}

const tip = {
  background: "#1C2333",
  border: "1px solid rgba(255,255,255,0.1)",
  borderRadius: 6,
  color: "#E8EDF5",
  fontSize: 12,
};

export default function App() {
  const [page, setPage] = useState<Page>("dashboards");
  const [connected, setConnected] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [dash, setDash] = useState<Dashboard | null>(null);
  const [metrics, setMetrics] = useState<MetricMeta[]>([]);
  const [alerts, setAlerts] = useState<AlertRule[]>([]);
  const [boards, setBoards] = useState<BoardMeta[]>([]);
  const [rendered, setRendered] = useState<RenderedBoard | null>(null);
  const [activeBoardId, setActiveBoardId] = useState<string | null>(null);
  const [summaryRows, setSummaryRows] = useState<MetricSummaryRow[]>([]);
  const [envFilter, setEnvFilter] = useState("prod");

  const [metric, setMetric] = useState("http.server.duration");
  const [agg, setAgg] = useState("avg");
  const [fromTags, setFromTags] = useState("*");
  const [groupBy, setGroupBy] = useState("service");
  const [rangeId, setRangeId] = useState<(typeof TIME_RANGES)[number]["id"]>("1h");
  const [paused, setPaused] = useState(false);
  const [viz, setViz] = useState<Viz>("line");
  const [layout, setLayout] = useState<LayoutMode>("timeseries");
  const [series, setSeries] = useState<QueryResult[]>([]);
  const [metricSearch, setMetricSearch] = useState("");
  const [summarySearch, setSummarySearch] = useState("");
  const [monitorPreview, setMonitorPreview] = useState<QueryResult[]>([]);

  const range = TIME_RANGES.find((t) => t.id === rangeId) ?? TIME_RANGES[1];
  const uniqueMetrics = useMemo(() => Array.from(new Set(metrics.map((m) => m.name))).sort(), [metrics]);

  const refreshMeta = useCallback(async () => {
    try {
      await api.health();
      const [d, m, a, b] = await Promise.all([
        api.dashboard(),
        api.metrics(),
        api.alerts(),
        api.boards(),
      ]);
      setDash(d);
      setMetrics(m);
      setAlerts(a);
      setBoards(b);
      if (!activeBoardId && b.length) setActiveBoardId(b[0].id);
      setConnected(true);
      setError(null);
    } catch (e) {
      setConnected(false);
      setError(e instanceof Error ? e.message : "Connection failed");
    }
  }, [activeBoardId]);

  const refreshBoard = useCallback(async () => {
    if (!activeBoardId) return;
    try {
      const board = await api.renderBoard(activeBoardId, range.ms, `env:${envFilter}`);
      setRendered(board);
      setConnected(true);
    } catch (e) {
      setError(e instanceof Error ? e.message : "Board render failed");
    }
  }, [activeBoardId, range.ms, envFilter]);

  const refreshSummary = useCallback(async () => {
    try {
      setSummaryRows(await api.metricsSummary(range.ms));
    } catch (e) {
      setError(e instanceof Error ? e.message : "Summary failed");
    }
  }, [range.ms]);

  const refreshQuery = useCallback(async () => {
    if (!metric) return;
    try {
      const end = Date.now();
      const tags = fromTags.trim() === "*" || !fromTags.trim() ? undefined : fromTags.trim();
      const results = await api.query({
        metric,
        tags,
        aggregation: agg,
        start_ms: end - range.ms,
        end_ms: end,
        step_ms: stepForRange(range.ms),
        group_by: groupBy || undefined,
      });
      setSeries(results.filter((r) => r.points.length > 0));
      setConnected(true);
    } catch (e) {
      setConnected(false);
      setError(e instanceof Error ? e.message : "Query failed");
    }
  }, [metric, fromTags, agg, range.ms, groupBy]);

  useEffect(() => {
    refreshMeta();
    const id = setInterval(refreshMeta, 5000);
    return () => clearInterval(id);
  }, [refreshMeta]);

  useEffect(() => {
    if (page === "dashboards") {
      refreshBoard();
      if (!paused) {
        const id = setInterval(refreshBoard, 8000);
        return () => clearInterval(id);
      }
    }
  }, [page, refreshBoard, paused]);

  useEffect(() => {
    if (page === "summary") {
      refreshSummary();
      if (!paused) {
        const id = setInterval(refreshSummary, 5000);
        return () => clearInterval(id);
      }
    }
  }, [page, refreshSummary, paused]);

  useEffect(() => {
    if (page === "explorer") {
      refreshQuery();
      if (!paused) {
        const id = setInterval(refreshQuery, 3000);
        return () => clearInterval(id);
      }
    }
  }, [page, refreshQuery, paused]);

  useEffect(() => {
    if (page !== "monitors") return;
    const end = Date.now();
    api
      .query({
        metric,
        tags: `env:${envFilter}`,
        aggregation: "avg",
        start_ms: end - range.ms,
        end_ms: end,
        step_ms: stepForRange(range.ms),
        group_by: "service",
      })
      .then((r) => setMonitorPreview(r.filter((s) => s.points.length)))
      .catch(() => setMonitorPreview([]));
  }, [page, metric, envFilter, range.ms]);

  const keys = series.map((s, i) => tagStr(s.tags) || `s${i}`);
  const chartData = useMemo(() => {
    const map = new Map<number, Record<string, number | string>>();
    series.forEach((s, i) => {
      const key = keys[i];
      s.points.forEach((p) => {
        const row = map.get(p.timestamp_ms) ?? { t: p.timestamp_ms, label: timeLabel(p.timestamp_ms, range.ms) };
        row[key] = p.value;
        map.set(p.timestamp_ms, row);
      });
    });
    return Array.from(map.values()).sort((a, b) => Number(a.t) - Number(b.t));
  }, [series, keys, range.ms]);

  const filteredMetricNames = uniqueMetrics.filter((n) => n.toLowerCase().includes(metricSearch.toLowerCase()));
  const filteredSummary = summaryRows.filter((r) => r.name.toLowerCase().includes(summarySearch.toLowerCase()));
  const queryPreview = `${agg}:${metric}{${fromTags.trim() || "*"}}${groupBy ? ` by {${groupBy}}` : ""}`;

  return (
    <div className="dd">
      <aside className="dd-nav">
        <div className="dd-logo">
          <span className="dd-mark">T</span>
          <div>
            <strong>Thine</strong>
            <small>Metrics</small>
          </div>
        </div>
        <nav>
          {(
            [
              ["dashboards", "Dashboards"],
              ["explorer", "Metrics Explorer"],
              ["summary", "Metrics Summary"],
              ["monitors", "Monitors"],
            ] as const
          ).map(([id, label]) => (
            <button key={id} className={page === id ? "active" : ""} onClick={() => setPage(id)}>
              {label}
            </button>
          ))}
        </nav>
        <div className="dd-nav-foot">
          <span className={`dot ${connected ? "ok" : "bad"}`} />
          {connected ? "Connected" : "Disconnected"}
          <small>{dash ? `${dash.series_count} series · ${fmt(dash.ingest_rate_per_sec, 1)}/s` : "—"}</small>
        </div>
      </aside>

      <div className="dd-main">
        <header className="dd-top">
          <div>
            <h1>
              {page === "dashboards" && (rendered?.name || "Dashboards")}
              {page === "explorer" && "Metrics Explorer"}
              {page === "summary" && "Metrics Summary"}
              {page === "monitors" && "Monitors"}
            </h1>
            <p className="dd-sub">
              {page === "dashboards" &&
                (rendered?.description || "Grid of query values, timeseries, and toplists")}
              {page === "explorer" && "Graph, filter, and split metrics — Datadog-style explorer"}
              {page === "summary" && "Granular metric stats with sparklines (min / avg / max / last)"}
              {page === "monitors" && "Threshold monitors with live metric preview"}
            </p>
          </div>
          <div className="dd-top-actions">
            {page === "dashboards" && (
              <>
                <select value={activeBoardId ?? ""} onChange={(e) => setActiveBoardId(e.target.value)}>
                  {boards.map((b) => (
                    <option key={b.id} value={b.id}>
                      {b.name}
                    </option>
                  ))}
                </select>
                <select value={envFilter} onChange={(e) => setEnvFilter(e.target.value)}>
                  <option value="prod">env:prod</option>
                  <option value="staging">env:staging</option>
                </select>
              </>
            )}
            <select value={rangeId} onChange={(e) => setRangeId(e.target.value as typeof rangeId)}>
              {TIME_RANGES.map((t) => (
                <option key={t.id} value={t.id}>
                  {t.label}
                </option>
              ))}
            </select>
            <button className={paused ? "ghost on" : "ghost"} onClick={() => setPaused((p) => !p)}>
              {paused ? "Paused" : "Live"}
            </button>
          </div>
        </header>

        {error && (
          <div className="dd-banner">
            <strong>Connection failed</strong>
            <span>{error}</span>
            <code>./scripts/dev.sh</code>
            <button onClick={() => { refreshMeta(); refreshBoard(); refreshQuery(); }}>Retry</button>
          </div>
        )}

        {page === "dashboards" && (
          <section className="dd-panel dash-panel">
            <div className="dd-filter-bar">
              <span>Filter by:</span>
              <code>env:{envFilter}</code>
              <span className="muted">
                {rendered ? `${rendered.widgets.length} widgets · refreshed live` : "Loading…"}
              </span>
            </div>
            <div className="dash-grid">
              {rendered?.widgets.map((w) => (
                <WidgetCard key={w.id} w={w} />
              ))}
              {!rendered && <div className="dd-empty">Loading dashboard…</div>}
            </div>
          </section>
        )}

        {page === "summary" && (
          <section className="dd-panel">
            <div className="dd-toolbar">
              <input
                placeholder="Search metrics"
                value={summarySearch}
                onChange={(e) => setSummarySearch(e.target.value)}
              />
              <span>{filteredSummary.length} metrics</span>
            </div>
            <table className="dd-table">
              <thead>
                <tr>
                  <th>Metric Name</th>
                  <th>Type</th>
                  <th>Series</th>
                  <th>Last</th>
                  <th>Avg</th>
                  <th>Min</th>
                  <th>Max</th>
                  <th>Sparkline</th>
                  <th>Tag keys</th>
                </tr>
              </thead>
              <tbody>
                {filteredSummary.map((r) => (
                  <tr
                    key={r.name}
                    onClick={() => {
                      setMetric(r.name);
                      setPage("explorer");
                    }}
                  >
                    <td>
                      <button className="linkish">{r.name}</button>
                    </td>
                    <td>{r.metric_type}</td>
                    <td>{r.series_count}</td>
                    <td>{fmt(r.last_value)}</td>
                    <td>{fmt(r.avg)}</td>
                    <td>{fmt(r.min)}</td>
                    <td>{fmt(r.max)}</td>
                    <td className="spark-cell">
                      <Sparkline points={r.sparkline} />
                    </td>
                    <td className="tags">{r.tag_keys.join(", ")}</td>
                  </tr>
                ))}
              </tbody>
            </table>
          </section>
        )}

        {page === "monitors" && (
          <section className="dd-panel">
            <div className="monitor-layout">
              <div className="dd-monitor-form">
                <h3>New monitor</h3>
                <p>
                  Alert when <code>{metric}</code> window average exceeds threshold
                </p>
                <MonitorCreate
                  metric={metric}
                  metrics={uniqueMetrics}
                  onMetric={setMetric}
                  onCreate={async (name, threshold) => {
                    await api.createAlert({
                      name,
                      metric,
                      threshold,
                      comparator: "gt",
                      window_ms: 60_000,
                      tags: { env: envFilter },
                    });
                    await refreshMeta();
                  }}
                />
              </div>
              <div className="dw timeseries monitor-preview">
                <div className="dw-title">
                  Preview · {metric}
                  <span>threshold monitor context</span>
                </div>
                <div className="dw-chart">
                  <ResponsiveContainer width="100%" height="100%">
                    <LineChart
                      data={(() => {
                        const map = new Map<number, Record<string, number | string>>();
                        monitorPreview.forEach((s, i) => {
                          const key = tagStr(s.tags) || `s${i}`;
                          s.points.forEach((p) => {
                            const row = map.get(p.timestamp_ms) ?? {
                              t: p.timestamp_ms,
                              label: timeLabel(p.timestamp_ms, range.ms),
                            };
                            row[key] = p.value;
                            map.set(p.timestamp_ms, row);
                          });
                        });
                        return Array.from(map.values()).sort((a, b) => Number(a.t) - Number(b.t));
                      })()}
                      margin={{ top: 8, right: 8, left: 0, bottom: 0 }}
                    >
                      <CartesianGrid stroke="rgba(255,255,255,0.06)" vertical={false} />
                      <XAxis dataKey="label" tick={{ fill: "#8B97A8", fontSize: 10 }} minTickGap={28} />
                      <YAxis tick={{ fill: "#8B97A8", fontSize: 10 }} width={42} />
                      <Tooltip contentStyle={tip} />
                      {monitorPreview.map((s, i) => (
                        <Line
                          key={tagStr(s.tags) || i}
                          type="monotone"
                          dataKey={tagStr(s.tags) || `s${i}`}
                          stroke={SERIES_COLORS[i % SERIES_COLORS.length]}
                          strokeWidth={2}
                          dot={false}
                          isAnimationActive={false}
                        />
                      ))}
                    </LineChart>
                  </ResponsiveContainer>
                </div>
              </div>
            </div>
            <table className="dd-table">
              <thead>
                <tr>
                  <th>Name</th>
                  <th>Metric</th>
                  <th>Condition</th>
                  <th>Window</th>
                  <th>Status</th>
                </tr>
              </thead>
              <tbody>
                {alerts.map((a) => (
                  <tr key={a.id}>
                    <td>{a.name}</td>
                    <td>
                      <code>{a.metric}</code>
                    </td>
                    <td>
                      {a.comparator} {a.threshold}
                    </td>
                    <td>{a.window_ms / 1000}s</td>
                    <td>{a.enabled ? "Enabled" : "Muted"}</td>
                  </tr>
                ))}
                {!alerts.length && (
                  <tr>
                    <td colSpan={5} className="tags">
                      No monitors yet
                    </td>
                  </tr>
                )}
              </tbody>
            </table>
          </section>
        )}

        {page === "explorer" && (
          <section className="dd-explorer">
            <div className="dd-query">
              <div className="dd-query-row">
                <label>A</label>
                <select value={agg} onChange={(e) => setAgg(e.target.value)}>
                  <option value="avg">avg by</option>
                  <option value="sum">sum by</option>
                  <option value="min">min by</option>
                  <option value="max">max by</option>
                  <option value="last">last</option>
                </select>
                <div className="dd-metric-pick">
                  <input list="metric-names" value={metric} onChange={(e) => setMetric(e.target.value)} />
                  <datalist id="metric-names">
                    {uniqueMetrics.map((n) => (
                      <option key={n} value={n} />
                    ))}
                  </datalist>
                </div>
                <span className="dd-from">from</span>
                <input className="dd-tags" value={fromTags} onChange={(e) => setFromTags(e.target.value)} />
                <span className="dd-from">by</span>
                <select value={groupBy} onChange={(e) => setGroupBy(e.target.value)}>
                  <option value="">(none)</option>
                  <option value="service">service</option>
                  <option value="env">env</option>
                </select>
              </div>
              <div className="dd-query-meta">
                <code>{queryPreview}</code>
                <div className="dd-viz-toggles">
                  {(["line", "area", "bars"] as Viz[]).map((v) => (
                    <button key={v} className={viz === v ? "on" : ""} onClick={() => setViz(v)}>
                      {v}
                    </button>
                  ))}
                  <button
                    className={layout === "split" ? "on" : ""}
                    onClick={() => setLayout((l) => (l === "split" ? "timeseries" : "split"))}
                  >
                    Split Graph
                  </button>
                </div>
              </div>
            </div>

            <div className="dd-body">
              <aside className="dd-metrics-rail">
                <input
                  placeholder="Filter metrics"
                  value={metricSearch}
                  onChange={(e) => setMetricSearch(e.target.value)}
                />
                <div className="dd-metric-list">
                  {filteredMetricNames.map((n) => (
                    <button key={n} className={n === metric ? "active" : ""} onClick={() => setMetric(n)}>
                      {n}
                    </button>
                  ))}
                </div>
              </aside>

              <div className="dd-graph-wrap">
                {layout === "timeseries" ? (
                  <div className="dd-graph-card">
                    <div className="dd-graph-head">
                      <strong>{metric}</strong>
                      <span>
                        {series.length} series · {chartData.length} points · {range.label}
                      </span>
                    </div>
                    <div className="dd-graph">
                      {chartData.length === 0 ? (
                        <div className="dd-empty">No data for this query.</div>
                      ) : (
                        <ResponsiveContainer width="100%" height="100%">
                          {viz === "bars" ? (
                            <BarChart data={chartData} margin={{ top: 12, right: 16, left: 8, bottom: 8 }}>
                              <CartesianGrid stroke="rgba(255,255,255,0.06)" vertical={false} />
                              <XAxis dataKey="label" tick={{ fill: "#8B97A8", fontSize: 11 }} minTickGap={36} />
                              <YAxis tick={{ fill: "#8B97A8", fontSize: 11 }} width={54} />
                              <Tooltip contentStyle={tip} />
                              <Legend />
                              {keys.map((k, i) => (
                                <Bar key={k} dataKey={k} fill={SERIES_COLORS[i % SERIES_COLORS.length]} />
                              ))}
                            </BarChart>
                          ) : viz === "area" ? (
                            <AreaChart data={chartData} margin={{ top: 12, right: 16, left: 8, bottom: 8 }}>
                              <CartesianGrid stroke="rgba(255,255,255,0.06)" vertical={false} />
                              <XAxis dataKey="label" tick={{ fill: "#8B97A8", fontSize: 11 }} minTickGap={36} />
                              <YAxis tick={{ fill: "#8B97A8", fontSize: 11 }} width={54} />
                              <Tooltip contentStyle={tip} />
                              <Legend />
                              {keys.map((k, i) => (
                                <Area
                                  key={k}
                                  type="monotone"
                                  dataKey={k}
                                  stroke={SERIES_COLORS[i % SERIES_COLORS.length]}
                                  fill={SERIES_COLORS[i % SERIES_COLORS.length]}
                                  fillOpacity={0.18}
                                  strokeWidth={2}
                                  dot={false}
                                  isAnimationActive={false}
                                />
                              ))}
                            </AreaChart>
                          ) : (
                            <LineChart data={chartData} margin={{ top: 12, right: 16, left: 8, bottom: 8 }}>
                              <CartesianGrid stroke="rgba(255,255,255,0.06)" vertical={false} />
                              <XAxis dataKey="label" tick={{ fill: "#8B97A8", fontSize: 11 }} minTickGap={36} />
                              <YAxis tick={{ fill: "#8B97A8", fontSize: 11 }} width={54} />
                              <Tooltip contentStyle={tip} />
                              <Legend />
                              {keys.map((k, i) => (
                                <Line
                                  key={k}
                                  type="monotone"
                                  dataKey={k}
                                  stroke={SERIES_COLORS[i % SERIES_COLORS.length]}
                                  strokeWidth={2}
                                  dot={false}
                                  isAnimationActive={false}
                                />
                              ))}
                            </LineChart>
                          )}
                        </ResponsiveContainer>
                      )}
                    </div>
                    <div className="dd-legend-table">
                      {series.map((s, i) => {
                        const last = s.points.at(-1)?.value;
                        const avg = s.points.reduce((a, p) => a + p.value, 0) / Math.max(1, s.points.length);
                        return (
                          <div key={keys[i]} className="dd-legend-row">
                            <i style={{ background: SERIES_COLORS[i % SERIES_COLORS.length] }} />
                            <code>{tagStr(s.tags)}</code>
                            <span>avg {fmt(avg)}</span>
                            <span>last {fmt(last)}</span>
                          </div>
                        );
                      })}
                    </div>
                  </div>
                ) : (
                  <div className="dd-split-grid">
                    {series.map((s, i) => {
                      const key = keys[i];
                      const data = s.points.map((p) => ({
                        label: timeLabel(p.timestamp_ms, range.ms),
                        value: p.value,
                      }));
                      return (
                        <div key={key} className="dd-graph-card split">
                          <div className="dd-graph-head">
                            <strong>{tagStr(s.tags)}</strong>
                            <span>{fmt(s.points.at(-1)?.value)}</span>
                          </div>
                          <div className="dd-graph short">
                            <ResponsiveContainer width="100%" height="100%">
                              <LineChart data={data} margin={{ top: 8, right: 8, left: 0, bottom: 0 }}>
                                <Line
                                  type="monotone"
                                  dataKey="value"
                                  stroke={SERIES_COLORS[i % SERIES_COLORS.length]}
                                  strokeWidth={2}
                                  dot={false}
                                  isAnimationActive={false}
                                />
                                <YAxis tick={{ fill: "#8B97A8", fontSize: 10 }} width={40} />
                              </LineChart>
                            </ResponsiveContainer>
                          </div>
                        </div>
                      );
                    })}
                  </div>
                )}
              </div>
            </div>
          </section>
        )}
      </div>
    </div>
  );
}

function MonitorCreate({
  metric,
  metrics,
  onMetric,
  onCreate,
}: {
  metric: string;
  metrics: string[];
  onMetric: (m: string) => void;
  onCreate: (name: string, threshold: number) => Promise<void>;
}) {
  const [name, setName] = useState("High latency");
  const [threshold, setThreshold] = useState("120");
  return (
    <div className="row">
      <select value={metric} onChange={(e) => onMetric(e.target.value)}>
        {metrics.map((m) => (
          <option key={m} value={m}>
            {m}
          </option>
        ))}
      </select>
      <input value={name} onChange={(e) => setName(e.target.value)} />
      <input value={threshold} onChange={(e) => setThreshold(e.target.value)} />
      <button onClick={() => onCreate(name, Number(threshold))}>Create Monitor</button>
    </div>
  );
}
