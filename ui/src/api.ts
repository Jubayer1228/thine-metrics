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
  tags: Tags;
  threshold: number;
  comparator: string;
  window_ms: number;
  enabled: boolean;
};

export type BoardMeta = {
  id: string;
  name: string;
  description?: string | null;
  widgets: { id: string; type: string; title: string; metric: string }[];
};

export type RenderedWidget = {
  id: string;
  type: "timeseries" | "query_value" | "toplist" | "note" | "group";
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
};

export type RenderedBoard = {
  id: string;
  name: string;
  description?: string | null;
  range_ms: number;
  start_ms: number;
  end_ms: number;
  widgets: RenderedWidget[];
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
  renderBoard: (id: string, range_ms: number, tags?: string) => {
    const qs = new URLSearchParams({ range_ms: String(range_ms) });
    if (tags) qs.set("tags", tags);
    return getJson<RenderedBoard>(`/api/v1/boards/${id}/render?${qs}`);
  },
  createAlert: async (body: {
    name: string;
    metric: string;
    threshold: number;
    comparator: string;
    window_ms?: number;
    tags?: Tags;
  }) => {
    const res = await fetch("/api/v1/alerts", {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify(body),
    });
    if (!res.ok) throw new Error(await res.text());
    return res.json();
  },
};
