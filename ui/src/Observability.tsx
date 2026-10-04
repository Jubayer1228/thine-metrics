import { useCallback, useEffect, useMemo, useState } from "react";
import {
  Bar,
  BarChart,
  CartesianGrid,
  Cell,
  Line,
  LineChart,
  ResponsiveContainer,
  Tooltip,
  XAxis,
  YAxis,
} from "recharts";
import { ApmServicePage } from "./ApmServicePage";
import { BitsAssist } from "./BitsAssist";
import { SoftwareCatalogPage } from "./SoftwareCatalog";
import { TraceExplorer } from "./TraceExplorer";
import {
  api,
  type ApmStat,
  type CloudcraftDiagram,
  type CostRec,
  type DataLineage,
  type DbActivity,
  type DbBlocking,
  type DbHostSample,
  type DbInstance,
  type DbMetricDef,
  type DbQuery,
  type DbQueryMetric,
  type DbSchema,
  type DbWaitEvent,
  type ExplainPlan,
  type HaStatus,
  type LiveProcess,
  type LogEvent,
  type LogPattern,
  type WatchdogAnomaly,
  type StreamInfo,
  type UsmaRed,
  type GpuSummary,
  type GpuProcess,
  type GpuSample,
  type AiProject,
  type AiRun,
  type AiHostSample,
  type AiExperiment,
  type AiEvalResult,
  type AiGrader,
  type AiDataset,
  type AiFeedback,
  type AiMetricDef,
  type PlatformEvent,
  type AlertEvent,
  type SloEntry,
  type UxOverview,
  type UxSession,
  type UxVital,
  type UxSynthetic,
  type UxTimeseriesPoint,
  type UxPage,
  type UxFunnelStep,
} from "./api";
import { chartTip } from "./theme";
import { fmt } from "./widgets";
import { DbPanel } from "./DbPanel";
import { AiPanel } from "./AiPanel";
import { GpuPanel } from "./GpuPanel";
import { InfraPanel } from "./InfraPanel";
import { ProcessesExplorer } from "./ProcessesExplorer";
import { ServerlessPanel } from "./ServerlessPanel";
import type { ObsDeepLink } from "./SideNav";
import { obsBaseTab } from "./SideNav";

type ObsTab = ObsDeepLink;

function Sig({ title, why }: { title: string; why: string }) {
  return (
    <div className="obs-sig">
      <strong>{title}</strong>
      <span>{why}</span>
    </div>
  );
}

const TABS: { id: ObsTab; label: string }[] = [
  { id: "overview", label: "Overview" },
  { id: "cloudcraft", label: "Cloudcraft" },
  { id: "usm", label: "USM / eBPF" },
  { id: "apm", label: "APM" },
  { id: "dbm", label: "Database" },
  { id: "ai", label: "AI / Agents" },
  { id: "hostmap", label: "Host Map" },
  { id: "container-map", label: "Container Map" },
  { id: "containers", label: "Containers" },
  { id: "infra-envs", label: "Environments" },
  { id: "infra-k8s", label: "K8s Util" },
  { id: "processes", label: "Processes" },
  { id: "serverless", label: "Serverless" },
  { id: "infra", label: "Infra Overview" },
  { id: "gpu", label: "GPU" },
  { id: "logs", label: "Logs & Errors" },
  { id: "events", label: "Events" },
  { id: "ux", label: "UX / RUM" },
  { id: "data", label: "Streams & Lineage" },
  { id: "cost", label: "Cloud Cost" },
  { id: "slos", label: "SLOs" },
  { id: "ha", label: "HA / Watchdog" },
];

const tip = chartTip;

const STATUS_COLOR: Record<string, string> = {
  ok: "#2ec4b6",
  warn: "#f4a261",
  alert: "#f25f5c",
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

export function ObservabilityPage({ initialTab = "overview" }: { initialTab?: ObsTab }) {
  const [tab, setTab] = useState<ObsTab>(initialTab);

  useEffect(() => {
    setTab(initialTab);
  }, [initialTab]);

  const baseTab = obsBaseTab(tab);
  const apmMode =
    tab === "apm-traces" ? "traces" : tab === "apm-map" ? "map" : tab === "apm-catalog" ? "catalog" : "services";
  const logMode =
    tab === "logs-live"
      ? "live"
      : tab === "logs-errors"
        ? "errors"
        : tab === "logs-sds"
          ? "sds"
          : tab === "logs-patterns"
            ? "patterns"
            : "explorer";
  const haMode = tab === "ha-watchdog" ? "watchdog" : "ha";
  const liveTail = tab === "logs-live";
  const [apmService, setApmService] = useState<string | null>(null);
  const [logQuery, setLogQuery] = useState("");
  const [logQueryDraft, setLogQueryDraft] = useState("");
  const [logSampleRate, setLogSampleRate] = useState(1);
  const [liveWsStatus, setLiveWsStatus] = useState<"off" | "connecting" | "live" | "error">("off");
  const [logPatterns, setLogPatterns] = useState<LogPattern[]>([]);
  const [pipelines, setPipelines] = useState<{ id: string; name: string; processors: string[] }[]>([]);
  const [pipelineResult, setPipelineResult] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [stats, setStats] = useState<{
    total: number;
    done: number;
    avg_parity_pct: number;
  } | null>(null);
  const [diagram, setDiagram] = useState<CloudcraftDiagram | null>(null);
  const [overlay, setOverlay] = useState("observability");
  const [groupBy, setGroupBy] = useState("service");
  const [usm, setUsm] = useState<UsmaRed[]>([]);
  const [ebpf, setEbpf] = useState<Record<string, unknown> | null>(null);
  const [apm, setApm] = useState<ApmStat[]>([]);
  const [explain, setExplain] = useState<ExplainPlan[]>([]);
  const [schemas, setSchemas] = useState<DbSchema[]>([]);
  const [queries, setQueries] = useState<DbQuery[]>([]);
  const [dbInstances, setDbInstances] = useState<DbInstance[]>([]);
  const [dbSummary, setDbSummary] = useState<Record<string, unknown> | null>(null);
  const [dbSamples, setDbSamples] = useState<DbHostSample[]>([]);
  const [dbQueryMetrics, setDbQueryMetrics] = useState<DbQueryMetric[]>([]);
  const [dbWaits, setDbWaits] = useState<DbWaitEvent[]>([]);
  const [dbBlocking, setDbBlocking] = useState<DbBlocking[]>([]);
  const [dbActivity, setDbActivity] = useState<DbActivity[]>([]);
  const [dbHealth, setDbHealth] = useState<{
    findings?: { db: string; severity: string; finding: string }[];
    blocking?: number;
  } | null>(null);
  const [dbCatalog, setDbCatalog] = useState<DbMetricDef[]>([]);
  const [dbApm, setDbApm] = useState<Record<string, unknown> | null>(null);
  const [aiSummary, setAiSummary] = useState<Record<string, unknown> | null>(null);
  const [aiProjects, setAiProjects] = useState<AiProject[]>([]);
  const [aiRuns, setAiRuns] = useState<AiRun[]>([]);
  const [aiSamples, setAiSamples] = useState<AiHostSample[]>([]);
  const [aiExperiments, setAiExperiments] = useState<AiExperiment[]>([]);
  const [aiEvals, setAiEvals] = useState<AiEvalResult[]>([]);
  const [aiGraders, setAiGraders] = useState<AiGrader[]>([]);
  const [aiDatasets, setAiDatasets] = useState<AiDataset[]>([]);
  const [aiFeedback, setAiFeedback] = useState<AiFeedback[]>([]);
  const [aiCatalog, setAiCatalog] = useState<AiMetricDef[]>([]);
  const [aiHealth, setAiHealth] = useState<{
    findings?: { project: string; severity: string; finding: string }[];
  } | null>(null);
  const [procs, setProcs] = useState<LiveProcess[]>([]);
  const [logs, setLogs] = useState<LogEvent[]>([]);
  const [errors, setErrors] = useState<
    {
      id: string;
      service: string;
      message: string;
      count: number;
      status?: string;
      assignee?: string | null;
      stack_frames?: string[];
      linked_trace_id?: string | null;
      linked_issues?: string[];
    }[]
  >([]);
  const [sds, setSds] = useState<{ rule_id: string; service: string; snippet: string }[]>([]);
  const [streams, setStreams] = useState<StreamInfo[]>([]);
  const [lineage, setLineage] = useState<DataLineage | null>(null);
  const [costRecs, setCostRecs] = useState<CostRec[]>([]);
  const [costDetail, setCostDetail] = useState<{ service: string; compute: number; storage: number; network: number; total: number }[]>([]);
  const [ha, setHa] = useState<HaStatus | null>(null);
  const [watchdog, setWatchdog] = useState<Record<string, unknown> | null>(null);
  const [watchdogAnoms, setWatchdogAnoms] = useState<WatchdogAnomaly[]>([]);
  const [workers, setWorkers] = useState<Record<string, unknown> | null>(null);
  const [scanning, setScanning] = useState(false);
  const [logFacets, setLogFacets] = useState<{ key: string; values: [string, number][] }[]>([]);
  const [logLevel, setLogLevel] = useState<string>("all");
  const [logService, setLogService] = useState<string>("all");
  const [discovering, setDiscovering] = useState(false);
  const [intakeArch, setIntakeArch] = useState<Record<string, unknown> | null>(null);
  const [gpuSummary, setGpuSummary] = useState<GpuSummary | null>(null);
  const [gpuProcs, setGpuProcs] = useState<GpuProcess[]>([]);
  const [gpuSamples, setGpuSamples] = useState<GpuSample[]>([]);
  const [gpuHealth, setGpuHealth] = useState<{
    devices_checked: number;
    healthy: number;
    degraded: number;
    findings: { gpu_id: string; severity: string; finding: string }[];
  } | null>(null);
  const [gpuCatalog, setGpuCatalog] = useState<
    { name: string; dcgm_field: string; unit: string; description: string; group: string }[]
  >([]);
  const [events, setEvents] = useState<PlatformEvent[]>([]);
  const [alertEvents, setAlertEvents] = useState<AlertEvent[]>([]);
  const [slos, setSlos] = useState<SloEntry[]>([]);
  const [uxOverview, setUxOverview] = useState<UxOverview | null>(null);
  const [uxSessions, setUxSessions] = useState<UxSession[]>([]);
  const [uxVitals, setUxVitals] = useState<UxVital[]>([]);
  const [uxSynthetics, setUxSynthetics] = useState<UxSynthetic[]>([]);
  const [uxTimeseries, setUxTimeseries] = useState<UxTimeseriesPoint[]>([]);
  const [uxPages, setUxPages] = useState<UxPage[]>([]);
  const [uxFunnel, setUxFunnel] = useState<UxFunnelStep[]>([]);
  const [eventSource, setEventSource] = useState<string>("all");
  const [eventImpact, setEventImpact] = useState<string>("all");

  const load = useCallback(async () => {
    try {
      const [
        st,
        diag,
        red,
        eb,
        ap,
        ex,
        sc,
        q,
        pr,
        lg,
        er,
        sd,
        sm,
        ln,
        cr,
        cd,
        hsStatus,
        wd,
        pw,
        gs,
        gp,
        gsam,
        gh,
        gc,
        lf,
        wa,
        arch,
        ev,
        aev,
        slo,
        uxo,
        uxs,
        uxv,
        uxsynt,
        uxts,
        uxp,
        uxf,
      ] = await Promise.all([
        api.featuresStats(),
        api.cloudcraftDiagram({ overlay, group_by: groupBy, provider: "aws" }),
        api.usmRed(),
        api.usmEbpf(),
        api.apmStats(),
        api.dbmExplain(),
        api.dbmSchema(),
        api.dbmQueries(),
        api.liveProcesses(),
        api.logsSearch({
          limit: 80,
          q: logQuery || undefined,
          service: logService !== "all" ? logService : undefined,
          level: logLevel !== "all" ? logLevel : undefined,
          live: liveTail,
        }),
        api.errors(),
        api.sdsScan(),
        api.streams(),
        api.dataLineage(),
        api.costRecommendations(),
        api.costDetail(),
        api.haStatus(),
        api.watchdogSummary(),
        api.pipelineWorkers(),
        api.gpuSummary(),
        api.gpuProcesses(),
        api.gpuSamples(undefined, 80),
        api.gpuHealth(),
        api.gpuMetricsCatalog(),
        api.logFacets(),
        api.watchdogAnomalies(),
        api.ingestArchitecture(),
        api.events(120),
        api.alertEvents(),
        api.slos(),
        api.uxOverview(),
        api.uxSessions(),
        api.uxVitals(),
        api.uxSynthetics(),
        api.uxTimeseries(),
        api.uxPages(),
        api.uxFunnel(),
      ]);
      setStats(st);
      setDiagram(diag);
      setUsm(red);
      setEbpf(eb);
      setApm(ap);
      setExplain(ex);
      setSchemas(sc);
      setQueries(q);
      setProcs(pr);
      setLogs(lg.logs);
      setLogSampleRate(lg.sample_rate ?? 1);
      setErrors(er);
      setSds(sd);
      setStreams(sm);
      setLineage(ln);
      setCostRecs(cr);
      setCostDetail(cd);
      setHa(hsStatus);
      setWatchdog(wd);
      setWorkers(pw);
      setGpuSummary(gs);
      setGpuProcs(gp);
      setGpuSamples(gsam);
      setGpuHealth(gh);
      setGpuCatalog(gc.metrics);
      setLogFacets(lf);
      setWatchdogAnoms(wa);
      setIntakeArch(arch);
      setEvents(ev);
      setAlertEvents(aev);
      setSlos(slo);
      setUxOverview(uxo);
      setUxSessions(uxs);
      setUxVitals(uxv);
      setUxSynthetics(uxsynt);
      setUxTimeseries(uxts);
      setUxPages(uxp);
      setUxFunnel(uxf);
      setError(null);
    } catch (e) {
      setError(e instanceof Error ? e.message : "Failed to load observability data");
    }
  }, [overlay, groupBy, logQuery, logService, logLevel, liveTail]);

  const loadDbm = useCallback(async () => {
    try {
      const [inst, sum, sam, qm, waits, blk, act, health, cat, apmLink] = await Promise.all([
        api.dbmInstances(),
        api.dbmSummary(),
        api.dbmSamples(undefined, 180),
        api.dbmQueryMetrics(50),
        api.dbmWaits(),
        api.dbmBlocking(),
        api.dbmActivity(50),
        api.dbmHealth(),
        api.dbmMetricsCatalog(),
        api.dbmApm(),
      ]);
      setDbInstances(inst);
      setDbSummary(sum);
      setDbSamples(sam);
      setDbQueryMetrics(qm);
      setDbWaits(waits);
      setDbBlocking(blk);
      setDbActivity(act);
      setDbHealth(health);
      setDbCatalog(cat.metrics);
      setDbApm(apmLink);
    } catch {
      /* keep prior */
    }
  }, []);

  useEffect(() => {
    load();
    const id = setInterval(load, 8000);
    return () => clearInterval(id);
  }, [load]);

  useEffect(() => {
    if (tab !== "dbm") return;
    loadDbm();
    const id = setInterval(loadDbm, 8000);
    return () => clearInterval(id);
  }, [tab, loadDbm]);

  const loadAi = useCallback(async () => {
    try {
      const [sum, proj, runs, sam, exp, ev, gr, ds, fb, cat, health] = await Promise.all([
        api.aiSummary(),
        api.aiProjects(),
        api.aiRuns(undefined, undefined, 80),
        api.aiSamples(undefined, 200),
        api.aiExperiments(),
        api.aiEvals(),
        api.aiGraders(),
        api.aiDatasets(),
        api.aiFeedback(50),
        api.aiMetricsCatalog(),
        api.aiHealth(),
      ]);
      setAiSummary(sum);
      setAiProjects(proj);
      setAiRuns(runs);
      setAiSamples(sam);
      setAiExperiments(exp);
      setAiEvals(ev);
      setAiGraders(gr);
      setAiDatasets(ds);
      setAiFeedback(fb);
      setAiCatalog(cat.metrics);
      setAiHealth(health);
    } catch {
      /* keep prior */
    }
  }, []);

  useEffect(() => {
    if (tab !== "ai") return;
    loadAi();
    const id = setInterval(loadAi, 8000);
    return () => clearInterval(id);
  }, [tab, loadAi]);

  // Datadog Live Tail — WebSocket stream (not polled HTTP).
  useEffect(() => {
    if (!liveTail) {
      setLiveWsStatus("off");
      return;
    }
    setLiveWsStatus("connecting");
    const proto = window.location.protocol === "https:" ? "wss" : "ws";
    const host = window.location.host;
    const qs = new URLSearchParams();
    if (logQuery) qs.set("q", logQuery);
    if (logService !== "all") qs.set("service", logService);
    if (logLevel !== "all") qs.set("level", logLevel);
    qs.set("live", "true");
    qs.set("limit", "40");
    const ws = new WebSocket(`${proto}://${host}/api/v1/logs/tail/ws?${qs}`);
    ws.onopen = () => setLiveWsStatus("live");
    ws.onerror = () => setLiveWsStatus("error");
    ws.onclose = () => setLiveWsStatus((s) => (s === "live" ? "off" : s));
    ws.onmessage = (ev) => {
      try {
        const msg = JSON.parse(String(ev.data)) as {
          type?: string;
          sample_rate?: number;
          logs?: LogEvent[];
        };
        if (msg.type === "batch" && msg.logs?.length) {
          setLogSampleRate(msg.sample_rate ?? 1);
          setLogs((prev) => {
            const merged = [...msg.logs!, ...prev];
            const seen = new Set<string>();
            const out: LogEvent[] = [];
            for (const l of merged) {
              const k = `${l.timestamp_ms}:${l.service}:${l.message}`;
              if (seen.has(k)) continue;
              seen.add(k);
              out.push(l);
              if (out.length >= 120) break;
            }
            return out;
          });
        }
      } catch {
        /* ignore */
      }
    };
    const ping = setInterval(() => {
      if (ws.readyState === WebSocket.OPEN) ws.send(JSON.stringify({ type: "ping" }));
    }, 15_000);
    return () => {
      clearInterval(ping);
      ws.close();
    };
  }, [liveTail, logQuery, logService, logLevel]);

  useEffect(() => {
    if (logMode !== "patterns") return;
    void Promise.all([api.logPatterns(40), api.pipelines()])
      .then(([p, pipes]) => {
        setLogPatterns(p);
        setPipelines(pipes);
      })
      .catch(() => undefined);
  }, [logMode]);

  const savings = useMemo(
    () => costRecs.reduce((a, r) => a + r.savings_month, 0),
    [costRecs],
  );
  const usmRps = useMemo(() => usm.reduce((a, u) => a + u.requests_per_sec, 0), [usm]);
  const apmErrors = useMemo(() => apm.reduce((a, s) => a + s.error_count, 0), [apm]);
  const logErrors = useMemo(() => logs.filter((l) => l.level === "error").length, [logs]);

  const cloudCostChart = useMemo(() => {
    if (!diagram?.nodes) return [];
    return diagram.nodes
      .map((n) => ({
        name: n.name,
        cost: Number((n as { cost_month?: number }).cost_month ?? 0),
        kind: n.kind,
      }))
      .filter((n) => n.cost > 0)
      .sort((a, b) => b.cost - a.cost)
      .slice(0, 10);
  }, [diagram]);

  const runWatchdog = async () => {
    setScanning(true);
    try {
      await api.watchdogScan();
      const [sum, anoms] = await Promise.all([api.watchdogSummary(), api.watchdogAnomalies()]);
      setWatchdog(sum);
      setWatchdogAnoms(anoms);
    } catch (e) {
      setError(e instanceof Error ? e.message : "Watchdog scan failed");
    } finally {
      setScanning(false);
    }
  };

  const runDiscover = async () => {
    setDiscovering(true);
    try {
      await api.usmDiscover();
      setUsm(await api.usmRed());
      setEbpf(await api.usmEbpf());
    } catch (e) {
      setError(e instanceof Error ? e.message : "USM discover failed");
    } finally {
      setDiscovering(false);
    }
  };

  const filteredLogs = useMemo(() => {
    return logs.filter((l) => {
      if (logLevel !== "all" && l.level !== logLevel) return false;
      if (logService !== "all" && l.service !== logService) return false;
      return true;
    });
  }, [logs, logLevel, logService]);

  const filteredEvents = useMemo(() => {
    return events.filter((e) => {
      if (eventSource !== "all" && e.source !== eventSource) return false;
      if (eventImpact !== "all" && (e.tags?.impact || "low") !== eventImpact) return false;
      return true;
    });
  }, [events, eventSource, eventImpact]);

  const eventSources = useMemo(() => {
    const set = new Set(events.map((e) => e.source || "unknown"));
    return ["all", ...Array.from(set).sort()];
  }, [events]);

  const criticalEvents = useMemo(
    () => events.filter((e) => e.tags?.impact === "critical" || e.alert_type === "error").length,
    [events],
  );

  const topWatchdog = watchdogAnoms[0];
  const breachingSlos = useMemo(() => slos.filter((s) => s.status === "breaching"), [slos]);

  return (
    <section className="dd-panel obs-page">
      <div className="obs-tabs">
        {TABS.map((t) => (
          <button key={t.id} className={baseTab === t.id ? "on" : ""} onClick={() => setTab(t.id)}>
            {t.label}
          </button>
        ))}
      </div>

      {error && (
        <div className="dd-banner">
          <strong>Observability load failed</strong>
          <span>{error}</span>
          <button onClick={load}>Retry</button>
        </div>
      )}

      {tab === "overview" && (
        <div className="obs-section">
          <div className="obs-kpi-row">
            <Kpi label="Features done" value={stats ? `${stats.done}/${stats.total}` : "—"} hint="catalog parity" />
            <Kpi label="Avg parity" value={stats ? `${stats.avg_parity_pct.toFixed(0)}%` : "—"} hint="vs Datadog SaaS" />
            <Kpi label="Cloud resources" value={String(diagram?.resource_count ?? "—")} hint="Cloudcraft map" />
            <Kpi label="USM RPS" value={fmt(usmRps, 0)} hint="eBPF-discovered" />
            <Kpi label="APM errors" value={String(apmErrors)} hint="in window" />
            <Kpi label="Cost salvage" value={`$${fmt(savings, 0)}/mo`} hint="recommendations" />
          </div>
          <p className="obs-lead">
            Unified observability + AI evals in one plane — metrics, traces, DBM, GPU, and LangSmith-class experiments —
            with a Rust ingest edge. Demo seed keeps every surface lit for product walkthroughs.
          </p>
          <BitsAssist />
          {intakeArch && (
            <div className="obs-card" style={{ marginTop: "0.75rem" }}>
              <h3>Dual-path intake (Datadog model)</h3>
              <p className="muted">{String(intakeArch.description ?? "")}</p>
              <pre className="obs-pre intake-diagram">
                {Array.isArray(intakeArch.diagram)
                  ? (intakeArch.diagram as string[]).join("\n")
                  : ""}
              </pre>
              <div className="obs-kpi-row">
                <Kpi
                  label="Native metrics"
                  value={String((intakeArch.paths as { native?: { accepted?: { metrics?: number } } })?.native?.accepted?.metrics ?? 0)}
                />
                <Kpi
                  label="OTel metrics"
                  value={String((intakeArch.paths as { otel?: { accepted?: { metrics?: number } } })?.otel?.accepted?.metrics ?? 0)}
                />
                <Kpi
                  label="OTel traces"
                  value={String((intakeArch.paths as { otel?: { accepted?: { traces?: number } } })?.otel?.accepted?.traces ?? 0)}
                />
                <Kpi
                  label="OTel logs"
                  value={String((intakeArch.paths as { otel?: { accepted?: { logs?: number } } })?.otel?.accepted?.logs ?? 0)}
                />
              </div>
            </div>
          )}
          <div className="obs-grid-2" style={{ marginTop: "0.75rem" }}>
            <div className="obs-card">
              <h3>APM latency by service</h3>
              <div className="obs-chart">
                <ResponsiveContainer width="100%" height="100%">
                  <BarChart data={apm.map((s) => ({ name: s.service, p95: s.p95_ms, avg: s.avg_ms }))}>
                    <CartesianGrid stroke="rgba(255,255,255,0.06)" vertical={false} />
                    <XAxis dataKey="name" tick={{ fill: "#8B97A8", fontSize: 11 }} />
                    <YAxis tick={{ fill: "#8B97A8", fontSize: 11 }} width={40} />
                    <Tooltip contentStyle={tip} />
                    <Bar dataKey="p95" fill="#5B91EB" name="p95 ms" />
                    <Bar dataKey="avg" fill="#2EC4B6" name="avg ms" />
                  </BarChart>
                </ResponsiveContainer>
              </div>
            </div>
            <div className="obs-card">
              <h3>USM RED (hits / errors / duration)</h3>
              <table className="dd-table compact">
                <thead>
                  <tr>
                    <th>Service</th>
                    <th>RPS</th>
                    <th>Err%</th>
                    <th>p95</th>
                    <th>Detect</th>
                  </tr>
                </thead>
                <tbody>
                  {usm.slice(0, 8).map((u) => (
                    <tr key={`${u.service}-${u.protocol}`}>
                      <td>
                        <code>{u.service}</code>
                      </td>
                      <td>{fmt(u.requests_per_sec, 0)}</td>
                      <td>{(u.error_rate * 100).toFixed(1)}%</td>
                      <td>{fmt(u.duration_p95_ms, 0)}ms</td>
                      <td>{u.detected_by}</td>
                    </tr>
                  ))}
                  {!usm.length && (
                    <tr>
                      <td colSpan={5} className="tags">
                        No USM endpoints yet
                      </td>
                    </tr>
                  )}
                </tbody>
              </table>
            </div>
          </div>
        </div>
      )}

      {tab === "cloudcraft" && (
        <div className="obs-section">
          <div className="obs-toolbar">
            <label>
              Overlay
              <select value={overlay} onChange={(e) => setOverlay(e.target.value)}>
                {(diagram?.overlays_available ?? ["infrastructure", "observability", "cost", "security", "apm", "monitors"]).map(
                  (o) => (
                    <option key={o} value={o}>
                      {o}
                    </option>
                  ),
                )}
              </select>
            </label>
            <label>
              Group by
              <select value={groupBy} onChange={(e) => setGroupBy(e.target.value)}>
                <option value="service">service</option>
                <option value="kind">kind</option>
                <option value="region">region</option>
                <option value="vpc">vpc</option>
              </select>
            </label>
            <span className="muted">
              {diagram?.resource_count ?? 0} resources · {diagram?.edges?.length ?? 0} edges · {diagram?.groups?.length ?? 0}{" "}
              groups
            </span>
          </div>
          <div className="obs-kpi-row">
            <Kpi
              label="Instrumented"
              value={String(diagram?.nodes?.filter((n) => n.agent_installed).length ?? 0)}
              hint="agent_installed"
            />
            <Kpi
              label="Alerting"
              value={String(diagram?.nodes?.filter((n) => n.alert_status === "alert" || n.alert_status === "warn").length ?? 0)}
              hint="monitors overlay"
            />
            <Kpi
              label="Monthly spend"
              value={`$${fmt(
                diagram?.nodes?.reduce((a, n) => a + Number((n as { cost_month?: number }).cost_month ?? 0), 0) ?? 0,
                0,
              )}`}
              hint="cost overlay"
            />
          </div>
          {diagram && diagram.nodes.length > 0 && (
            <div className="obs-card cc-canvas" style={{ marginBottom: "0.75rem" }}>
              <h3>Diagram · {overlay}</h3>
              <svg viewBox="0 0 720 260" className="cc-svg">
                {(diagram.edges ?? []).map((e, i) => {
                  const a = diagram.nodes.findIndex((n) => n.id === e.from);
                  const b = diagram.nodes.findIndex((n) => n.id === e.to);
                  if (a < 0 || b < 0) return null;
                  const ax = 60 + (a % 6) * 110;
                  const ay = 40 + Math.floor(a / 6) * 70;
                  const bx = 60 + (b % 6) * 110;
                  const by = 40 + Math.floor(b / 6) * 70;
                  return (
                    <line
                      key={i}
                      x1={ax}
                      y1={ay}
                      x2={bx}
                      y2={by}
                      stroke="rgba(232,237,245,0.2)"
                      strokeWidth={1.5}
                    />
                  );
                })}
                {diagram.nodes.slice(0, 18).map((n, i) => {
                  const x = 60 + (i % 6) * 110;
                  const y = 40 + Math.floor(i / 6) * 70;
                  const color =
                    n.alert_status === "alert" ? "#F25F5C" : n.alert_status === "warn" ? "#F4A261" : n.agent_installed ? "#2EC4B6" : "#5B91EB";
                  return (
                    <g key={n.id}>
                      <rect x={x - 40} y={y - 18} width={80} height={36} rx={6} fill="#151c28" stroke={color} strokeWidth={1.5} />
                      <text x={x} y={y - 2} textAnchor="middle" fill="#E8EDF5" fontSize="9" fontWeight="700">
                        {n.name.slice(0, 12)}
                      </text>
                      <text x={x} y={y + 11} textAnchor="middle" fill="#8B97A8" fontSize="8">
                        {n.kind}
                      </text>
                    </g>
                  );
                })}
              </svg>
            </div>
          )}
          <div className="obs-grid-2">
            <div className="obs-card">
              <h3>Architecture groups</h3>
              <div className="cc-groups">
                {(diagram?.groups ?? []).map((g) => (
                  <div key={g.group} className="cc-group">
                    <strong>{g.group}</strong>
                    <span>{g.count} resources</span>
                    <code>{g.resource_ids.join(", ")}</code>
                  </div>
                ))}
              </div>
            </div>
            <div className="obs-card">
              <h3>Resource cost (top 10)</h3>
              <div className="obs-chart">
                <ResponsiveContainer width="100%" height="100%">
                  <BarChart data={cloudCostChart} layout="vertical" margin={{ left: 80 }}>
                    <CartesianGrid stroke="rgba(255,255,255,0.06)" horizontal={false} />
                    <XAxis type="number" tick={{ fill: "#8B97A8", fontSize: 10 }} />
                    <YAxis type="category" dataKey="name" tick={{ fill: "#8B97A8", fontSize: 10 }} width={78} />
                    <Tooltip contentStyle={tip} />
                    <Bar dataKey="cost" fill="#A371E3" name="$/mo">
                      {cloudCostChart.map((e) => (
                        <Cell key={e.name} fill={e.kind === "rds" ? "#F4A261" : "#A371E3"} />
                      ))}
                    </Bar>
                  </BarChart>
                </ResponsiveContainer>
              </div>
            </div>
          </div>
          <table className="dd-table">
            <thead>
              <tr>
                <th>Resource</th>
                <th>Kind</th>
                <th>Region</th>
                <th>Service</th>
                <th>Agent</th>
                <th>Alert</th>
                <th>Cost/mo</th>
              </tr>
            </thead>
            <tbody>
              {(diagram?.nodes ?? []).map((n) => (
                <tr key={n.id}>
                  <td>
                    <code>{n.name}</code>
                  </td>
                  <td>{n.kind}</td>
                  <td>{n.region}</td>
                  <td>{n.service ?? "—"}</td>
                  <td>{n.agent_installed ? "yes" : "no"}</td>
                  <td>
                    <span className="pill" style={{ background: STATUS_COLOR[n.alert_status] ?? "#8b97a8" }}>
                      {n.alert_status || "—"}
                    </span>
                  </td>
                  <td>${fmt((n as { cost_month?: number }).cost_month, 0)}</td>
                </tr>
              ))}
            </tbody>
          </table>
        </div>
      )}

      {tab === "usm" && (
        <div className="obs-section">
          <Sig
            title={`${usm.length} services discovered without instrumentation`}
            why={`${fmt(usmRps, 0)} RPS via eBPF — compare RED vs APM to find shadow traffic and unsigned services`}
          />
          <div className="obs-toolbar">
            <button type="button" className="ghost" disabled={discovering} onClick={() => void runDiscover()}>
              {discovering ? "Discovering…" : "Run USM discover"}
            </button>
            <span className="muted">Refresh eBPF / RED map from live traffic</span>
          </div>
          <div className="obs-kpi-row">
            <Kpi label="Endpoints" value={String(usm.length)} hint="protocol auto-detect" />
            <Kpi label="eBPF agents" value={String(ebpf?.agents ?? "—")} hint={String(ebpf?.mode ?? "")} />
            <Kpi label="Flows/sec" value={fmt(Number(ebpf?.total_flows_per_sec ?? 0), 0)} />
            <Kpi label="Healthy agents" value={String(ebpf?.healthy_agents ?? "—")} />
          </div>
          <div className="obs-card">
            <h3>Universal Service Monitoring — RED</h3>
            <p className="muted">No code changes — discovered from eBPF/network flows.</p>
            <table className="dd-table">
              <thead>
                <tr>
                  <th>Service</th>
                  <th>Protocol</th>
                  <th>Requests/s</th>
                  <th>Error rate</th>
                  <th>Duration p95</th>
                  <th>Metrics emitted</th>
                </tr>
              </thead>
              <tbody>
                {usm.map((u) => (
                  <tr key={`${u.service}-${u.protocol}`}>
                    <td>
                      <code>{u.service}</code>
                    </td>
                    <td>{u.protocol}</td>
                    <td>{fmt(u.requests_per_sec, 1)}</td>
                    <td>{(u.error_rate * 100).toFixed(2)}%</td>
                    <td>{fmt(u.duration_p95_ms, 1)} ms</td>
                    <td className="tags">{u.metric_names?.slice(0, 2).join(" · ")}</td>
                  </tr>
                ))}
              </tbody>
            </table>
          </div>
        </div>
      )}

      {baseTab === "apm" && (
        <div className="obs-section">
          {apmService ? (
            <ApmServicePage
              service={apmService}
              onBack={() => setApmService(null)}
              onOpenTraces={() => {
                setApmService(null);
                setTab("apm-traces");
              }}
            />
          ) : (
            <>
              <div className="obs-subtabs">
                {(
                  [
                    ["services", "apm", "Services"],
                    ["catalog", "apm-catalog", "Software Catalog"],
                    ["traces", "apm-traces", "Traces / Waterfall"],
                    ["map", "apm-map", "Service Map"],
                  ] as const
                ).map(([mode, id, label]) => (
                  <button key={id} type="button" className={apmMode === mode ? "on" : ""} onClick={() => setTab(id)}>
                    {label}
                  </button>
                ))}
              </div>
              {apmMode === "catalog" ? (
                <SoftwareCatalogPage onOpenService={(name) => setApmService(name)} />
              ) : (
                <>
                  <Sig
                    title={
                      apmMode === "traces"
                        ? "Trace Explorer — live & indexed spans"
                        : apmMode === "map"
                          ? "Service Map — real-time dependency graph"
                          : apm.length
                            ? `Worst p95: ${apm.reduce((a, s) => (s.p95_ms > a.p95_ms ? s : a)).service} @ ${fmt(Math.max(...apm.map((s) => s.p95_ms)), 0)}ms`
                            : "No APM services"
                    }
                    why={
                      apmMode === "traces"
                        ? "Search spans like Datadog Trace Explorer — open a row for the flame-graph waterfall"
                        : apmMode === "map"
                          ? "Nodes are services; edges are observed calls — filter noisy deps and jump into traces"
                          : `${apmErrors} errors in window — click a service for the APM Service Page`
                    }
                  />
                  {apmMode === "services" && (
                    <>
                      <div className="obs-kpi-row">
                        <Kpi label="Services" value={String(apm.length)} />
                        <Kpi label="Total requests" value={String(apm.reduce((a, s) => a + s.request_count, 0))} />
                        <Kpi label="Errors" value={String(apmErrors)} />
                        <Kpi
                          label="Worst p95"
                          value={
                            apm.length
                              ? `${apm.reduce((a, s) => (s.p95_ms > a.p95_ms ? s : a)).service} ${fmt(
                                  Math.max(...apm.map((s) => s.p95_ms)),
                                  0,
                                )}ms`
                              : "—"
                          }
                        />
                      </div>
                      <div className="obs-chart tall">
                        <ResponsiveContainer width="100%" height="100%">
                          <BarChart data={apm}>
                            <CartesianGrid stroke="rgba(255,255,255,0.06)" vertical={false} />
                            <XAxis dataKey="service" tick={{ fill: "#8B97A8", fontSize: 11 }} />
                            <YAxis tick={{ fill: "#8B97A8", fontSize: 11 }} />
                            <Tooltip contentStyle={tip} />
                            <Bar dataKey="p50_ms" fill="#4CC9F0" name="p50" />
                            <Bar dataKey="p95_ms" fill="#5B91EB" name="p95" />
                            <Bar dataKey="p99_ms" fill="#F25F5C" name="p99" />
                          </BarChart>
                        </ResponsiveContainer>
                      </div>
                      <table className="dd-table">
                        <thead>
                          <tr>
                            <th>Service</th>
                            <th>Requests</th>
                            <th>Errors</th>
                            <th>Error rate</th>
                            <th>Avg</th>
                            <th>p50</th>
                            <th>p95</th>
                            <th>p99</th>
                          </tr>
                        </thead>
                        <tbody>
                          {apm.map((s) => (
                            <tr
                              key={s.service}
                              style={{ cursor: "pointer" }}
                              onClick={() => setApmService(s.service)}
                            >
                              <td>
                                <code>{s.service}</code>
                              </td>
                              <td>{s.request_count}</td>
                              <td>{s.error_count}</td>
                              <td>{(s.error_rate * 100).toFixed(1)}%</td>
                              <td>{fmt(s.avg_ms, 1)}ms</td>
                              <td>{fmt(s.p50_ms, 1)}ms</td>
                              <td>{fmt(s.p95_ms, 1)}ms</td>
                              <td>{fmt(s.p99_ms, 1)}ms</td>
                            </tr>
                          ))}
                        </tbody>
                      </table>
                    </>
                  )}
                  {apmMode === "traces" && (
                    <TraceExplorer statsHint="Live-style span search · click a row for waterfall" />
                  )}
                  {apmMode === "map" && (
                    <TraceExplorer
                      statsHint="Service Map from APM + network + catalog"
                      initialView="map"
                    />
                  )}
                </>
              )}
            </>
          )}
        </div>
      )}

      {tab === "dbm" && (
        <DbPanel
          summary={dbSummary}
          instances={dbInstances}
          samples={dbSamples}
          queryMetrics={dbQueryMetrics}
          queries={queries}
          explain={explain}
          schemas={schemas}
          waits={dbWaits}
          blocking={dbBlocking}
          activity={dbActivity}
          health={dbHealth}
          catalog={dbCatalog}
          apm={dbApm}
        />
      )}

      {tab === "ai" && (
        <AiPanel
          summary={aiSummary}
          projects={aiProjects}
          runs={aiRuns}
          samples={aiSamples}
          experiments={aiExperiments}
          evals={aiEvals}
          graders={aiGraders}
          datasets={aiDatasets}
          feedback={aiFeedback}
          catalog={aiCatalog}
          health={aiHealth}
          onRefresh={loadAi}
        />
      )}

      {(tab === "infra" ||
        tab === "hostmap" ||
        tab === "containers" ||
        tab === "container-map" ||
        tab === "infra-envs" ||
        tab === "infra-k8s") && (
        <InfraPanel
          procs={procs}
          view={
            tab === "hostmap"
              ? "hostmap"
              : tab === "container-map"
                ? "container-map"
                : tab === "containers"
                  ? "containers"
                  : tab === "infra-envs"
                    ? "envs"
                    : tab === "infra-k8s"
                      ? "k8s"
                      : "overview"
          }
        />
      )}

      {tab === "processes" && <ProcessesExplorer />}

      {tab === "serverless" && <ServerlessPanel />}

      {baseTab === "logs" && (
        <div className="obs-section">
          <div className="obs-subtabs">
            {(
              [
                ["explorer", "logs", "Log Explorer"],
                ["live", "logs-live", "Live Tail"],
                ["errors", "logs-errors", "Error Tracking"],
                ["sds", "logs-sds", "SDS / PII"],
                ["patterns", "logs-patterns", "Facets"],
              ] as const
            ).map(([mode, id, label]) => (
              <button key={id} type="button" className={logMode === mode ? "on" : ""} onClick={() => setTab(id)}>
                {label}
              </button>
            ))}
          </div>
          <Sig
            title={
              logMode === "live"
                ? "Live Tail — near real-time structured logs"
                : logMode === "errors"
                  ? `${errors.length} error groups`
                  : logMode === "sds"
                    ? `${sds.length} sensitive-data hits`
                    : logMode === "patterns"
                      ? `${logFacets.length} facet dimensions`
                      : `${logErrors} error lines · ${errors.length} groups · ${sds.length} SDS hits`
            }
            why={
              logMode === "live"
                ? "Stream like Datadog Live Tail — facet by level/service; narrow the query when volume is high"
                : logMode === "errors"
                  ? "Error Tracking Explorer — aggregate by service + message fingerprint"
                  : logMode === "sds"
                    ? "Sensitive Data Scanner — PII/secrets matched in warn/error paths"
                    : logMode === "patterns"
                      ? "Facet panel for pivoting — click values to scope the explorer"
                      : "Full Log Explorer — search, facets, stream, and error groups together"
            }
          />
          <div className="obs-kpi-row">
            <Kpi label="Log sample" value={String(filteredLogs.length)} hint={`${logs.length} loaded`} />
            <Kpi
              label="Sample rate"
              value={logSampleRate < 1 ? `${(logSampleRate * 100).toFixed(0)}%` : "100%"}
              hint={logMode === "live" ? `WS ${liveWsStatus}` : "explorer"}
            />
            <Kpi label="Error lines" value={String(logErrors)} hint="in sample" />
            <Kpi label="Error groups" value={String(errors.length)} />
            <Kpi label="SDS hits" value={String(sds.length)} hint="PII/secrets" />
          </div>
          {(logMode === "explorer" || logMode === "live") && (
            <div className="obs-card" style={{ marginBottom: "0.75rem" }}>
              <h3>Query</h3>
              <p className="muted" style={{ marginTop: 0 }}>
                Datadog syntax: <code>service:api @http.status_code:500 -env:dev status:error</code>
              </p>
              <div className="bits-input">
                <input
                  value={logQueryDraft}
                  onChange={(e) => setLogQueryDraft(e.target.value)}
                  onKeyDown={(e) => {
                    if (e.key === "Enter") {
                      setLogQuery(logQueryDraft.trim());
                      void load();
                    }
                  }}
                  placeholder='service:api @http.status_code:500 "timeout"'
                />
                <button
                  type="button"
                  onClick={() => {
                    setLogQuery(logQueryDraft.trim());
                    void load();
                  }}
                >
                  Search
                </button>
              </div>
              {logQuery && (
                <small className="muted">
                  Active: <code>{logQuery}</code>
                </small>
              )}
            </div>
          )}
          {(logMode === "explorer" || logMode === "live" || logMode === "patterns") && (
            <div className="log-facets">
              {logFacets.map((f) => (
                <div key={f.key} className="log-facet">
                  <strong>{f.key}</strong>
                  <div>
                    <button
                      type="button"
                      className={(f.key === "level" ? logLevel : logService) === "all" ? "on" : ""}
                      onClick={() => (f.key === "level" ? setLogLevel("all") : setLogService("all"))}
                    >
                      all
                    </button>
                    {f.values.slice(0, 8).map(([v, n]) => (
                      <button
                        key={v}
                        type="button"
                        className={(f.key === "level" ? logLevel : logService) === v ? "on" : ""}
                        onClick={() => (f.key === "level" ? setLogLevel(v) : setLogService(v))}
                      >
                        {v} <em>{n}</em>
                      </button>
                    ))}
                  </div>
                </div>
              ))}
            </div>
          )}
          {(logMode === "explorer" || logMode === "live") && (
            <div className={`obs-card${logMode === "live" ? " log-live" : ""}`}>
              <h3>
                {logMode === "live" ? `Live Tail · WebSocket (${liveWsStatus})` : "Recent logs"}
              </h3>
              {logMode === "live" && (
                <p className="muted">Streaming via `/api/v1/logs/tail/ws` — not HTTP polling.</p>
              )}
              <div className="log-stream">
                {filteredLogs.slice(0, logMode === "live" ? 50 : 30).map((l, i) => (
                  <div key={i} className={`log-line ${l.level}`}>
                    <span>{new Date(l.timestamp_ms).toLocaleTimeString()}</span>
                    <b>{l.level}</b>
                    <code>{l.service}</code>
                    <em>{l.message}</em>
                  </div>
                ))}
                {!filteredLogs.length && <p className="muted">No logs match facets</p>}
              </div>
            </div>
          )}
          {logMode === "errors" && (
            <div className="obs-card">
              <h3>Error Tracking</h3>
              <p className="muted">Lifecycle: open → ignored / resolved / regressing — Datadog Error Tracking parity.</p>
              <table className="dd-table">
                <thead>
                  <tr>
                    <th>Service</th>
                    <th>Message</th>
                    <th>Count</th>
                    <th>Assignee</th>
                    <th>Status</th>
                    <th>Actions</th>
                  </tr>
                </thead>
                <tbody>
                  {errors.map((e) => (
                    <tr key={e.id}>
                      <td>
                        <code>{e.service}</code>
                      </td>
                      <td className="tags">
                        <div>{e.message}</div>
                        {e.stack_frames?.length ? (
                          <details>
                            <summary>stack ({e.stack_frames.length})</summary>
                            <pre style={{ fontSize: 11 }}>{e.stack_frames.join("\n")}</pre>
                          </details>
                        ) : null}
                        {e.linked_trace_id && (
                          <small className="muted">
                            trace <code>{e.linked_trace_id}</code>
                          </small>
                        )}
                        {e.linked_issues?.length ? (
                          <small className="muted">issues: {e.linked_issues.join(", ")}</small>
                        ) : null}
                      </td>
                      <td>{e.count}</td>
                      <td>{e.assignee || "—"}</td>
                      <td>
                        <span className={`sev ${e.status === "resolved" ? "ok" : e.status === "ignored" ? "warn" : "alert"}`}>
                          {e.status || "open"}
                        </span>
                      </td>
                      <td>
                        <div style={{ display: "flex", gap: "0.35rem", flexWrap: "wrap" }}>
                          {(["open", "ignored", "resolved", "regressing"] as const).map((st) => (
                            <button
                              key={st}
                              type="button"
                              className="ghost"
                              disabled={(e.status || "open") === st}
                              onClick={() => {
                                void api
                                  .patchError(e.id, { status: st })
                                  .then(() => load())
                                  .catch((err) =>
                                    setError(err instanceof Error ? err.message : "Patch failed"),
                                  );
                              }}
                            >
                              {st}
                            </button>
                          ))}
                          <button
                            type="button"
                            className="ghost"
                            onClick={() => {
                              const who = window.prompt("Assignee", e.assignee || "") ?? "";
                              void api
                                .patchError(e.id, { assignee: who })
                                .then(() => load())
                                .catch((err) =>
                                  setError(err instanceof Error ? err.message : "Assign failed"),
                                );
                            }}
                          >
                            assign
                          </button>
                          <button
                            type="button"
                            className="ghost"
                            onClick={() => {
                              const issue = window.prompt("Link issue (e.g. JIRA-123)", e.linked_issues?.[0] || "") ?? "";
                              if (!issue) return;
                              void api
                                .linkError(e.id, { issue, trace_id: e.linked_trace_id || undefined })
                                .then(() => load())
                                .catch((err) =>
                                  setError(err instanceof Error ? err.message : "Link failed"),
                                );
                            }}
                          >
                            link
                          </button>
                        </div>
                      </td>
                    </tr>
                  ))}
                  {!errors.length && (
                    <tr>
                      <td colSpan={6} className="muted">
                        No error groups
                      </td>
                    </tr>
                  )}
                </tbody>
              </table>
            </div>
          )}
          {logMode === "sds" && (
            <div className="obs-card">
              <h3>Sensitive Data Scanner</h3>
              <table className="dd-table">
                <thead>
                  <tr>
                    <th>Rule</th>
                    <th>Service</th>
                    <th>Snippet</th>
                  </tr>
                </thead>
                <tbody>
                  {sds.map((s, i) => (
                    <tr key={i}>
                      <td>
                        <code>{s.rule_id}</code>
                      </td>
                      <td>{s.service}</td>
                      <td className="tags">{s.snippet}</td>
                    </tr>
                  ))}
                  {!sds.length && (
                    <tr>
                      <td colSpan={3} className="muted">
                        No SDS hits in sample
                      </td>
                    </tr>
                  )}
                </tbody>
              </table>
            </div>
          )}
          {logMode === "patterns" && (
            <div className="obs-grid-2">
              <div className="obs-card">
                <h3>Log Patterns (ML-style clustering)</h3>
                <p className="muted">Drain-like templates — numbers/UUIDs normalized to &lt;*&gt;.</p>
                <table className="dd-table compact">
                  <thead>
                    <tr>
                      <th>Count</th>
                      <th>Template</th>
                      <th>Services</th>
                    </tr>
                  </thead>
                  <tbody>
                    {logPatterns.map((p) => (
                      <tr key={p.id}>
                        <td>{p.count}</td>
                        <td className="tags">
                          <code>{p.template}</code>
                        </td>
                        <td>{p.services.join(", ")}</td>
                      </tr>
                    ))}
                    {!logPatterns.length && (
                      <tr>
                        <td colSpan={3} className="muted">
                          No patterns yet
                        </td>
                      </tr>
                    )}
                  </tbody>
                </table>
              </div>
              <div className="obs-card">
                <h3>Observability Pipelines</h3>
                <p className="muted">Analytics / processing pipelines — list + dry-run.</p>
                <ul className="corr-metric-list">
                  {pipelines.map((p) => (
                    <li key={p.id}>
                      <div>
                        <strong>{p.name}</strong>
                        <div className="muted">{p.processors.join(" → ")}</div>
                      </div>
                      <button
                        type="button"
                        className="ghost"
                        onClick={() => {
                          void api
                            .runPipeline(p.id, logs.slice(0, 10))
                            .then((r) => setPipelineResult(JSON.stringify(r, null, 2)))
                            .catch((e) => setPipelineResult(e instanceof Error ? e.message : "run failed"));
                        }}
                      >
                        Run
                      </button>
                    </li>
                  ))}
                </ul>
                {pipelineResult && (
                  <pre style={{ fontSize: 11, maxHeight: 220, overflow: "auto" }}>{pipelineResult}</pre>
                )}
              </div>
            </div>
          )}
          {logMode === "explorer" && (
            <div className="obs-grid-2" style={{ marginTop: "0.75rem" }}>
              <div className="obs-card">
                <h3>Error groups preview</h3>
                <table className="dd-table compact">
                  <thead>
                    <tr>
                      <th>Service</th>
                      <th>Message</th>
                      <th>Count</th>
                    </tr>
                  </thead>
                  <tbody>
                    {errors.slice(0, 6).map((e) => (
                      <tr key={e.id}>
                        <td>{e.service}</td>
                        <td className="tags">{e.message}</td>
                        <td>{e.count}</td>
                      </tr>
                    ))}
                  </tbody>
                </table>
              </div>
              <div className="obs-card">
                <h3>SDS preview</h3>
                <table className="dd-table compact">
                  <thead>
                    <tr>
                      <th>Rule</th>
                      <th>Service</th>
                    </tr>
                  </thead>
                  <tbody>
                    {sds.slice(0, 6).map((s, i) => (
                      <tr key={i}>
                        <td>
                          <code>{s.rule_id}</code>
                        </td>
                        <td>{s.service}</td>
                      </tr>
                    ))}
                    {!sds.length && (
                      <tr>
                        <td colSpan={2} className="muted">
                          Clean sample
                        </td>
                      </tr>
                    )}
                  </tbody>
                </table>
              </div>
            </div>
          )}
        </div>
      )}

      {tab === "events" && (
        <div className="obs-section">
          <Sig
            title={criticalEvents ? `${criticalEvents} high-impact events in window` : "Event stream healthy"}
            why={
              criticalEvents
                ? "Deploy → flag → latency monitor → Watchdog → incident narrative is live — use impact filter to triage"
                : "No critical impact tags — keep watching deploys and monitor transitions"
            }
          />
          <div className="obs-kpi-row">
            <Kpi label="Events" value={String(filteredEvents.length)} hint={`${events.length} loaded`} />
            <Kpi label="Critical / error" value={String(criticalEvents)} />
            <Kpi label="Alert fires" value={String(alertEvents.length)} hint="monitor transitions" />
            <Kpi
              label="Deploys / changes"
              value={String(events.filter((e) => e.source === "deploy" || e.source === "change").length)}
            />
          </div>
          <div className="log-facets">
            <div className="log-facet">
              <strong>source</strong>
              <div>
                {eventSources.map((s) => (
                  <button key={s} type="button" className={eventSource === s ? "on" : ""} onClick={() => setEventSource(s)}>
                    {s}
                  </button>
                ))}
              </div>
            </div>
            <div className="log-facet">
              <strong>impact</strong>
              <div>
                {["all", "critical", "high", "medium", "low"].map((s) => (
                  <button key={s} type="button" className={eventImpact === s ? "on" : ""} onClick={() => setEventImpact(s)}>
                    {s}
                  </button>
                ))}
              </div>
            </div>
          </div>
          <div className="obs-grid-2">
            <div className="obs-card">
              <h3>Event Explorer — change correlation timeline</h3>
              <div className="log-stream">
                {filteredEvents.slice(0, 50).map((e, i) => (
                  <div
                    key={i}
                    className={`log-line ${e.alert_type === "error" ? "error" : e.alert_type === "warning" ? "warn" : "info"}`}
                  >
                    <span>{new Date(e.timestamp_ms).toLocaleTimeString()}</span>
                    <b>{e.tags?.impact || e.alert_type || "info"}</b>
                    <code>{e.source || "—"}</code>
                    <em>
                      <strong>{e.title}</strong> — {e.text}
                      {e.tags?.service ? ` · ${e.tags.service}` : ""}
                    </em>
                  </div>
                ))}
                {!filteredEvents.length && <p className="muted">No events for this filter</p>}
              </div>
            </div>
            <div className="obs-card">
              <h3>Alert fires (monitor engine)</h3>
              <p className="muted">Threshold crossings from live alert rules — pairs with Event Explorer deploys.</p>
              <table className="dd-table compact">
                <thead>
                  <tr>
                    <th>Rule</th>
                    <th>Metric</th>
                    <th>Value</th>
                    <th>Status</th>
                  </tr>
                </thead>
                <tbody>
                  {alertEvents.slice(0, 14).map((a) => (
                    <tr key={a.id}>
                      <td>{a.rule_name}</td>
                      <td>
                        <code>{a.metric}</code>
                      </td>
                      <td>
                        {fmt(a.value, 2)} / {fmt(a.threshold, 2)}
                      </td>
                      <td>
                        <span className={`sev ${a.status}`}>{a.status}</span>
                      </td>
                    </tr>
                  ))}
                  {!alertEvents.length && (
                    <tr>
                      <td colSpan={4} className="tags">
                        No recent alert fires — create a monitor to populate
                      </td>
                    </tr>
                  )}
                </tbody>
              </table>
            </div>
          </div>
        </div>
      )}

      {tab === "ux" && (
        <div className="obs-section">
          <Sig
            title={uxOverview?.significance || "UX Monitoring"}
            why={`Conversion ${(uxOverview?.conversion_rate ?? 0) * 100}% · revenue at risk $${fmt(uxOverview?.revenue_at_risk_usd ?? 0, 0)}`}
          />
          <div className="obs-kpi-row">
            <Kpi label="Sessions 24h" value={fmt(uxOverview?.sessions_24h ?? 0, 0)} />
            <Kpi label="Active users" value={fmt(uxOverview?.active_users ?? 0, 0)} />
            <Kpi label="Bounce rate" value={`${fmt((uxOverview?.bounce_rate ?? 0) * 100, 1)}%`} />
            <Kpi label="Web Vitals OK" value={`${fmt(uxOverview?.web_vitals_ok_pct ?? 0, 1)}%`} />
            <Kpi
              label="Synthetics"
              value={`${uxOverview?.synthetics_ok ?? 0}/${uxOverview?.synthetics_total ?? 0}`}
              hint="passing"
            />
            <Kpi label="Revenue at risk" value={`$${fmt(uxOverview?.revenue_at_risk_usd ?? 0, 0)}`} hint="/day" />
          </div>
          <div className="obs-card">
            <h3>Sessions · errors · LCP (24h)</h3>
            <div className="obs-chart tall">
              <ResponsiveContainer width="100%" height="100%">
                <LineChart data={uxTimeseries}>
                  <CartesianGrid stroke="rgba(255,255,255,0.06)" vertical={false} />
                  <XAxis dataKey="label" tick={{ fill: "#8B97A8", fontSize: 10 }} minTickGap={24} />
                  <YAxis yAxisId="l" tick={{ fill: "#8B97A8", fontSize: 10 }} width={40} />
                  <YAxis yAxisId="r" orientation="right" tick={{ fill: "#8B97A8", fontSize: 10 }} width={40} />
                  <Tooltip contentStyle={tip} />
                  <Line yAxisId="l" type="monotone" dataKey="sessions" stroke="#5B91EB" strokeWidth={2} dot={false} name="sessions" />
                  <Line yAxisId="l" type="monotone" dataKey="errors" stroke="#F25F5C" strokeWidth={2} dot={false} name="errors" />
                  <Line yAxisId="r" type="monotone" dataKey="lcp_p75_ms" stroke="#F4A261" strokeWidth={2} dot={false} name="LCP p75" />
                </LineChart>
              </ResponsiveContainer>
            </div>
          </div>
          <div className="obs-grid-2">
            <div className="obs-card">
              <h3>RUM sessions — frustration signals</h3>
              <table className="dd-table compact">
                <thead>
                  <tr>
                    <th>User</th>
                    <th>View</th>
                    <th>Device</th>
                    <th>Impact</th>
                    <th>Flags</th>
                  </tr>
                </thead>
                <tbody>
                  {uxSessions.map((s) => (
                    <tr key={s.id}>
                      <td>{s.user}</td>
                      <td>
                        <code>{s.view}</code>
                      </td>
                      <td>
                        {s.device} · {s.browser}
                      </td>
                      <td>
                        <span className={`sev ${s.impact === "critical" || s.impact === "high" ? "critical" : "ok"}`}>
                          {s.impact || "low"}
                        </span>
                      </td>
                      <td className="tags">
                        {s.bounced ? "bounce " : ""}
                        {s.has_error ? "error " : ""}
                        {s.frustration || "ok"}
                      </td>
                    </tr>
                  ))}
                </tbody>
              </table>
            </div>
            <div className="obs-card">
              <h3>Top pages by impact</h3>
              <table className="dd-table compact">
                <thead>
                  <tr>
                    <th>Path</th>
                    <th>Views</th>
                    <th>Err%</th>
                    <th>Why it matters</th>
                  </tr>
                </thead>
                <tbody>
                  {uxPages.map((p) => (
                    <tr key={p.path}>
                      <td>
                        <code>{p.path}</code>
                        <div className="tags">{p.impact}</div>
                      </td>
                      <td>{fmt(p.views, 0)}</td>
                      <td>{(p.error_rate * 100).toFixed(1)}%</td>
                      <td className="tags">{p.note}</td>
                    </tr>
                  ))}
                </tbody>
              </table>
            </div>
          </div>
          <div className="obs-grid-2">
            <div className="obs-card">
              <h3>Web Vitals</h3>
              <table className="dd-table compact">
                <thead>
                  <tr>
                    <th>Vital</th>
                    <th>p75</th>
                    <th>Good</th>
                    <th>Significance</th>
                  </tr>
                </thead>
                <tbody>
                  {uxVitals.map((v) => (
                    <tr key={v.name}>
                      <td>
                        <code>{v.name}</code>
                      </td>
                      <td>{v.p75_ms != null ? `${fmt(v.p75_ms, 0)}ms` : fmt(v.p75 ?? 0, 2)}</td>
                      <td>{fmt(v.good_pct, 0)}%</td>
                      <td className="tags">{v.significance || "—"}</td>
                    </tr>
                  ))}
                </tbody>
              </table>
              <h3 style={{ marginTop: "1rem" }}>Product funnel</h3>
              <div className="obs-chart">
                <ResponsiveContainer width="100%" height="100%">
                  <BarChart data={uxFunnel} layout="vertical" margin={{ left: 80 }}>
                    <CartesianGrid stroke="rgba(255,255,255,0.06)" horizontal={false} />
                    <XAxis type="number" tick={{ fill: "#8B97A8", fontSize: 10 }} />
                    <YAxis type="category" dataKey="step" tick={{ fill: "#8B97A8", fontSize: 10 }} width={90} />
                    <Tooltip contentStyle={tip} />
                    <Bar dataKey="users" fill="#A371E3" name="users" />
                  </BarChart>
                </ResponsiveContainer>
              </div>
            </div>
            <div className="obs-card">
              <h3>Synthetic tests</h3>
              <table className="dd-table compact">
                <thead>
                  <tr>
                    <th>Name</th>
                    <th>Latency</th>
                    <th>Status</th>
                    <th>Why</th>
                  </tr>
                </thead>
                <tbody>
                  {uxSynthetics.map((s) => (
                    <tr key={s.id}>
                      <td>
                        <strong>{s.name}</strong>
                        <div className="tags">{s.type}</div>
                      </td>
                      <td>{fmt(s.latency_ms, 0)}ms</td>
                      <td>
                        <span className={`sev ${s.status === "ok" ? "ok" : "critical"}`}>{s.status}</span>
                      </td>
                      <td className="tags">{s.significance || "—"}</td>
                    </tr>
                  ))}
                </tbody>
              </table>
            </div>
          </div>
        </div>
      )}

      {tab === "slos" && (
        <div className="obs-section">
          <Sig
            title={
              breachingSlos.length
                ? `${breachingSlos.length} SLO(s) breaching — error budget burning`
                : "All SLOs within target"
            }
            why={
              breachingSlos.length
                ? `${breachingSlos.map((s) => s.name).join(", ")} — pair with Events + Watchdog for the deploy/flag story`
                : "Budgets healthy — keep burn-rate monitors on 1h/6h windows"
            }
          />
          <div className="obs-kpi-row">
            <Kpi label="SLOs" value={String(slos.length)} />
            <Kpi label="Breaching" value={String(breachingSlos.length)} />
            <Kpi
              label="Avg budget left"
              value={`${fmt(slos.length ? slos.reduce((a, s) => a + s.budget_left, 0) / slos.length : 0, 1)}%`}
            />
            <Kpi
              label="Worst current"
              value={
                slos.length
                  ? `${fmt(Math.min(...slos.map((s) => s.current_pct)), 1)}%`
                  : "—"
              }
            />
          </div>
          <div className="obs-grid-2">
            <div className="obs-card">
              <h3>Service Level Objectives</h3>
              <table className="dd-table compact">
                <thead>
                  <tr>
                    <th>Name</th>
                    <th>Metric</th>
                    <th>Target</th>
                    <th>Current</th>
                    <th>Budget</th>
                    <th>Status</th>
                  </tr>
                </thead>
                <tbody>
                  {slos.map((s) => (
                    <tr key={s.id}>
                      <td>
                        <strong>{s.name}</strong>
                      </td>
                      <td>
                        <code>{s.metric}</code>
                      </td>
                      <td>{fmt(s.target, 1)}%</td>
                      <td>{fmt(s.current_pct, 1)}%</td>
                      <td>{fmt(s.budget_left, 1)}%</td>
                      <td>
                        <span className={`sev ${s.status === "breaching" ? "critical" : "ok"}`}>{s.status}</span>
                      </td>
                    </tr>
                  ))}
                </tbody>
              </table>
            </div>
            <div className="obs-card">
              <h3>Budget remaining</h3>
              <div className="obs-chart">
                <ResponsiveContainer width="100%" height="100%">
                  <BarChart data={slos.map((s) => ({ name: s.name.replace(/ SLO| budget/gi, ""), budget: s.budget_left, target: s.target }))}>
                    <CartesianGrid stroke="rgba(255,255,255,0.06)" vertical={false} />
                    <XAxis dataKey="name" tick={{ fill: "#8B97A8", fontSize: 10 }} />
                    <YAxis tick={{ fill: "#8B97A8", fontSize: 10 }} />
                    <Tooltip contentStyle={tip} />
                    <Bar dataKey="budget" fill="#2EC4B6" name="budget left %" />
                  </BarChart>
                </ResponsiveContainer>
              </div>
            </div>
          </div>
        </div>
      )}

      {tab === "data" && (
        <div className="obs-section">
          <div className="obs-kpi-row">
            <Kpi label="Streams" value={String(streams.length)} />
            <Kpi
              label="Total lag"
              value={String(streams.reduce((a, s) => a + s.lag, 0))}
              hint="messages behind"
            />
            <Kpi label="Lineage edges" value={String(lineage?.edges?.length ?? 0)} />
            <Kpi label="Freshness anomalies" value={String(lineage?.freshness_anomalies ?? 0)} />
          </div>
          <div className="obs-grid-2">
            <div className="obs-card">
              <h3>Data streams</h3>
              <table className="dd-table compact">
                <thead>
                  <tr>
                    <th>Stream</th>
                    <th>Lag</th>
                    <th>Throughput</th>
                  </tr>
                </thead>
                <tbody>
                  {streams.map((s) => (
                    <tr key={s.name}>
                      <td>
                        <code>{s.name}</code>
                      </td>
                      <td>{s.lag}</td>
                      <td>{fmt(s.throughput, 0)}/s</td>
                    </tr>
                  ))}
                </tbody>
              </table>
              {workers && (
                <div style={{ marginTop: "0.75rem" }}>
                  <h3>Pipeline workers</h3>
                  <table className="dd-table compact">
                    <tbody>
                      {Object.entries(
                        (typeof workers.workers === "object" && workers.workers
                          ? (workers.workers as Record<string, unknown>)
                          : workers) as Record<string, unknown>,
                      )
                        .slice(0, 12)
                        .map(([k, v]) => (
                          <tr key={k}>
                            <td>
                              <code>{k}</code>
                            </td>
                            <td>{typeof v === "object" ? JSON.stringify(v).slice(0, 80) : String(v)}</td>
                          </tr>
                        ))}
                    </tbody>
                  </table>
                </div>
              )}
            </div>
            <div className="obs-card">
              <h3>Data lineage</h3>
              <div className="lineage">
                {(lineage?.edges ?? []).map((e, i) => (
                  <div key={i} className="lineage-edge">
                    <code>{e.from_asset}</code>
                    <span>→ {e.kind} →</span>
                    <code>{e.to_asset}</code>
                  </div>
                ))}
              </div>
            </div>
          </div>
        </div>
      )}

      {tab === "cost" && (
        <div className="obs-section">
          <div className="obs-kpi-row">
            <Kpi label="Recommendations" value={String(costRecs.length)} />
            <Kpi label="Potential savings" value={`$${fmt(savings, 0)}/mo`} />
            <Kpi
              label="Tracked spend"
              value={`$${fmt(
                costDetail.reduce((a, c) => a + c.total, 0),
                0,
              )}`}
            />
          </div>
          <div className="obs-grid-2">
            <div className="obs-card">
              <h3>Recommendations</h3>
              <table className="dd-table compact">
                <thead>
                  <tr>
                    <th>Action</th>
                    <th>Resource</th>
                    <th>Save/mo</th>
                  </tr>
                </thead>
                <tbody>
                  {costRecs.map((r) => (
                    <tr key={r.id}>
                      <td>
                        <strong>{r.title}</strong>
                        <div className="tags">{r.rationale}</div>
                      </td>
                      <td>
                        <code>{r.resource_id}</code>
                      </td>
                      <td>${fmt(r.savings_month, 0)}</td>
                    </tr>
                  ))}
                </tbody>
              </table>
            </div>
            <div className="obs-card">
              <h3>Cost by service</h3>
              <div className="obs-chart">
                <ResponsiveContainer width="100%" height="100%">
                  <BarChart data={costDetail}>
                    <CartesianGrid stroke="rgba(255,255,255,0.06)" vertical={false} />
                    <XAxis dataKey="service" tick={{ fill: "#8B97A8", fontSize: 10 }} />
                    <YAxis tick={{ fill: "#8B97A8", fontSize: 10 }} />
                    <Tooltip contentStyle={tip} />
                    <Bar dataKey="compute" stackId="a" fill="#5B91EB" />
                    <Bar dataKey="storage" stackId="a" fill="#A371E3" />
                    <Bar dataKey="network" stackId="a" fill="#2EC4B6" />
                  </BarChart>
                </ResponsiveContainer>
              </div>
            </div>
          </div>
        </div>
      )}

      {tab === "gpu" && (
        <GpuPanel
          summary={gpuSummary}
          procs={gpuProcs}
          samples={gpuSamples}
          health={gpuHealth}
          catalog={gpuCatalog}
        />
      )}

      {baseTab === "ha" && (
        <div className="obs-section">
          <div className="obs-subtabs">
            <button type="button" className={haMode === "ha" ? "on" : ""} onClick={() => setTab("ha")}>
              HA status
            </button>
            <button type="button" className={haMode === "watchdog" ? "on" : ""} onClick={() => setTab("ha-watchdog")}>
              Watchdog anomalies
            </button>
          </div>
          <Sig
            title={
              topWatchdog
                ? `${String(topWatchdog.severity || "anomaly").toUpperCase()}: ${String(topWatchdog.metric || "metric")} ${fmt(Number(topWatchdog.z_score ?? 0), 1)}σ`
                : "Watchdog quiet — HA quorum intact"
            }
            why={
              topWatchdog
                ? String(topWatchdog.message || topWatchdog.summary || "Investigate correlated Events + APM")
                : `Quorum ${ha?.quorum ? "yes" : "no"} · failover ${ha?.failover_ready ? "ready" : "not ready"} · RPO/RTO ${ha ? `${ha.rpo_seconds}s/${ha.rto_seconds}s` : "—"}`
            }
          />
          <div className="obs-kpi-row">
            <Kpi label="Mode" value={ha?.mode ?? "—"} />
            <Kpi label="Primary" value={ha?.primary ?? "—"} />
            <Kpi label="Quorum" value={ha?.quorum ? "yes" : "no"} />
            <Kpi label="Failover ready" value={ha?.failover_ready ? "yes" : "no"} />
            <Kpi label="Watchdog critical" value={String(watchdog?.critical ?? watchdogAnoms.filter((a) => a.severity === "critical").length)} />
            <Kpi label="Anomalies" value={String(watchdogAnoms.length)} />
          </div>
          <div className="obs-grid-2">
            {haMode === "ha" && (
            <div className="obs-card">
              <h3>Multi-region HA</h3>
              <table className="dd-table compact">
                <thead>
                  <tr>
                    <th>Region</th>
                    <th>Role</th>
                    <th>Healthy</th>
                    <th>Lag</th>
                    <th>Series</th>
                  </tr>
                </thead>
                <tbody>
                  {(ha?.regions ?? []).map((r) => (
                    <tr key={r.id}>
                      <td>
                        <code>{r.name}</code>
                      </td>
                      <td>{r.role}</td>
                      <td>{r.healthy ? "ok" : "down"}</td>
                      <td>{r.lag_ms}ms</td>
                      <td>{fmt(Number((r as { series_count?: number }).series_count ?? 0), 0)}</td>
                    </tr>
                  ))}
                </tbody>
              </table>
            </div>
            )}
            <div className="obs-card">
              <h3>Watchdog ML — why it matters</h3>
              <div className="obs-kpi-row" style={{ marginBottom: "0.5rem" }}>
                <Kpi label="Algorithm" value={String(watchdog?.algorithm ?? "seasonal_mad")} />
                <Kpi label="Critical" value={String(watchdog?.critical ?? 0)} />
                <Kpi label="High" value={String(watchdog?.high ?? 0)} />
                <Kpi label="σ threshold" value={String(watchdog?.threshold_sigma ?? 3)} />
              </div>
              <table className="dd-table compact">
                <thead>
                  <tr>
                    <th>Metric</th>
                    <th>Sev</th>
                    <th>σ / score</th>
                    <th>Baseline → observed</th>
                    <th>Significance</th>
                  </tr>
                </thead>
                <tbody>
                  {watchdogAnoms.slice(0, 12).map((a, i) => (
                    <tr key={String(a.id ?? i)}>
                      <td>
                        <code>{String(a.metric ?? "—")}</code>
                        <div className="tags">{a.tags?.service || a.tags?.host || "—"}</div>
                      </td>
                      <td>
                        <span className={`sev ${String(a.severity ?? "medium")}`}>{String(a.severity ?? "—")}</span>
                      </td>
                      <td>
                        {a.z_score != null ? `${fmt(Number(a.z_score), 1)}σ` : "—"} /{" "}
                        {a.score != null ? fmt(Number(a.score), 2) : "—"}
                      </td>
                      <td>
                        {a.baseline != null && a.observed != null
                          ? `${fmt(Number(a.baseline), 2)} → ${fmt(Number(a.observed), 2)}`
                          : "—"}
                      </td>
                      <td className="tags">{String(a.message || a.summary || "—")}</td>
                    </tr>
                  ))}
                  {!watchdogAnoms.length && (
                    <tr>
                      <td colSpan={5} className="tags">
                        No anomalies cached — run a scan to populate
                      </td>
                    </tr>
                  )}
                </tbody>
              </table>
              <button className="ghost" disabled={scanning} onClick={runWatchdog} style={{ marginTop: "0.65rem" }}>
                {scanning ? "Scanning…" : "Run Watchdog scan"}
              </button>
            </div>
          </div>
        </div>
      )}
    </section>
  );
}
