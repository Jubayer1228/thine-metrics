export type Tags = Record<string, string>;

export type MetricMeta = {
  name: string;
  metric_type: string;
  tags: Tags;
  unit?: string | null;
  description?: string | null;
  last_value?: number | null;
  last_seen_ms?: number | null;
  sample_count: number;
};

export type Sample = { timestamp_ms: number; value: number };

export type QueryResult = {
  metric: string;
  tags: Tags;
  aggregation: string;
  points: Sample[];
};

export type Dashboard = {
  series_count: number;
  sample_count: number;
  ingest_rate_per_sec: number;
  active_alerts: number;
  uptime_secs: number;
  top_metrics: MetricMeta[];
};

export type AlertRule = {
  id: string;
  name: string;
  metric: string;
  tags?: Tags;
  threshold: number;
  comparator: string;
  window_ms: number;
  enabled: boolean;
};

export type TemplateVariable = {
  name: string;
  tag: string;
  default: string;
  available_values: string[];
  prefix: string;
};

export type SavedView = {
  id: string;
  name: string;
  description?: string | null;
  selections: Record<string, string>;
};

export type BoardAnnotation = {
  id: string;
  timestamp_ms: number;
  label: string;
  color?: string | null;
};

export type ShareConfig = {
  public: boolean;
  token?: string | null;
  refresh_secs: number;
};

export type BoardMeta = {
  id: string;
  name: string;
  description?: string | null;
  layout_type?: "dashboard" | "timeboard" | "screenboard";
  widgets: { id: string; type: string; title: string; metric: string }[];
  template_variables?: TemplateVariable[];
  tags?: string[];
  share?: ShareConfig | null;
  author?: string | null;
  deleted_at?: string | null;
  recoverable_until?: string | null;
  created_at?: string;
  updated_at?: string | null;
};

export type GraphAnomalyRegion = {
  start_ms: number;
  end_ms: number;
  severity: string;
  summary: string;
  deviation_pct: number;
  z_score: number;
  influential_tags: { key: string; value: string; contribution: number; message: string }[];
};

export type DashboardAnomalyIssue = {
  id: string;
  title: string;
  metric: string;
  widget_ids: string[];
  widget_titles: string[];
  detected_at_ms: number;
  anomaly: GraphAnomalyRegion;
  influential_tags: GraphAnomalyRegion["influential_tags"];
  co_occurring_metrics: string[];
  next_steps: string[];
};

export type DashboardAnomaliesResponse = {
  board_id: string;
  board_name: string;
  auto_detect: boolean;
  issues: DashboardAnomalyIssue[];
  scanned_widgets: number;
  elapsed_hint_ms: number;
};

export type CorrelationHit = {
  source_type: string;
  source: string;
  metric: string;
  tags: Tags;
  score: number;
  z_score: number;
  correlations: number;
  preview: Sample[];
};

export type CorrelationGroup = {
  source_type: string;
  source: string;
  correlations: number;
  preview: Sample[];
  metrics: CorrelationHit[];
};

export type CorrelationSearchResponse = {
  metric: string;
  interest_start_ms: number;
  interest_end_ms: number;
  results: CorrelationGroup[];
  hits: CorrelationHit[];
};

export type WatchdogExplainResult = {
  metric: string;
  anomaly: GraphAnomalyRegion | null;
  affects_everyone: boolean;
  influential_tags: GraphAnomalyRegion["influential_tags"];
  findings: {
    tag_key: string;
    tag_value: string;
    contribution: number;
    message: string;
    with_tag_peak: number;
    without_tag_peak: number;
  }[];
  summary: string;
};

export type DashboardList = {
  id: string;
  name: string;
  board_ids: string[];
};

export type DashboardsGuide = {
  title: string;
  docs: string;
  overview: string;
  layouts: { id: string; label: string; desc: string }[];
  features: { id: string; path: string; desc: string }[];
  refresh_rates: { timeframe: string; secs: number }[];
  widget_types: string[];
};

export type WidgetTypeName =
  | "timeseries"
  | "query_value"
  | "toplist"
  | "note"
  | "group"
  | "heatmap"
  | "distribution"
  | "pie_chart"
  | "table"
  | "hostmap"
  | "slo"
  | "event_stream"
  | "alert_graph"
  | "change"
  | "scatter_plot"
  | "funnel"
  | "list_stream"
  | "check_status";

export type RenderedWidget = {
  id: string;
  type: WidgetTypeName;
  title: string;
  layout: { x: number; y: number; w: number; h: number };
  unit?: string | null;
  display?: string | null;
  text?: string | null;
  value?: number | null;
  previous_value?: number | null;
  change_pct?: number | null;
  sparkline: Sample[];
  series: QueryResult[];
  toplist: { label: string; value: number; tags: Tags }[];
  overlays?: BoardAnnotation[];
  tab?: string | null;
  functions?: string[];
  anomalies?: GraphAnomalyRegion[];
  accent?: string | null;
};

export type RenderedBoard = {
  id: string;
  name: string;
  description?: string | null;
  layout_type: "dashboard" | "timeboard" | "screenboard";
  range_ms: number;
  start_ms: number;
  end_ms: number;
  refresh_secs: number;
  template_variables: TemplateVariable[];
  template_selections: Record<string, string>;
  saved_views: SavedView[];
  annotations: BoardAnnotation[];
  share?: ShareConfig | null;
  widgets: RenderedWidget[];
  tabs: string[];
};

export type MetricSummaryRow = {
  name: string;
  metric_type: string;
  series_count: number;
  tag_keys: string[];
  last_value?: number | null;
  avg?: number | null;
  min?: number | null;
  max?: number | null;
  unit?: string | null;
  sparkline: Sample[];
};

async function getJson<T>(path: string): Promise<T> {
  let res: Response;
  try {
    res = await fetch(path);
  } catch {
    throw new Error(
      `Connection failed — is Thine running? Start with: ./scripts/dev.sh (http://localhost:4318)`,
    );
  }
  if (!res.ok) throw new Error(`${res.status} ${res.statusText}`);
  return res.json();
}

export const api = {
  health: () => getJson<{ status: string; version: string }>("/health"),
  ingestArchitecture: () => getJson<Record<string, unknown>>("/api/v1/ingest/architecture"),
  dashboard: () => getJson<Dashboard>("/api/v1/dashboard"),
  metrics: (prefix?: string) =>
    getJson<MetricMeta[]>(
      prefix ? `/api/v1/metrics?prefix=${encodeURIComponent(prefix)}` : "/api/v1/metrics",
    ),
  metricsSummary: (range_ms = 3_600_000) =>
    getJson<MetricSummaryRow[]>(`/api/v1/metrics/summary?range_ms=${range_ms}`),
  query: async (params: {
    metric: string;
    tags?: string;
    aggregation?: string;
    start_ms?: number;
    end_ms?: number;
    step_ms?: number;
    group_by?: string;
  }) => {
    const qs = new URLSearchParams();
    qs.set("metric", params.metric);
    if (params.tags) qs.set("tags", params.tags);
    if (params.aggregation) qs.set("aggregation", params.aggregation);
    if (params.start_ms) qs.set("start_ms", String(params.start_ms));
    if (params.end_ms) qs.set("end_ms", String(params.end_ms));
    if (params.step_ms) qs.set("step_ms", String(params.step_ms));
    if (params.group_by) qs.set("group_by", params.group_by);
    const data = await getJson<{ results: QueryResult[] }>(`/api/v1/query?${qs}`);
    return data.results;
  },
  alerts: () => getJson<AlertRule[]>("/api/v1/alerts"),
  boards: () => getJson<BoardMeta[]>("/api/v1/boards"),
  boardLists: () => getJson<DashboardList[]>("/api/v1/boards/lists"),
  dashboardsGuide: () => getJson<DashboardsGuide>("/api/v1/dashboards/guide"),
  renderBoard: (id: string, range_ms: number, opts?: { tags?: string; vars?: string }) => {
    const qs = new URLSearchParams({ range_ms: String(range_ms) });
    if (opts?.tags) qs.set("tags", opts.tags);
    if (opts?.vars) qs.set("vars", opts.vars);
    return getJson<RenderedBoard>(`/api/v1/boards/${id}/render?${qs}`);
  },
  createBoard: async (body: {
    name: string;
    description?: string;
    layout_type?: string;
    template_variables?: TemplateVariable[];
    tags?: string[];
    widgets?: unknown[];
  }) => {
    const res = await fetch("/api/v1/boards", {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify(body),
    });
    if (!res.ok) throw new Error(await res.text());
    return res.json() as Promise<BoardMeta>;
  },
  shareBoard: async (id: string, publicShare: boolean) => {
    const res = await fetch(`/api/v1/boards/${id}/share`, {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify({ public: publicShare }),
    });
    if (!res.ok) throw new Error(await res.text());
    return res.json() as Promise<BoardMeta>;
  },
  clipboardGet: () => getJson<unknown[]>("/api/v1/boards/clipboard"),
  clipboardSet: async (widgets: unknown[]) => {
    const res = await fetch("/api/v1/boards/clipboard", {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify(widgets),
    });
    if (!res.ok) throw new Error(await res.text());
    return res.json() as Promise<{ ok: boolean; count: number }>;
  },
  deletedBoards: () => getJson<BoardMeta[]>("/api/v1/boards/deleted"),
  restoreBoard: async (id: string, list_id?: string) => {
    const res = await fetch(`/api/v1/boards/${id}/restore`, {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify({ list_id: list_id || null }),
    });
    if (!res.ok) throw new Error(await res.text());
    return res.json() as Promise<BoardMeta>;
  },
  deleteBoard: async (id: string) => {
    const res = await fetch(`/api/v1/boards/${id}`, { method: "DELETE" });
    if (!res.ok && res.status !== 204) throw new Error(await res.text());
  },
  boardAnomalies: (id: string, range_ms: number, auto_detect = true) =>
    getJson<DashboardAnomaliesResponse>(
      `/api/v1/boards/${id}/anomalies?range_ms=${range_ms}&auto_detect=${auto_detect}`,
    ),
  graphCorrelations: async (body: {
    metric: string;
    tags?: Tags;
    start_ms: number;
    end_ms: number;
    interest_start_ms?: number;
    interest_end_ms?: number;
    sources?: string[];
    namespaces?: string[];
    limit?: number;
  }) => {
    const res = await fetch("/api/v1/graph_insights/correlations", {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify(body),
    });
    if (!res.ok) throw new Error(await res.text());
    return res.json() as Promise<CorrelationSearchResponse>;
  },
  graphExplain: async (body: {
    metric: string;
    tags?: Tags;
    start_ms: number;
    end_ms: number;
    group_keys?: string[];
  }) => {
    const res = await fetch("/api/v1/graph_insights/explain", {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify(body),
    });
    if (!res.ok) throw new Error(await res.text());
    return res.json() as Promise<WatchdogExplainResult>;
  },
  graphInsightsGuide: () => getJson<Record<string, unknown>>("/api/v1/graph_insights/guide"),
  createAlert: async (body: {
    name: string;
    metric: string;
    threshold: number;
    comparator: string;
    window_ms?: number;
    tags?: Tags;
    options?: {
      detection_method?: string;
      warning_threshold?: number | null;
      recovery_threshold?: number | null;
      recipients?: string;
      evaluate?: string;
      change_type?: string;
      comparison_window_ms?: number;
      anomaly_direction?: string;
      forecast_horizon_ms?: number;
      group_by?: string | null;
      message?: string;
      severity?: string;
    };
  }) => {
    const res = await fetch("/api/v1/alerts", {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify(body),
    });
    if (!res.ok) throw new Error(await res.text());
    return res.json();
  },
  testAlertNotify: async (body: { recipients: string; rule_name: string; metric: string }) => {
    const res = await fetch("/api/v1/notifications/test", {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify(body),
    });
    if (!res.ok) throw new Error(await res.text());
    return res.json() as Promise<{ deliveries: number }>;
  },

  // Observability surfaces
  featuresStats: () =>
    getJson<{
      total: number;
      done: number;
      partial: number;
      stub: number;
      planned: number;
      avg_parity_pct: number;
    }>("/api/v1/features/stats"),
  features: () => getJson<FeatureEntry[]>("/api/v1/features"),
  cloudcraftDiagram: (params?: {
    provider?: string;
    overlay?: string;
    group_by?: string;
    q?: string;
  }) => {
    const qs = new URLSearchParams();
    if (params?.provider) qs.set("provider", params.provider);
    if (params?.overlay) qs.set("overlay", params.overlay);
    if (params?.group_by) qs.set("group_by", params.group_by);
    if (params?.q) qs.set("q", params.q);
    const q = qs.toString();
    return getJson<CloudcraftDiagram>(`/api/v1/cloudcraft/diagram${q ? `?${q}` : ""}`);
  },
  cloudcraftResources: () => getJson<CloudResource[]>("/api/v1/cloudcraft/resources"),
  liveProcesses: (host?: string, limit = 50) => {
    const qs = new URLSearchParams();
    if (host) qs.set("host", host);
    qs.set("limit", String(limit));
    const q = qs.toString();
    return getJson<LiveProcess[]>(`/api/v1/infra/processes${q ? `?${q}` : ""}`);
  },
  processesExplorer: (opts?: {
    host?: string;
    user?: string;
    command?: string;
    service?: string;
    env?: string;
    q?: string;
    limit?: number;
  }) => {
    const p = new URLSearchParams();
    if (opts?.host) p.set("host", opts.host);
    if (opts?.user) p.set("user", opts.user);
    if (opts?.command) p.set("command", opts.command);
    if (opts?.service) p.set("service", opts.service);
    if (opts?.env) p.set("env", opts.env);
    if (opts?.q) p.set("q", opts.q);
    p.set("limit", String(opts?.limit ?? 200));
    return getJson<ProcessExplorer>(`/api/v1/infra/processes/explorer?${p}`);
  },
  usmRed: () => getJson<UsmaRed[]>("/api/v1/usm/red"),
  usmEbpf: () => getJson<Record<string, unknown>>("/api/v1/usm/ebpf"),
  apmStats: () => getJson<ApmStat[]>("/api/v1/apm/stats"),
  apmService: (name: string) => getJson<Record<string, unknown>>(`/api/v1/apm/services/${encodeURIComponent(name)}`),
  catalogServices: () => getJson<CatalogService[]>("/api/v1/catalog/services"),
  catalogScorecards: () => getJson<ServiceScorecard[]>("/api/v1/catalog/scorecards"),
  catalogScorecard: (name: string) =>
    getJson<ServiceScorecard>(`/api/v1/catalog/services/${encodeURIComponent(name)}/scorecard`),
  catalogDefinition: (name: string) =>
    getJson<{ service: string; yaml: string }>(
      `/api/v1/catalog/services/${encodeURIComponent(name)}/definition`,
    ),
  putCatalogDefinition: async (name: string, yaml: string) => {
    const res = await fetch(`/api/v1/catalog/services/${encodeURIComponent(name)}/definition`, {
      method: "PUT",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify({ yaml }),
    });
    if (!res.ok) throw new Error(await res.text());
    return res.json() as Promise<CatalogService>;
  },
  logPatterns: (limit = 40) => getJson<LogPattern[]>(`/api/v1/logs/patterns?limit=${limit}`),
  kubeContainerMap: () => getJson<KubeContainerMap>("/api/v1/infra/containermap"),
  shareNotebook: async (id: string) => {
    const res = await fetch(`/api/v1/notebooks/${id}/share`, { method: "POST" });
    if (!res.ok) throw new Error(await res.text());
    return res.json() as Promise<Notebook>;
  },
  linkError: async (id: string, body: { issue: string; trace_id?: string }) => {
    const res = await fetch(`/api/v1/errors/${encodeURIComponent(id)}/link`, {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify(body),
    });
    if (!res.ok) throw new Error(await res.text());
    return res.json();
  },
  pipelines: () => getJson<{ id: string; name: string; processors: string[] }[]>("/api/v1/pipelines"),
  runPipeline: async (id: string, events: LogEvent[] = []) => {
    const res = await fetch(`/api/v1/pipelines/${encodeURIComponent(id)}/run`, {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify(events),
    });
    if (!res.ok) throw new Error(await res.text());
    return res.json();
  },
  dbmExplain: () => getJson<ExplainPlan[]>("/api/v1/dbm/explain"),
  dbmSchema: () => getJson<DbSchema[]>("/api/v1/dbm/schema"),
  dbmQueries: () => getJson<DbQuery[]>("/api/v1/dbm/queries"),
  dbmApm: () => getJson<Record<string, unknown>>("/api/v1/dbm/apm"),
  dbmInstances: () => getJson<DbInstance[]>("/api/v1/dbm/instances"),
  dbmSummary: () => getJson<Record<string, unknown>>("/api/v1/dbm/summary"),
  dbmHealth: () =>
    getJson<{
      instances_checked: number;
      findings: { db: string; severity: string; finding: string }[];
      blocking: number;
      wait_events: number;
    }>("/api/v1/dbm/health"),
  dbmMetricsCatalog: () =>
    getJson<{ count: number; metrics: DbMetricDef[]; groups: Record<string, number> }>(
      "/api/v1/dbm/metrics",
    ),
  dbmSamples: (db?: string, limit = 180) => {
    const qs = new URLSearchParams({ limit: String(limit) });
    if (db) qs.set("db", db);
    return getJson<DbHostSample[]>(`/api/v1/dbm/samples?${qs}`);
  },
  dbmQueryMetrics: (limit = 50) =>
    getJson<DbQueryMetric[]>(`/api/v1/dbm/query_metrics?limit=${limit}`),
  dbmWaits: () => getJson<DbWaitEvent[]>("/api/v1/dbm/waits"),
  dbmBlocking: () => getJson<DbBlocking[]>("/api/v1/dbm/blocking"),
  dbmActivity: (limit = 50) => getJson<DbActivity[]>(`/api/v1/dbm/activity?limit=${limit}`),
  dbmEmit: async () => {
    const res = await fetch("/api/v1/dbm/emit", { method: "POST", headers: { "Content-Type": "application/json" }, body: "{}" });
    if (!res.ok) throw new Error(await res.text());
    return res.json() as Promise<{ points_emitted: number }>;
  },
  aiSummary: () => getJson<Record<string, unknown>>("/api/v1/ai/summary"),
  aiProjects: () => getJson<AiProject[]>("/api/v1/ai/projects"),
  aiRuns: (project?: string, runType?: string, limit = 100) => {
    const qs = new URLSearchParams({ limit: String(limit) });
    if (project) qs.set("project", project);
    if (runType) qs.set("run_type", runType);
    return getJson<AiRun[]>(`/api/v1/ai/runs?${qs}`);
  },
  aiRunsBatch: async (runs: Partial<AiRun>[]) => {
    const res = await fetch("/api/v1/ai/runs/batch", {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify(runs),
    });
    if (!res.ok) throw new Error(await res.text());
    return res.json() as Promise<{ ingested: number; elapsed_us: number }>;
  },
  aiFeedback: (limit = 100) => getJson<AiFeedback[]>(`/api/v1/ai/feedback?limit=${limit}`),
  aiDatasets: () => getJson<AiDataset[]>("/api/v1/ai/datasets"),
  aiExamples: (datasetId?: string) =>
    getJson<AiExample[]>(
      datasetId ? `/api/v1/ai/examples?dataset_id=${encodeURIComponent(datasetId)}` : "/api/v1/ai/examples",
    ),
  aiExperiments: () => getJson<AiExperiment[]>("/api/v1/ai/experiments"),
  aiRunExperiment: async (datasetId: string, trials = 3) => {
    const res = await fetch("/api/v1/ai/experiments/run", {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify({ dataset_id: datasetId, trials }),
    });
    if (!res.ok) throw new Error(await res.text());
    return res.json() as Promise<Record<string, unknown>>;
  },
  aiEvals: (experimentId?: string) =>
    getJson<AiEvalResult[]>(
      experimentId
        ? `/api/v1/ai/evals?experiment_id=${encodeURIComponent(experimentId)}`
        : "/api/v1/ai/evals",
    ),
  aiGraders: () => getJson<AiGrader[]>("/api/v1/ai/graders"),
  aiSamples: (project?: string, limit = 180) => {
    const qs = new URLSearchParams({ limit: String(limit) });
    if (project) qs.set("project", project);
    return getJson<AiHostSample[]>(`/api/v1/ai/samples?${qs}`);
  },
  aiMetricsCatalog: () =>
    getJson<{ count: number; metrics: AiMetricDef[]; groups: Record<string, number> }>(
      "/api/v1/ai/metrics",
    ),
  aiHealth: () =>
    getJson<{
      projects_checked: number;
      findings: { project: string; severity: string; finding: string }[];
      experiments: number;
    }>("/api/v1/ai/health"),
  aiEmit: async () => {
    const res = await fetch("/api/v1/ai/emit", { method: "POST", headers: { "Content-Type": "application/json" }, body: "{}" });
    if (!res.ok) throw new Error(await res.text());
    return res.json() as Promise<{ points_emitted: number }>;
  },
  streams: () => getJson<StreamInfo[]>("/api/v1/streams"),
  dataLineage: () => getJson<DataLineage>("/api/v1/data/lineage"),
  costRecommendations: () => getJson<CostRec[]>("/api/v1/cost/recommendations"),
  costDetail: () => getJson<CostLine[]>("/api/v1/cost/detail"),
  haStatus: () => getJson<HaStatus>("/api/v1/ha/status"),
  watchdogSummary: () => getJson<Record<string, unknown>>("/api/v1/watchdog/summary"),
  watchdogScan: async () => {
    const res = await fetch("/api/v1/watchdog/scan", { method: "POST", headers: { "Content-Type": "application/json" }, body: "{}" });
    if (!res.ok) throw new Error(await res.text());
    return res.json();
  },
  containers: () => getJson<{ id: string; image: string; host: string; status: string }[]>("/api/v1/containers"),
  containerStats: () => getJson<Record<string, unknown>>("/api/v1/containers/stats"),
  containersExplorer: (opts?: {
    env?: string;
    namespace?: string;
    host?: string;
    service?: string;
    q?: string;
  }) => {
    const p = new URLSearchParams();
    if (opts?.env) p.set("env", opts.env);
    if (opts?.namespace) p.set("namespace", opts.namespace);
    if (opts?.host) p.set("host", opts.host);
    if (opts?.service) p.set("service", opts.service);
    if (opts?.q) p.set("q", opts.q);
    const qs = p.toString();
    return getJson<ContainerExplorer>(`/api/v1/containers/explorer${qs ? `?${qs}` : ""}`);
  },
  infraEnvs: () => getJson<InfraEnvs>("/api/v1/infra/envs"),
  infraHostmap: (groupBy = "env", resource = "host") => {
    const qs = new URLSearchParams({
      group_by: groupBy,
      resource,
    });
    return getJson<InfraHostmap>(`/api/v1/infra/hostmap?${qs}`);
  },
  k8sUtilization: () => getJson<K8sUtilization>("/api/v1/infra/k8s/utilization"),
  serverless: () => getJson<ServerlessFunction[]>("/api/v1/serverless/functions"),
  serverlessOverview: () => getJson<ServerlessOverview>("/api/v1/serverless/overview"),
  logsSearch: async (
    opts?: number | { limit?: number; q?: string; service?: string; level?: string; live?: boolean },
  ) => {
    const o = typeof opts === "number" ? { limit: opts } : opts ?? {};
    const qs = new URLSearchParams();
    qs.set("limit", String(o.limit ?? 50));
    if (o.q) qs.set("q", o.q);
    if (o.service) qs.set("service", o.service);
    if (o.level) qs.set("level", o.level);
    if (o.live) qs.set("live", "true");
    const res = await getJson<{ logs: LogEvent[]; sample_rate: number; live: boolean; query?: string | null } | LogEvent[]>(
      `/api/v1/logs/search?${qs}`,
    );
    if (Array.isArray(res)) return { logs: res, sample_rate: 1, live: false };
    return res;
  },
  logFacets: () => getJson<{ key: string; values: [string, number][] }[]>("/api/v1/logs/facets"),
  errors: () =>
    getJson<
      {
        id: string;
        service: string;
        message: string;
        count: number;
        status?: string;
        assignee?: string | null;
        last_seen_ms?: number;
      }[]
    >("/api/v1/errors"),
  patchError: async (id: string, body: { status?: string; assignee?: string }) => {
    const res = await fetch(`/api/v1/errors/${encodeURIComponent(id)}`, {
      method: "PATCH",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify(body),
    });
    if (!res.ok) throw new Error(await res.text());
    return res.json();
  },
  sdsScan: () => getJson<{ rule_id: string; service: string; snippet: string }[]>("/api/v1/sds/scan"),
  pipelineWorkers: () => getJson<Record<string, unknown>>("/api/v1/pipelines/workers"),
  hosts: () => getJson<Record<string, unknown>[]>("/api/v1/infra/hosts"),
  gpuSummary: () => getJson<GpuSummary>("/api/v1/gpu/summary"),
  gpuDevices: () => getJson<GpuDevice[]>("/api/v1/gpu/devices"),
  gpuProcesses: (gpuId?: string) =>
    getJson<GpuProcess[]>(
      gpuId ? `/api/v1/gpu/processes?gpu_id=${encodeURIComponent(gpuId)}` : "/api/v1/gpu/processes",
    ),
  gpuSamples: (gpuId?: string, limit = 60) => {
    const qs = new URLSearchParams({ limit: String(limit) });
    if (gpuId) qs.set("gpu_id", gpuId);
    return getJson<GpuSample[]>(`/api/v1/gpu/samples?${qs}`);
  },
  gpuMetricsCatalog: () =>
    getJson<{
      count: number;
      groups: string[];
      metrics: { name: string; dcgm_field: string; unit: string; description: string; group: string }[];
    }>("/api/v1/gpu/metrics"),
  gpuHealth: () =>
    getJson<{
      devices_checked: number;
      healthy: number;
      degraded: number;
      findings: { gpu_id: string; severity: string; finding: string }[];
    }>("/api/v1/gpu/health"),
  gpuFleet: () => getJson<Record<string, unknown>>("/api/v1/gpu/fleet"),
  gpuEmit: async () => {
    const res = await fetch("/api/v1/gpu/emit", { method: "POST" });
    if (!res.ok) throw new Error(await res.text());
    return res.json() as Promise<{ points_emitted: number }>;
  },
  competitorsCompare: () => getJson<CompetitorsCompare>("/api/v1/compare"),
  datadogCompare: () => getJson<Record<string, unknown>>("/api/v1/compare/datadog"),

  notebooks: () => getJson<Notebook[]>("/api/v1/notebooks"),
  createNotebook: async (body: { title: string; cells: NotebookCell[] }) => {
    const res = await fetch("/api/v1/notebooks", {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify(body),
    });
    if (!res.ok) throw new Error(await res.text());
    return res.json() as Promise<Notebook>;
  },
  updateNotebook: async (id: string, body: { title?: string; cells?: NotebookCell[] }) => {
    const res = await fetch(`/api/v1/notebooks/${id}`, {
      method: "PUT",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify(body),
    });
    if (!res.ok) throw new Error(await res.text());
    return res.json() as Promise<Notebook>;
  },
  deleteNotebook: async (id: string) => {
    const res = await fetch(`/api/v1/notebooks/${id}`, { method: "DELETE" });
    if (!res.ok && res.status !== 204) throw new Error(await res.text());
  },
  apmTraces: (limit = 40) => getJson<ApmTraceSummary[]>(`/api/v1/apm/traces?limit=${limit}`),
  apmTrace: (id: string) => getJson<ApmTraceTree>(`/api/v1/apm/traces/${encodeURIComponent(id)}`),
  serviceMap: () => getJson<ServiceMap>("/api/v1/service-map"),
  alertEvents: () => getJson<AlertEvent[]>("/api/v1/alerts/events"),
  events: (limit = 50) => getJson<PlatformEvent[]>(`/api/v1/events?limit=${limit}`),
  uxOverview: () => getJson<UxOverview>("/api/v1/ux/overview"),
  uxSessions: () => getJson<UxSession[]>("/api/v1/ux/sessions"),
  uxVitals: () => getJson<UxVital[]>("/api/v1/ux/vitals"),
  uxSynthetics: () => getJson<UxSynthetic[]>("/api/v1/ux/synthetics"),
  uxTimeseries: () => getJson<UxTimeseriesPoint[]>("/api/v1/ux/timeseries"),
  uxPages: () => getJson<UxPage[]>("/api/v1/ux/pages"),
  uxFunnel: () => getJson<UxFunnelStep[]>("/api/v1/ux/funnel"),
  sloBudgets: () => getJson<Record<string, unknown>[]>("/api/v1/slos/budgets"),
  integrations: () => getJson<IntegrationTile[]>("/api/v1/integrations"),
  integrationsSearch: (q: string) =>
    getJson<IntegrationTile[]>(`/api/v1/integrations/search?q=${encodeURIComponent(q)}`),
  marketplace: () => getJson<{ id: string; name: string; installed: boolean }[]>("/api/v1/marketplace"),
  installApp: async (id: string) => {
    const res = await fetch(`/api/v1/marketplace/${encodeURIComponent(id)}/install`, { method: "POST" });
    if (!res.ok) throw new Error(await res.text());
    return res.json();
  },
  slos: () => getJson<SloEntry[]>("/api/v1/slos"),
  incidents: () => getJson<Incident[]>("/api/v1/incidents"),
  dora: () => getJson<DoraMetrics>("/api/v1/dora"),
  mcpTools: () => getJson<{ name: string; description: string }[]>("/api/v1/mcp/tools"),
  bitsChat: async (message: string) => {
    const res = await fetch("/api/v1/bits/chat", {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify({ message }),
    });
    if (!res.ok) throw new Error(await res.text());
    return res.json() as Promise<BitsReply>;
  },
  watchdogAnomalies: () => getJson<WatchdogAnomaly[]>("/api/v1/watchdog/anomalies"),
  usmDiscover: async () => {
    const res = await fetch("/api/v1/usm/discover", { method: "POST", headers: { "Content-Type": "application/json" }, body: "{}" });
    if (!res.ok) throw new Error(await res.text());
    return res.json();
  },
  updateBoard: async (
    id: string,
    body: {
      name?: string;
      description?: string;
      template_variables?: TemplateVariable[];
      widgets?: Array<{
        id?: string;
        type: string;
        title: string;
        metric?: string;
        aggregation?: string;
        group_by?: string;
        tags?: Tags;
        layout?: { x: number; y: number; w: number; h: number };
        text?: string;
        accent?: string;
        unit?: string;
        display?: string;
      }>;
    },
  ) => {
    const res = await fetch(`/api/v1/boards/${id}`, {
      method: "PUT",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify(body),
    });
    if (!res.ok) throw new Error(await res.text());
    return res.json();
  },
  getBoard: (id: string) =>
    getJson<BoardMeta & { widgets: unknown[]; template_variables?: TemplateVariable[] }>(
      `/api/v1/boards/${id}`,
    ),

  fleetAgents: () => getJson<FleetAgent[]>("/api/v1/fleet/agents"),
  fleetSummary: () => getJson<FleetSummary>("/api/v1/fleet/summary"),
  fleetHeartbeat: async (body: {
    id: string;
    version: string;
    host: string;
    platform?: string;
  }) => {
    const res = await fetch("/api/v1/fleet/heartbeat", {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify(body),
    });
    if (!res.ok) throw new Error(await res.text());
    return res.json() as Promise<FleetAgent>;
  },
  fleetConfigure: async (profile: string, agent_ids?: string[]) => {
    const res = await fetch("/api/v1/fleet/configure", {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify({ profile, agent_ids }),
    });
    if (!res.ok) throw new Error(await res.text());
    return res.json() as Promise<{
      profile: string;
      agents_updated: number;
      checks: string[];
      metrics_enabled: boolean;
      logs_enabled: boolean;
      apm_enabled: boolean;
    }>;
  },
  fleetBootstrap: async (body: {
    platform?: string;
    host?: string;
    version?: string;
    id?: string;
  }) => {
    const res = await fetch("/api/v1/fleet/bootstrap", {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify(body),
    });
    if (!res.ok) throw new Error(await res.text());
    return res.json() as Promise<{
      ok: boolean;
      agent: FleetAgent;
      message: string;
      next: string;
    }>;
  },
  fleetBootstrapStop: async (id: string) => {
    const res = await fetch("/api/v1/fleet/bootstrap/stop", {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify({ id }),
    });
    if (!res.ok) throw new Error(await res.text());
    return res.json() as Promise<{ ok: boolean; stopped?: string }>;
  },
  fleetRollout: async (target_version: string) => {
    const res = await fetch("/api/v1/fleet/rollout", {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify({ target_version }),
    });
    if (!res.ok) throw new Error(await res.text());
    return res.json() as Promise<{ target_version: string; agents_updated: number }>;
  },
};

export type FleetAgent = {
  id: string;
  version: string;
  host: string;
  status: string;
  platform?: string;
  last_seen_ms?: number;
  config_profile?: string;
  checks?: string[];
  metrics_enabled?: boolean;
  logs_enabled?: boolean;
  apm_enabled?: boolean;
};

export type FleetSummary = {
  total: number;
  healthy: number;
  configured: number;
  install_pct: number;
  platforms: Record<string, number>;
};

export type FeatureEntry = {
  id: string;
  name: string;
  category: string;
  status: string;
  endpoints: string[];
  notes: string;
  parity_pct: number;
  thine_strength: string;
  datadog_advantage: string;
  recommendation: string;
};

export type CloudResource = {
  id: string;
  name: string;
  kind: string;
  provider: string;
  region: string;
  vpc?: string | null;
  service?: string | null;
  cost_month: number;
  agent_installed: boolean;
  alert_status: string;
};

export type CloudcraftDiagram = {
  overlay: string;
  resource_count: number;
  groups: { group: string; resource_ids: string[]; count: number }[];
  nodes: CloudResource[];
  edges: { from: string; to: string; bytes?: number }[];
  overlays_available: string[];
};

export type LiveProcess = {
  pid: number;
  host: string;
  user: string;
  cmdline: string;
  cpu_pct: number;
  mem_rss_mb: number;
  service?: string | null;
  command?: string;
  env?: string;
  team?: string;
  az?: string;
  started_ms?: number;
};

export type ProcessRow = {
  pid: number;
  host: string;
  user: string;
  cmdline: string;
  command: string;
  cpu_pct: number;
  mem_rss_mb: number;
  service?: string | null;
  env: string;
  team: string;
  az?: string;
  started_ms: number;
};

export type ProcessExplorer = {
  count: number;
  returned: number;
  processes: ProcessRow[];
  command_groups: { command: string; count: number; avg_cpu_pct: number; avg_mem_rss_mb: number }[];
  facets: Record<string, { value: string; count: number }[]>;
  significance?: string;
};

export type ServerlessFunction = {
  name: string;
  runtime: string;
  invocations_24h: number;
  errors_24h: number;
  cold_starts_24h: number;
  avg_duration_ms: number;
  memory_mb: number;
  cloud?: string;
  region?: string;
  service?: string;
  env?: string;
  estimated_cost_24h?: number;
  timeout_errors_24h?: number;
  oom_errors_24h?: number;
  memory_used_pct?: number;
  kind?: string;
};

export type ServerlessOverview = {
  functions: ServerlessFunction[];
  totals: {
    functions: number;
    invocations_24h: number;
    errors_24h: number;
    cold_starts_24h: number;
    estimated_cost_24h: number;
    error_rate: number;
  };
  by_cloud: Record<string, number>;
  by_kind: Record<string, number>;
  significance?: string;
};

export type ContainerExplorerRow = {
  id: string;
  name: string;
  image: string;
  host: string;
  status: string;
  env: string;
  service: string;
  version: string;
  runtime: string;
  kube_namespace?: string | null;
  pod_name?: string | null;
  kube_deployment?: string | null;
  cpu_pct: number;
  cpu_limit: number;
  cpu_util_vs_limit_pct: number;
  mem_usage_mb: number;
  mem_limit_mb: number;
  mem_rss_mb: number;
  mem_util_vs_limit_pct: number;
  net_rx_bps: number;
  net_tx_bps: number;
  restarts: number;
  started_ms: number;
  over_provisioned: boolean;
  hot: boolean;
};

export type ContainerExplorer = {
  count: number;
  facets: Record<string, { value: string; count: number }[]>;
  containers: ContainerExplorerRow[];
  significance?: string;
};

export type InfraEnvRow = {
  env: string;
  hosts: number;
  containers: number;
  running: number;
  avg_host_cpu_pct: number;
  hot_containers: number;
  namespaces: number;
  services: number;
  mem_mib: number;
};

export type InfraEnvs = {
  envs: InfraEnvRow[];
  unified_tags: string[];
  significance?: string;
};

export type HostmapChild = {
  id: string;
  name: string;
  kind: string;
  container_id?: string;
  cpu_pct?: number;
  mem_pct?: number;
  readiness?: number;
  restarts?: number;
  status?: string;
  service?: string;
  env?: string;
  kube_namespace?: string | null;
  kube_deployment?: string | null;
  image?: string;
};

export type HostmapCell = {
  id: string;
  name: string;
  kind?: string;
  alias?: string;
  group: string;
  env: string;
  service: string;
  az: string;
  instance_type: string;
  cpu_pct: number;
  mem_pct?: number;
  memory_mib: number;
  disk_pct: number;
  load_15: number;
  cores: number;
  container_count: number;
  pod_count?: number;
  status: string;
  agent_version: string;
  fill: number;
  apps?: string[];
  tags?: Record<string, string>;
  error_logs?: number;
  readiness?: number;
  cost_score?: number;
  agent_outdated?: number;
  children?: HostmapChild[];
  host?: string;
  kube_namespace?: string | null;
  kube_deployment?: string | null;
};

export type HostmapSuggestedQuery = {
  id: string;
  title: string;
  resource: string;
  secondary: string;
  fill_by: string;
  size_by: string;
  group_by: string[];
  filter: string;
  secondary_fill?: string;
};

export type InfraHostmap = {
  resource?: string;
  resource_options?: string[];
  secondary_options?: string[];
  group_by: string;
  fill_by: string;
  fill_options?: string[];
  size_options?: string[];
  group_options?: string[];
  filter_tags?: string[];
  hosts: HostmapCell[];
  groups: Record<string, HostmapCell[]>;
  counts?: { host: number; pod: number; container: number; cluster: number };
  suggested_queries?: HostmapSuggestedQuery[];
  significance?: string;
};

export type K8sUtilRow = {
  kube_namespace: string;
  kube_deployment: string;
  pods: number;
  cpu_usage: number;
  cpu_requested: number;
  cpu_limit: number;
  cpu_usage_vs_request_pct: number;
  cpu_usage_vs_limit_pct: number;
  memory_usage_mb: number;
  memory_requested_mb: number;
  memory_limit_mb: number;
  memory_usage_vs_request_pct: number;
  memory_usage_vs_limit_pct: number;
  waste_hint: string;
};

export type K8sUtilization = {
  group_by: string[];
  rows: K8sUtilRow[];
  significance?: string;
};

export type UsmaRed = {
  service: string;
  protocol: string;
  requests_per_sec: number;
  error_rate: number;
  duration_p95_ms: number;
  detected_by: string;
  metric_names: string[];
};

export type ApmStat = {
  service: string;
  request_count: number;
  error_count: number;
  error_rate: number;
  p50_ms: number;
  p95_ms: number;
  p99_ms: number;
  avg_ms: number;
};

export type CatalogService = {
  name: string;
  team: string;
  tier: string;
  languages: string[];
  links: Record<string, string>;
  kind?: string;
  type?: string;
  lifecycle?: string | null;
  definition_yaml?: string | null;
};

export type ServiceScorecard = {
  service: string;
  score_pct: number;
  checks: { id: string; name: string; passed: boolean; weight: number; detail: string }[];
};

export type LogPattern = {
  id: string;
  template: string;
  count: number;
  services: string[];
  sample_message: string;
  first_seen_ms: number;
  last_seen_ms: number;
};

export type KubeMapNode = {
  id: string;
  kind: string;
  name: string;
  parent?: string | null;
  cpu_pct: number;
  mem_pct: number;
  status: string;
  restarts: number;
  service: string;
  env: string;
};

export type KubeContainerMap = {
  nodes: KubeMapNode[];
  counts: { namespaces: number; deployments: number; containers: number };
  fill_by: string[];
};

export type ExplainPlan = {
  query_fingerprint: string;
  db: string;
  total_cost: number;
  estimated_rows: number;
  plan: Record<string, unknown>;
};

export type DbSchema = {
  db: string;
  schema: string;
  table: string;
  columns: string[];
  indexes: string[];
  approx_rows: number;
};

export type DbQuery = {
  sql: string;
  duration_ms: number;
  calls: number;
  db: string;
};

export type DbInstance = {
  name: string;
  engine: string;
  qps: number;
  slow_queries: number;
  host: string;
  role: string;
  version: string;
  port: number;
  connections: number;
  active_connections: number;
  idle_connections: number;
  waiting_connections: number;
  max_connections: number;
  connections_pct: number;
  tps: number;
  rollbacks_per_sec: number;
  avg_query_ms: number;
  p95_query_ms: number;
  p99_query_ms: number;
  rows_returned_per_sec: number;
  rows_fetched_per_sec: number;
  rows_inserted_per_sec: number;
  rows_updated_per_sec: number;
  rows_deleted_per_sec: number;
  buffer_hit_ratio: number;
  blocks_hit_per_sec: number;
  blocks_read_per_sec: number;
  temp_bytes_per_sec: number;
  deadlocks_per_sec: number;
  locks_waiting: number;
  avg_lock_wait_ms: number;
  replication_lag_ms: number;
  disk_read_ops: number;
  disk_write_ops: number;
  cpu_pct: number;
  mem_used_pct: number;
  dead_rows: number;
  live_rows: number;
  autovacuum_workers: number;
  index_bloat_pct: number;
  table_bloat_pct: number;
  buffer_pool_utilization: number;
  buffer_pool_bytes: number;
  buffer_pool_dirty_bytes: number;
  tmp_tables_per_sec: number;
  tmp_disk_tables_per_sec: number;
  open_files: number;
  connection_errors_per_sec: number;
  uptime_hours: number;
  size_gb: number;
};

export type DbHostSample = {
  timestamp_ms: number;
  db: string;
  qps: number;
  tps: number;
  connections: number;
  active_connections: number;
  idle_connections: number;
  waiting_connections: number;
  avg_query_ms: number;
  p95_query_ms: number;
  rows_returned_per_sec: number;
  rows_affected_per_sec: number;
  buffer_hit_ratio: number;
  disk_read_ops: number;
  disk_write_ops: number;
  replication_lag_ms: number;
  deadlocks: number;
  locks_waiting: number;
  cpu_pct: number;
  mem_used_pct: number;
};

export type DbQueryMetric = {
  fingerprint: string;
  sql: string;
  db: string;
  engine: string;
  calls: number;
  requests_per_sec: number;
  avg_latency_ms: number;
  p95_latency_ms: number;
  max_latency_ms: number;
  total_time_ms: number;
  pct_time: number;
  rows_per_sec: number;
  rows_examined_per_sec: number;
  shared_blks_hit: number;
  shared_blks_read: number;
  shared_blks_dirtied: number;
  temp_blks_written: number;
  apm_service?: string | null;
};

export type DbWaitEvent = {
  db: string;
  event_type: string;
  event: string;
  waits: number;
  wait_time_ms: number;
  pct_wait_time: number;
};

export type DbBlocking = {
  db: string;
  blocked_pid: number;
  blocking_pid: number;
  blocked_sql: string;
  blocking_sql: string;
  wait_event: string;
  duration_ms: number;
};

export type DbActivity = {
  db: string;
  pid: number;
  user: string;
  application: string;
  client_addr: string;
  state: string;
  wait_event_type?: string | null;
  wait_event?: string | null;
  query: string;
  duration_ms: number;
};

export type DbMetricDef = {
  name: string;
  unit: string;
  description: string;
  group: string;
  engines: string[];
};

export type AiProject = {
  id: string;
  name: string;
  description: string;
  region: string;
  modality: string;
  provider: string;
  model: string;
  runs_24h: number;
  error_rate: number;
  avg_latency_ms: number;
  p95_latency_ms: number;
  tokens_per_sec: number;
  cost_usd_24h: number;
  pass_rate: number;
};

export type AiRun = {
  id: string;
  trace_id: string;
  parent_run_id?: string | null;
  project: string;
  name: string;
  run_type: string;
  status: string;
  start_ms: number;
  end_ms: number;
  latency_ms: number;
  ttft_ms?: number | null;
  model?: string | null;
  provider?: string | null;
  input_tokens: number;
  output_tokens: number;
  reasoning_tokens: number;
  cache_read_tokens: number;
  cache_write_tokens: number;
  cost_usd: number;
  error?: string | null;
  tool_name?: string | null;
  modality: string;
  tags: string[];
  input_preview: string;
  output_preview: string;
  trajectory: string[];
};

export type AiFeedback = {
  id: string;
  run_id: string;
  key: string;
  score: number;
  comment?: string | null;
  source: string;
  timestamp_ms: number;
};

export type AiDataset = {
  id: string;
  name: string;
  description: string;
  example_count: number;
  modality: string;
};

export type AiExample = {
  id: string;
  dataset_id: string;
  inputs: Record<string, unknown>;
  outputs: Record<string, unknown>;
  metadata: Record<string, unknown>;
};

export type AiExperiment = {
  id: string;
  name: string;
  dataset_id: string;
  project: string;
  status: string;
  trials_per_task: number;
  pass_at_k: number;
  pass_hat_k: number;
  avg_score: number;
  started_ms: number;
  finished_ms?: number | null;
};

export type AiEvalResult = {
  id: string;
  experiment_id: string;
  example_id: string;
  trial: number;
  pattern: string;
  grader: string;
  score: number;
  passed: boolean;
  latency_ms: number;
  detail: string;
};

export type AiGrader = {
  id: string;
  name: string;
  kind: string;
  description: string;
};

export type AiHostSample = {
  timestamp_ms: number;
  project: string;
  modality: string;
  requests: number;
  errors: number;
  latency_ms: number;
  ttft_ms: number;
  tokens_in: number;
  tokens_out: number;
  cost_usd: number;
  pass_rate: number;
};

export type AiMetricDef = {
  name: string;
  unit: string;
  description: string;
  group: string;
  modalities: string[];
};

export type StreamInfo = { name: string; lag: number; throughput: number };

export type DataLineage = {
  nodes: string[];
  edges: { from_asset: string; to_asset: string; kind: string }[];
  freshness_anomalies: number;
};

export type CostRec = {
  id: string;
  resource_id: string;
  title: string;
  savings_month: number;
  rationale: string;
};

export type CostLine = {
  service: string;
  compute: number;
  storage: number;
  network: number;
  total: number;
};

export type HaStatus = {
  mode: string;
  primary: string;
  quorum: boolean;
  failover_ready: boolean;
  rpo_seconds: number;
  rto_seconds: number;
  regions: {
    id: string;
    name: string;
    role: string;
    healthy: boolean;
    lag_ms: number;
    series_count?: number;
  }[];
};

export type LogEvent = {
  timestamp_ms: number;
  level: string;
  service: string;
  message: string;
};

export type CompetitorsCompare = {
  catalog_features: number;
  datadog_avg_parity_pct: number;
  averages: Record<string, number>;
  profiles: {
    name: string;
    positioning: string;
    deployment: string;
    license_model: string;
    otel_stance: string;
    strengths: string[];
    weaknesses: string[];
    best_when: string;
    url: string;
  }[];
  capabilities: {
    capability: string;
    category: string;
    thine: number;
    signoz: number;
    grafana: number;
    datadog: number;
    new_relic: number;
    cloudwatch: number;
    clickstack: number;
    dash0: number;
    note: string;
  }[];
  thine_category_leads: string[];
  vs_signoz: {
    summary: string;
    thine_wins: { capability: string; thine: number; signoz: number; delta: number }[];
    signoz_leads: { capability: string; thine: number; signoz: number; delta: number }[];
  };
  pick_guide: { choose: string; when: string }[];
  sources: string[];
};

export type GpuDevice = {
  id: string;
  uuid: string;
  model: string;
  host: string;
  pci_bus_id?: string;
  driver_version?: string;
  cuda_version?: string;
  gpu_util: number;
  sm_active: number;
  sm_occupancy: number;
  gr_engine_active: number;
  mem_copy_util: number;
  memory_used_mb: number;
  memory_free_mb: number;
  memory_total_mb: number;
  memory_reserved_mb: number;
  memory_used_percent: number;
  dram_active: number;
  power_usage_w: number;
  power_management_limit_w: number;
  total_energy_consumption_mj?: number;
  temperature_c: number;
  memory_temperature_c: number;
  fan_speed_pct: number;
  sm_clock_mhz: number;
  mem_clock_mhz: number;
  graphics_clock_mhz?: number;
  pstate?: number;
  pipe_tensor_active: number;
  pipe_fp16_active: number;
  pipe_fp32_active: number;
  pipe_fp64_active?: number;
  pipe_integer_active?: number;
  enc_utilization: number;
  dec_utilization: number;
  pcie_replay_total?: number;
  nvlink_bandwidth_total: number;
  nvlink_replay_errors?: number;
  xid_errors_total: number;
  remapped_rows_pending?: number;
  remapped_rows_failed?: number;
  remapped_rows_correctable?: number;
  remapped_rows_uncorrectable?: number;
  ecc_sbe_volatile_total?: number;
  ecc_dbe_volatile_total?: number;
  health_status: string;
  throttle_reasons?: Record<string, boolean>;
};

export type GpuProcess = {
  pid: number;
  gpu_id: string;
  host: string;
  process_name: string;
  pod?: string | null;
  namespace?: string | null;
  sm_active: number;
  memory_usage_mb: number;
  gpu_util: number;
  workload?: string | null;
};

export type GpuSample = {
  timestamp_ms: number;
  gpu_id: string;
  sm_active: number;
  sm_occupancy: number;
  gpu_util: number;
  memory_used_percent: number;
  power_usage_w: number;
  temperature_c: number;
  pipe_tensor_active: number;
};

export type GpuSummary = {
  devices: number;
  processes: number;
  avg_util: number;
  avg_sm_active: number;
  avg_sm_occupancy: number;
  avg_memory_used_percent: number;
  total_memory_mb: number;
  total_memory_capacity_mb: number;
  total_power_w: number;
  avg_temperature_c: number;
  throttled_devices: number;
  unhealthy_devices: number;
  xid_errors_total: number;
  metrics_catalog_count: number;
  devices_detail: GpuDevice[];
};

export type NotebookCell = {
  kind: "markdown" | "metric" | "query" | string;
  content: string;
};

export type Notebook = {
  id: string;
  title: string;
  cells: NotebookCell[];
  updated_at_ms: number;
  share_token?: string | null;
};

export type ApmTraceSummary = {
  trace_id: string;
  span_id: string;
  service: string;
  name: string;
  duration_ms: number;
  timestamp_ms: number;
  status: string;
};

export type ApmSpan = ApmTraceSummary & {
  parent_id?: string | null;
  resource?: string;
};

export type ApmTraceTree = {
  trace_id: string;
  root: string;
  spans: ApmSpan[];
  total_duration_ms: number;
  service_count: number;
};

export type ServiceMap = {
  detected_via: string[];
  nodes: { id: string; error_rate: number; p95_ms: number; requests: number }[];
  edges: { from: string; to: string; protocol: string; bytes: number }[];
};

export type AlertEvent = {
  id: string;
  rule_id: string;
  rule_name: string;
  metric: string;
  value: number;
  threshold: number;
  fired_at: string;
  status: string;
};

export type PlatformEvent = {
  timestamp_ms: number;
  title: string;
  text: string;
  alert_type: string;
  tags: Record<string, string>;
  source: string;
};

export type UxOverview = {
  sessions_24h: number;
  active_users: number;
  bounce_rate: number;
  error_sessions: number;
  avg_session_ms: number;
  views_per_session: number;
  web_vitals_ok_pct: number;
  conversion_rate?: number;
  revenue_at_risk_usd?: number;
  significance?: string;
  synthetics_ok: number;
  synthetics_total: number;
  lcp_p75_ms: number;
  cls_p75: number;
  inp_p75_ms: number;
};

export type UxSession = {
  id: string;
  user: string;
  view: string;
  country: string;
  device: string;
  browser: string;
  duration_ms: number;
  bounced: boolean;
  has_error: boolean;
  frustration?: string | null;
  impact?: string;
  started_ms: number;
};

export type UxVital = {
  name: string;
  p75_ms?: number;
  p75?: number;
  good_pct: number;
  needs_improvement_pct: number;
  poor_pct: number;
  budget_ms?: number;
  budget?: number;
  significance?: string;
};

export type UxSynthetic = {
  id: string;
  name: string;
  type: string;
  url: string;
  locations: string[];
  status: string;
  latency_ms: number;
  last_run_ms: number;
  significance?: string;
};

export type UxTimeseriesPoint = {
  t: number;
  label: string;
  sessions: number;
  errors: number;
  bounce_pct: number;
  lcp_p75_ms: number;
};

export type UxPage = {
  path: string;
  views: number;
  avg_lcp_ms: number;
  error_rate: number;
  bounce_rate: number;
  impact: string;
  note: string;
};

export type UxFunnelStep = {
  step: string;
  users: number;
  drop_pct: number;
};

export type IntegrationTile = {
  id: string;
  title: string;
  enabled: boolean;
  category: string;
};

export type SloEntry = {
  id: string;
  name: string;
  metric: string;
  target: number;
  window_ms: number;
  tags: Record<string, string>;
  current_pct: number;
  budget_left: number;
  status: string;
};

export type Incident = {
  id: string;
  title: string;
  severity: string;
  status: string;
  service: string;
  created_at_ms: number;
};

export type DoraMetrics = {
  deploy_frequency_per_day: number;
  lead_time_hours: number;
  change_failure_rate: number;
  mttr_hours: number;
  ai_impact_score: number;
};

export type BitsAction = {
  type: string;
  page?: string;
  board_id?: string;
  notebook_id?: string;
  platform?: string;
  id?: string;
};

export type BitsReply = {
  reply: string;
  role: string;
  intent?: string;
  model?: string;
  citations?: string[];
  actions?: BitsAction[];
  artifacts?: Record<string, unknown> | null;
};

export type WatchdogAnomaly = {
  id?: string;
  metric?: string;
  service?: string;
  severity?: string;
  score?: number;
  z_score?: number;
  baseline?: number;
  observed?: number;
  message?: string;
  summary?: string;
  tags?: Record<string, string>;
  detected_at_ms?: number;
  [key: string]: unknown;
};
