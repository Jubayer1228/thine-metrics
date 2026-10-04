import { useCallback, useEffect, useMemo, useState } from "react";
import { setAuthHeaderProvider } from "./api";
import { useAuth } from "./auth";
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
} from "./api";
import { BitsPage } from "./BitsPage";
import { ComparePage } from "./Compare";
import { DashboardsPage, type DashNavAction } from "./Dashboards";
import { DocsPage } from "./Docs";
import { FleetPage } from "./Fleet";
import { GetStartedPage } from "./GetStarted";
import { IntegrationsPage } from "./Integrations";
import { MetricsCorrelationsPage } from "./MetricsCorrelations";
import { MonitorsPage } from "./Monitors";
import { NotebooksPage } from "./Notebooks";
import { ObservabilityPage } from "./Observability";
import { SideNav, type AppPage, type ObsDeepLink } from "./SideNav";
import { ThemeToggle, chartTip } from "./theme";
import { fmt, Sparkline } from "./widgets";
import "./App.css";

const SERIES_COLORS = ["#5B91EB", "#A371E3", "#FF6B6B", "#F4A261", "#2EC4B6", "#E9C46A", "#F77FBE", "#4CC9F0"];
const TIME_RANGES = [
  { id: "15m", label: "Past 15 Minutes", ms: 15 * 60_000 },
  { id: "1h", label: "Past 1 Hour", ms: 60 * 60_000 },
  { id: "4h", label: "Past 4 Hours", ms: 4 * 60 * 60_000 },
  { id: "1d", label: "Past 1 Day", ms: 24 * 60 * 60_000 },
] as const;

type Page = AppPage;
type Viz = "line" | "area" | "bars";
type LayoutMode = "timeseries" | "split";

/** Deep-link map for left-nav + Observability tabs — Datadog-style `#route` URLs. */
const HASH_ROUTES: Record<string, { page: Page; obsTab?: ObsDeepLink }> = {
  "get-started": { page: "get-started" },
  dashboards: { page: "dashboards" },
  monitors: { page: "monitors" },
  explorer: { page: "explorer" },
  summary: { page: "summary" },
  correlations: { page: "correlations" },
  notebooks: { page: "notebooks" },
  integrations: { page: "integrations" },
  compare: { page: "compare" },
  fleet: { page: "fleet" },
  bits: { page: "bits" },
  docs: { page: "docs" },
  watchdog: { page: "observability", obsTab: "ha" },
  "ha-watchdog": { page: "observability", obsTab: "ha-watchdog" },
  events: { page: "observability", obsTab: "events" },
  apm: { page: "observability", obsTab: "apm" },
  "apm-traces": { page: "observability", obsTab: "apm-traces" },
  "apm-map": { page: "observability", obsTab: "apm-map" },
  "apm-catalog": { page: "observability", obsTab: "apm-catalog" },
  logs: { page: "observability", obsTab: "logs" },
  "logs-live": { page: "observability", obsTab: "logs-live" },
  "logs-errors": { page: "observability", obsTab: "logs-errors" },
  "logs-sds": { page: "observability", obsTab: "logs-sds" },
  "logs-patterns": { page: "observability", obsTab: "logs-patterns" },
  usm: { page: "observability", obsTab: "usm" },
  security: { page: "observability", obsTab: "usm" },
  ai: { page: "observability", obsTab: "ai" },
  ux: { page: "observability", obsTab: "ux" },
  infra: { page: "observability", obsTab: "infra" },
  hostmap: { page: "observability", obsTab: "hostmap" },
  containers: { page: "observability", obsTab: "containers" },
  "container-map": { page: "observability", obsTab: "container-map" },
  "infra-envs": { page: "observability", obsTab: "infra-envs" },
  "infra-k8s": { page: "observability", obsTab: "infra-k8s" },
  processes: { page: "observability", obsTab: "processes" },
  serverless: { page: "observability", obsTab: "serverless" },
  gpu: { page: "observability", obsTab: "gpu" },
  dbm: { page: "observability", obsTab: "dbm" },
  cloudcraft: { page: "observability", obsTab: "cloudcraft" },
  slos: { page: "observability", obsTab: "slos" },
  data: { page: "observability", obsTab: "data" },
  cost: { page: "observability", obsTab: "cost" },
  ha: { page: "observability", obsTab: "ha" },
  observability: { page: "observability", obsTab: "overview" },
  overview: { page: "observability", obsTab: "overview" },
};

function hashFor(page: Page, obsTab: ObsDeepLink | null): string {
  if (page === "observability" && obsTab) return `#${obsTab}`;
  if (page === "observability") return "#overview";
  return `#${page}`;
}

function parseHash(): { page: Page; obsTab: ObsDeepLink | null } | null {
  const raw = (typeof window !== "undefined" ? window.location.hash : "").replace(/^#/, "").trim();
  if (!raw) return null;
  const key = raw.split("?")[0].split("/")[0];
  const hit = HASH_ROUTES[key];
  if (!hit) return null;
  return { page: hit.page, obsTab: hit.obsTab ?? null };
}

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

const tip = chartTip;

export default function App() {
  const { session, authHeaders } = useAuth();
  useEffect(() => {
    setAuthHeaderProvider(authHeaders);
  }, [authHeaders]);

  const initialRoute = parseHash();
  const [page, setPageState] = useState<Page>(initialRoute?.page ?? "get-started");
  const [connected, setConnected] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [dash, setDash] = useState<Dashboard | null>(null);
  const [metrics, setMetrics] = useState<MetricMeta[]>([]);
  const [alerts, setAlerts] = useState<AlertRule[]>([]);
  const [boards, setBoards] = useState<BoardMeta[]>([]);
  const [activeBoardId, setActiveBoardId] = useState<string | null>(null);
  const [summaryRows, setSummaryRows] = useState<MetricSummaryRow[]>([]);
  const [obsTab, setObsTabState] = useState<ObsDeepLink | null>(initialRoute?.obsTab ?? "overview");
  const [bitsPrompt, setBitsPrompt] = useState<string | null>(null);
  const [notebookFocusId, setNotebookFocusId] = useState<string | null>(null);
  const [dashNavAction, setDashNavAction] = useState<DashNavAction | null>(null);
  const [dashNavSeq, setDashNavSeq] = useState(0);
  const [activeDashListId, setActiveDashListId] = useState<string | null>(null);
  const [alertEvents, setAlertEvents] = useState<import("./api").AlertEvent[]>([]);

  const setPage = useCallback((p: Page) => {
    setPageState(p);
    if (p !== "observability") {
      const next = hashFor(p, null);
      if (window.location.hash !== next) window.location.hash = next;
    }
  }, []);
  const setObsTab = useCallback((tab: ObsDeepLink) => {
    setObsTabState(tab);
    setPageState("observability");
    const next = hashFor("observability", tab);
    if (window.location.hash !== next) window.location.hash = next;
  }, []);

  useEffect(() => {
    const apply = () => {
      const r = parseHash();
      if (!r) return;
      setPageState(r.page);
      if (r.obsTab) setObsTabState(r.obsTab);
    };
    window.addEventListener("hashchange", apply);
    return () => window.removeEventListener("hashchange", apply);
  }, []);

  const handleDashAction = useCallback((action: DashNavAction) => {
    setPage("dashboards");
    setDashNavAction(action);
    setDashNavSeq((n) => n + 1);
  }, [setPage]);

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
    api.alertEvents().then(setAlertEvents).catch(() => setAlertEvents([]));
  }, [page]);

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
    <div className={`dd${page === "dashboards" || page === "observability" || page === "explorer" || page === "summary" || page === "correlations" || page === "monitors" || page === "compare" || page === "get-started" || page === "fleet" || page === "bits" ? " dd-has-subnav" : ""}`}>
      <SideNav
        page={page}
        connected={connected}
        seriesLabel={dash ? `${dash.series_count} series · ${fmt(dash.ingest_rate_per_sec, 1)}/s` : "—"}
        boardCount={boards.length}
        onPage={setPage}
        obsTab={obsTab}
        onObsTab={setObsTab}
        onDashAction={handleDashAction}
        onBitsPrompt={(prompt) => {
          setBitsPrompt(prompt);
          setPage("bits");
        }}
        activeDashListId={activeDashListId}
      />

      <div className="dd-main">
        <header className="dd-top">
          <div>
            <h1>
              {page === "get-started" && "Welcome"}
              {page === "fleet" && "Fleet Automation"}
              {page === "bits" && "Bits AI"}
              {page === "dashboards" && "Dashboards"}
              {page === "observability" &&
                (obsTab === "hostmap"
                  ? "Host Map"
                  : obsTab === "containers"
                    ? "Containers Explorer"
                    : obsTab === "infra-envs"
                      ? "Environments"
                      : obsTab === "infra-k8s"
                        ? "Kubernetes utilization"
                        : obsTab === "processes"
                          ? "Live Processes"
                          : obsTab === "serverless"
                            ? "Serverless"
                            : obsTab === "infra"
                              ? "Infrastructure"
                              : "Observability")}
              {page === "compare" && "Compare"}
              {page === "integrations" && "Integrations"}
              {page === "notebooks" && "Notebooks"}
              {page === "explorer" && "Metrics Explorer"}
              {page === "summary" && "Metrics Summary"}
              {page === "correlations" && "Metric Correlations"}
              {page === "monitors" && "Monitors"}
              {page === "docs" && "Docs"}
            </h1>
            <p className="dd-sub">
              {page === "get-started" &&
                "Install your first agent, then build dashboards and monitors — Datadog tutorial path"}
              {page === "fleet" &&
                "View, install, configure, and upgrade agents across hosts and container platforms"}
              {page === "bits" &&
                "Agentic tutorial runner — install agents, Golden Signals boards, monitors with recovery"}
              {page === "dashboards" &&
                "Datadog-parity boards — layouts, template variables, widgets, functions, overlays, sharing"}
              {page === "observability" &&
                (obsTab === "hostmap"
                  ? "Honeycomb host map — filter, group, fill by CPU, size by metric"
                  : obsTab === "containers"
                    ? "Real-time containers — facets, Boolean search, RSS/CPU vs limits, live logs"
                    : obsTab === "infra-envs"
                      ? "Unified env/service/version tagging across hosts and containers"
                      : obsTab === "infra-k8s"
                        ? "Kubernetes CPU/memory usage vs requests and limits"
                        : obsTab === "processes"
                          ? "Real-time process CPU/RSS — faceted search, scatter by command group"
                          : obsTab === "serverless"
                            ? "Lambda, Azure App Service, Cloud Run — cold starts, cost, enhanced metrics"
                            : obsTab === "infra"
                              ? "Host Map, Containers Explorer, environments, and K8s utilization"
                              : "Traces, service map, DBM, AI evals, Bits RCA — unified beyond Datadog + LangSmith silos")}
              {page === "compare" &&
                "Thine vs SigNoz, Grafana, Datadog, New Relic, CloudWatch, ClickStack, Dash0"}
              {page === "integrations" &&
                "Catalog, marketplace, SLOs, incidents, DORA — wired to live platform APIs"}
              {page === "notebooks" &&
                "Executable RCA notebooks — metric cells hit the live query engine"}
              {page === "explorer" && "Graph, filter, and split metrics — Datadog-style explorer"}
              {page === "summary" && "Granular metric stats with sparklines (min / avg / max / last)"}
              {page === "correlations" && "Find metrics with irregular behavior in the same window — Graph Insights parity"}
              {page === "monitors" &&
                "Metric threshold wizard — recovery threshold, severity routing, notifications"}
              {page === "docs" && "How to use each feature against the live Thine API"}
            </p>
          </div>
          <div className="dd-top-actions">
            {session ? (
              <span className="org-badge" title={session.org_id}>
                {session.org_name}
              </span>
            ) : null}
            <ThemeToggle />
            <button type="button" className="ghost" onClick={() => setPage("get-started")}>
              Get Started
            </button>
            <button type="button" className="ghost" onClick={() => setPage("bits")}>
              Bits AI
            </button>
            {page !== "docs" && page !== "observability" && page !== "compare" && page !== "get-started" && page !== "fleet" && page !== "bits" && (
              <select value={rangeId} onChange={(e) => setRangeId(e.target.value as typeof rangeId)}>
                {TIME_RANGES.map((t) => (
                  <option key={t.id} value={t.id}>
                    {t.label}
                  </option>
                ))}
              </select>
            )}
            {page !== "docs" && page !== "compare" && page !== "get-started" && (
              <button className={paused ? "ghost on" : "ghost"} onClick={() => setPaused((p) => !p)}>
                {paused ? "Paused" : "Live"}
              </button>
            )}
          </div>
        </header>

        {error && (
          <div className="dd-banner">
            <strong>Connection failed</strong>
            <span>{error}</span>
            <code>./scripts/dev.sh</code>
            <button onClick={() => { refreshMeta(); refreshQuery(); }}>Retry</button>
          </div>
        )}

        {page === "observability" && <ObservabilityPage initialTab={obsTab ?? "overview"} />}
        {page === "compare" && <ComparePage />}
        {page === "integrations" && <IntegrationsPage />}
        {page === "notebooks" && <NotebooksPage initialNotebookId={notebookFocusId} />}
        {page === "docs" && <DocsPage />}
        {page === "get-started" && (
          <GetStartedPage
            onInstallAgent={() => setPage("fleet")}
            onOpenDashboards={() => {
              setPage("dashboards");
              handleDashAction({ type: "open-list", preset: "all" });
            }}
          />
        )}
        {page === "fleet" && <FleetPage initialTab="install" />}
        {page === "bits" && (
          <BitsPage
            initialPrompt={bitsPrompt}
            onBoardsChanged={() => {
              void refreshMeta();
            }}
            onNavigate={(target) => {
              if (target.page === "dashboards") {
                if (target.boardId) setActiveBoardId(target.boardId);
                setPage("dashboards");
                handleDashAction({ type: "open-board" });
                void refreshMeta();
              } else if (target.page === "fleet") {
                setPage("fleet");
              } else if (target.page === "monitors") {
                setPage("monitors");
                void refreshMeta();
              } else if (target.page === "get-started") {
                setPage("get-started");
              } else if (target.page === "observability") {
                setPage("observability");
              } else if (target.page === "notebooks") {
                setNotebookFocusId(target.notebookId ?? null);
                setPage("notebooks");
              }
            }}
          />
        )}

        {page === "dashboards" && (
          <DashboardsPage
            boards={boards}
            activeBoardId={activeBoardId}
            onSelectBoard={setActiveBoardId}
            onBoardsChanged={() => { void refreshMeta(); }}
            rangeMs={range.ms}
            paused={paused}
            navAction={dashNavAction}
            navActionSeq={dashNavSeq}
            onActiveListChange={setActiveDashListId}
          />
        )}

        {page === "correlations" && (
          <MetricsCorrelationsPage
            metrics={uniqueMetrics}
            rangeMs={range.ms}
            initialMetric={metric}
          />
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
          <MonitorsPage
            metrics={uniqueMetrics}
            metric={metric}
            onMetric={setMetric}
            alerts={alerts}
            alertEvents={alertEvents}
            rangeMs={range.ms}
            onCreated={() => {
              void refreshMeta();
              api.alertEvents().then(setAlertEvents).catch(() => setAlertEvents([]));
            }}
          />
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

