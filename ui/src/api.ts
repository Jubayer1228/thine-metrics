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

async function getJson<T>(path: string): Promise<T> {
  const res = await fetch(path);
  if (!res.ok) throw new Error(`${res.status} ${res.statusText}`);
  return res.json();
}

export const api = {
  dashboard: () => getJson<Dashboard>("/api/v1/dashboard"),
  metrics: (prefix?: string) =>
    getJson<MetricMeta[]>(
      prefix ? `/api/v1/metrics?prefix=${encodeURIComponent(prefix)}` : "/api/v1/metrics",
    ),
  query: async (params: {
    metric: string;
    service?: string;
    env?: string;
    aggregation?: string;
    start_ms?: number;
    end_ms?: number;
    step_ms?: number;
  }) => {
    const qs = new URLSearchParams();
    qs.set("metric", params.metric);
    if (params.service) qs.set("service", params.service);
    if (params.env) qs.set("env", params.env);
    if (params.aggregation) qs.set("aggregation", params.aggregation);
    if (params.start_ms) qs.set("start_ms", String(params.start_ms));
    if (params.end_ms) qs.set("end_ms", String(params.end_ms));
    if (params.step_ms) qs.set("step_ms", String(params.step_ms));
    const data = await getJson<{ results: QueryResult[] }>(`/api/v1/query?${qs}`);
    return data.results;
  },
  alerts: () => getJson<AlertRule[]>("/api/v1/alerts"),
  createAlert: async (body: {
    name: string;
    metric: string;
    threshold: number;
    comparator: string;
    window_ms?: number;
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
