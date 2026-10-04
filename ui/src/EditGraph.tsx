import { useEffect, useMemo, useState } from "react";
import {
  CartesianGrid,
  Line,
  LineChart,
  ResponsiveContainer,
  Tooltip,
  XAxis,
  YAxis,
} from "recharts";
import { api, type QueryResult } from "./api";

export type GraphDraft = {
  id?: string;
  type: string;
  title: string;
  metric: string;
  aggregation: string;
  group_by?: string;
  unit?: string;
  display?: string;
  accent?: string;
};

const VIZ: { id: string; label: string }[] = [
  { id: "timeseries", label: "Timeseries" },
  { id: "query_value", label: "Query Value" },
  { id: "table", label: "Table" },
  { id: "heatmap", label: "Heatmap" },
  { id: "scatter_plot", label: "Scatter Plot" },
  { id: "distribution", label: "Distribution" },
  { id: "toplist", label: "Top List" },
  { id: "bar", label: "Bar Chart" },
  { id: "slo", label: "SLO" },
  { id: "hostmap", label: "Host Map" },
  { id: "pie", label: "Pie Chart" },
  { id: "treemap", label: "Tree Map" },
];

const AGGS = ["avg", "sum", "min", "max", "count"] as const;

type Props = {
  open: boolean;
  metrics: string[];
  rangeMs: number;
  initial?: GraphDraft | null;
  onClose: () => void;
  onSave: (draft: GraphDraft) => void;
};

export function EditGraphModal({ open, metrics, rangeMs, initial, onClose, onSave }: Props) {
  const [viz, setViz] = useState(initial?.type || "timeseries");
  const [title, setTitle] = useState(initial?.title || "Untitled graph");
  const [metric, setMetric] = useState(initial?.metric || metrics[0] || "system.cpu.user");
  const [agg, setAgg] = useState(initial?.aggregation || "avg");
  const [groupBy, setGroupBy] = useState(initial?.group_by || "");
  const [display, setDisplay] = useState(initial?.display || "lines");
  const [preview, setPreview] = useState<QueryResult[]>([]);
  const [tab, setTab] = useState<"edit" | "json">("edit");

  useEffect(() => {
    if (!open) return;
    setViz(initial?.type || "timeseries");
    setTitle(initial?.title || "Untitled graph");
    setMetric(initial?.metric || metrics[0] || "system.cpu.user");
    setAgg(initial?.aggregation || "avg");
    setGroupBy(initial?.group_by || "");
    setDisplay(initial?.display || "lines");
    setTab("edit");
  }, [open, initial, metrics]);

  useEffect(() => {
    if (!open || !metric) return;
    const end = Date.now();
    const start = end - rangeMs;
    const step = rangeMs <= 60 * 60_000 ? 30_000 : 60_000;
    api
      .query({
        metric,
        aggregation: agg,
        start_ms: start,
        end_ms: end,
        step_ms: step,
        group_by: groupBy || undefined,
      })
      .then(setPreview)
      .catch(() => setPreview([]));
  }, [open, metric, agg, groupBy, rangeMs]);

  const chartData = useMemo(() => {
    const map = new Map<number, Record<string, number | string>>();
    preview.forEach((s, i) => {
      const key = Object.entries(s.tags)
        .map(([k, v]) => `${k}:${v}`)
        .join(",") || `s${i}`;
      s.points.forEach((p) => {
        const row = map.get(p.timestamp_ms) ?? {
          t: p.timestamp_ms,
          label: new Date(p.timestamp_ms).toLocaleTimeString([], {
            hour: "2-digit",
            minute: "2-digit",
          }),
        };
        row[key] = p.value;
        map.set(p.timestamp_ms, row);
      });
    });
    return Array.from(map.values()).sort((a, b) => Number(a.t) - Number(b.t));
  }, [preview]);

  const keys = useMemo(() => {
    const set = new Set<string>();
    for (const row of chartData) {
      for (const k of Object.keys(row)) {
        if (k !== "t" && k !== "label") set.add(k);
      }
    }
    return [...set];
  }, [chartData]);

  if (!open) return null;

  const draft: GraphDraft = {
    id: initial?.id,
    type: viz,
    title: title.trim() || metric,
    metric,
    aggregation: agg,
    group_by: groupBy || undefined,
    display: viz === "timeseries" ? display : undefined,
    unit: initial?.unit,
    accent: initial?.accent,
  };

  return (
    <div className="dd-modal-backdrop" onClick={onClose}>
      <div className="edit-graph-modal" role="dialog" aria-labelledby="edit-graph-title" onClick={(e) => e.stopPropagation()}>
        <header>
          <h2 id="edit-graph-title">Edit Graph</h2>
          <button type="button" className="dd-modal-close" aria-label="Close" onClick={onClose}>
            ×
          </button>
        </header>

        <div className="eg-preview">
          <ResponsiveContainer width="100%" height="100%">
            <LineChart data={chartData} margin={{ top: 8, right: 12, left: 0, bottom: 0 }}>
              <CartesianGrid stroke="rgba(0,0,0,0.06)" vertical={false} />
              <XAxis dataKey="label" tick={{ fill: "#6b7785", fontSize: 10 }} minTickGap={28} />
              <YAxis tick={{ fill: "#6b7785", fontSize: 10 }} width={42} />
              <Tooltip />
              {keys.map((k, i) => (
                <Line
                  key={k}
                  type="monotone"
                  dataKey={k}
                  stroke={["#632ca6", "#3d8bfd", "#e07a3d", "#2ec4b6"][i % 4]}
                  strokeWidth={2}
                  dot={false}
                  isAnimationActive={false}
                />
              ))}
            </LineChart>
          </ResponsiveContainer>
          {!chartData.length && <div className="eg-empty">No data in range — pick another metric</div>}
        </div>

        <section className="eg-section">
          <h3>1. Select your visualization</h3>
          <div className="eg-viz-grid">
            {VIZ.map((v) => (
              <button
                key={v.id}
                type="button"
                className={viz === v.id ? "on" : ""}
                onClick={() => setViz(v.id)}
              >
                {v.label}
              </button>
            ))}
          </div>
        </section>

        <section className="eg-section">
          <div className="eg-tabs">
            <button type="button" className={tab === "edit" ? "on" : ""} onClick={() => setTab("edit")}>
              Edit
            </button>
            <button type="button" className={tab === "json" ? "on" : ""} onClick={() => setTab("json")}>
              JSON
            </button>
          </div>
          <h3>2. Graph your data</h3>
          {tab === "edit" ? (
            <>
              <label className="eg-title">
                Title
                <input value={title} onChange={(e) => setTitle(e.target.value)} />
              </label>
              <div className="eg-query-row">
                <span className="eg-q">a</span>
                <select value="metrics" disabled>
                  <option value="metrics">Metrics</option>
                </select>
                <select value={metric} onChange={(e) => setMetric(e.target.value)}>
                  {(metrics.includes(metric) ? metrics : [metric, ...metrics]).map((m) => (
                    <option key={m} value={m}>
                      {m}
                    </option>
                  ))}
                </select>
                <span className="muted">from</span>
                <code>(everywhere)</code>
                <select value={agg} onChange={(e) => setAgg(e.target.value)}>
                  {AGGS.map((a) => (
                    <option key={a} value={a}>
                      {a} by
                    </option>
                  ))}
                </select>
                <input
                  placeholder="group by (service, host…)"
                  value={groupBy}
                  onChange={(e) => setGroupBy(e.target.value)}
                />
              </div>
              {viz === "timeseries" && (
                <div className="eg-display row">
                  <label>
                    Display
                    <select value={display} onChange={(e) => setDisplay(e.target.value)}>
                      <option value="lines">Lines</option>
                      <option value="areas">Areas</option>
                      <option value="bars">Bars</option>
                    </select>
                  </label>
                </div>
              )}
            </>
          ) : (
            <pre className="eg-json">{JSON.stringify(draft, null, 2)}</pre>
          )}
        </section>

        <footer>
          <button type="button" className="ghost" onClick={onClose}>
            Cancel
          </button>
          <button
            type="button"
            onClick={() => {
              onSave(draft);
              onClose();
            }}
          >
            Save
          </button>
        </footer>
      </div>
    </div>
  );
}
