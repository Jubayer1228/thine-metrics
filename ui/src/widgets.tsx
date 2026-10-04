import type { CSSProperties, ReactNode } from "react";
import {
  Area,
  AreaChart,
  Bar,
  BarChart,
  CartesianGrid,
  Cell,
  Line,
  LineChart,
  ResponsiveContainer,
  Scatter,
  ScatterChart,
  Tooltip,
  XAxis,
  YAxis,
} from "recharts";
import type { RenderedWidget, Sample } from "./api";
import { chartGrid, chartTip } from "./theme";

const SERIES_COLORS = [
  "#4285F4",
  "#34A853",
  "#FBBC04",
  "#EA4335",
  "#A142F4",
  "#24C1E0",
  "#F538A0",
  "#FF6D01",
];

const tip = chartTip;
const tick = { fill: "var(--surf-muted)", fontSize: 10 } as const;

export function fmt(n: number | undefined | null, digits = 2) {
  if (n == null || Number.isNaN(n)) return "—";
  if (Math.abs(n) >= 1_000_000) return `${(n / 1_000_000).toFixed(2)}M`;
  if (Math.abs(n) >= 1000) return n.toLocaleString(undefined, { maximumFractionDigits: 1 });
  return n.toFixed(digits);
}

function tagStr(tags: Record<string, string>) {
  const pairs = Object.entries(tags).filter(([k]) => !k.startsWith("telemetry."));
  return pairs.length ? pairs.map(([k, v]) => `${k}:${v}`).join(",") : "*";
}

function sparkData(points: Sample[]) {
  return points.map((p) => ({ v: p.value, t: p.timestamp_ms }));
}

function normalizeDisplay(display: string | undefined, type: string): "bars" | "area" | "line" {
  const d = (display || (type === "heatmap" ? "area" : "line")).toLowerCase();
  if (d === "bars" || d === "bar") return "bars";
  if (d === "area" || d === "areas") return "area";
  return "line";
}

function accentColor(accent: string | null | undefined): string {
  switch (accent) {
    case "green":
      return "#34A853";
    case "red":
      return "#EA4335";
    case "orange":
      return "#FF6D01";
    case "purple":
      return "#A142F4";
    case "yellow":
      return "#FBBC04";
    default:
      return "#4285F4";
  }
}

export function gridStyle(w: RenderedWidget): CSSProperties {
  const { x, y, w: width, h } = w.layout;
  return {
    gridColumn: `${x + 1} / span ${Math.max(1, width)}`,
    gridRow: `${y + 1} / span ${Math.max(1, h)}`,
  };
}

export function Sparkline({
  points,
  color = "#4285F4",
  height = "100%",
}: {
  points: Sample[];
  color?: string;
  height?: number | string;
}) {
  if (!points.length) return <div className="spark empty" />;
  const gid = `spark-${color.replace("#", "")}`;
  return (
    <div className="spark" style={{ height }}>
      <ResponsiveContainer width="100%" height="100%">
        <AreaChart data={sparkData(points)} margin={{ top: 2, right: 0, left: 0, bottom: 0 }}>
          <defs>
            <linearGradient id={gid} x1="0" y1="0" x2="0" y2="1">
              <stop offset="0%" stopColor={color} stopOpacity={0.35} />
              <stop offset="100%" stopColor={color} stopOpacity={0.02} />
            </linearGradient>
          </defs>
          <Area
            type="monotone"
            dataKey="v"
            stroke={color}
            fill={`url(#${gid})`}
            strokeWidth={2}
            dot={false}
            isAnimationActive={false}
          />
        </AreaChart>
      </ResponsiveContainer>
    </div>
  );
}

function ChartShell({ children }: { children: ReactNode }) {
  return <div className="dw-chart">{children}</div>;
}

function SeriesChart({
  w,
  data,
  keys,
  display,
}: {
  w: RenderedWidget;
  data: Record<string, number | string>[];
  keys: string[];
  display: "bars" | "area" | "line";
}) {
  const margin = { top: 10, right: 10, left: 0, bottom: 4 };
  if (display === "bars") {
    return (
      <ResponsiveContainer width="100%" height="100%">
        <BarChart data={data} margin={margin}>
          <CartesianGrid stroke={chartGrid} vertical={false} strokeDasharray="3 6" />
          <XAxis dataKey="label" tick={tick} minTickGap={28} axisLine={false} tickLine={false} />
          <YAxis tick={tick} width={44} axisLine={false} tickLine={false} />
          <Tooltip contentStyle={tip} cursor={{ fill: "var(--surf-hover)" }} />
          {keys.map((k, i) => (
            <Bar
              key={k}
              dataKey={k}
              fill={SERIES_COLORS[i % SERIES_COLORS.length]}
              radius={[4, 4, 0, 0]}
              maxBarSize={28}
            />
          ))}
        </BarChart>
      </ResponsiveContainer>
    );
  }

  if (display === "area") {
    return (
      <ResponsiveContainer width="100%" height="100%">
        <AreaChart data={data} margin={margin}>
          <defs>
            {keys.map((k, i) => {
              const c = SERIES_COLORS[i % SERIES_COLORS.length];
              return (
                <linearGradient key={k} id={`area-${w.id}-${i}`} x1="0" y1="0" x2="0" y2="1">
                  <stop offset="0%" stopColor={c} stopOpacity={0.4} />
                  <stop offset="100%" stopColor={c} stopOpacity={0.02} />
                </linearGradient>
              );
            })}
          </defs>
          <CartesianGrid stroke={chartGrid} vertical={false} strokeDasharray="3 6" />
          <XAxis dataKey="label" tick={tick} minTickGap={28} axisLine={false} tickLine={false} />
          <YAxis tick={tick} width={44} axisLine={false} tickLine={false} />
          <Tooltip contentStyle={tip} />
          {keys.map((k, i) => (
            <Area
              key={k}
              type="monotone"
              dataKey={k}
              stroke={SERIES_COLORS[i % SERIES_COLORS.length]}
              fill={`url(#area-${w.id}-${i})`}
              strokeWidth={2.25}
              strokeLinecap="round"
              dot={false}
              isAnimationActive={false}
            />
          ))}
        </AreaChart>
      </ResponsiveContainer>
    );
  }

  return (
    <ResponsiveContainer width="100%" height="100%">
      <LineChart data={data} margin={margin}>
        <CartesianGrid stroke={chartGrid} vertical={false} strokeDasharray="3 6" />
        <XAxis dataKey="label" tick={tick} minTickGap={28} axisLine={false} tickLine={false} />
        <YAxis tick={tick} width={44} axisLine={false} tickLine={false} />
        <Tooltip contentStyle={tip} />
        {keys.map((k, i) => (
          <Line
            key={k}
            type="monotone"
            dataKey={k}
            stroke={SERIES_COLORS[i % SERIES_COLORS.length]}
            strokeWidth={2.25}
            strokeLinecap="round"
            dot={false}
            activeDot={{ r: 3.5, strokeWidth: 0 }}
            isAnimationActive={false}
          />
        ))}
      </LineChart>
    </ResponsiveContainer>
  );
}

function buildSeriesData(w: RenderedWidget) {
  const keys = w.series.map((s, i) => tagStr(s.tags) || `series-${i}`);
  const map = new Map<number, Record<string, number | string>>();
  w.series.forEach((s, i) => {
    const key = keys[i];
    s.points.forEach((p) => {
      const row = map.get(p.timestamp_ms) ?? {
        t: p.timestamp_ms,
        label: new Date(p.timestamp_ms).toLocaleTimeString([], { hour: "2-digit", minute: "2-digit" }),
      };
      row[key] = p.value;
      map.set(p.timestamp_ms, row);
    });
  });
  const data = Array.from(map.values()).sort((a, b) => Number(a.t) - Number(b.t));
  return { keys, data };
}

function buildHeatmapBuckets(w: RenderedWidget) {
  return w.series.slice(0, 12).map((s, i) => {
    const label = tagStr(s.tags) || `series-${i}`;
    const step = Math.max(1, Math.floor(s.points.length / 24));
    const cells: number[] = [];
    for (let j = 0; j < s.points.length; j += step) {
      const slice = s.points.slice(j, j + step);
      cells.push(slice.reduce((a, p) => a + p.value, 0) / Math.max(1, slice.length));
    }
    const max = Math.max(...cells, 1);
    return { label, cells: cells.slice(0, 24), max };
  });
}

function heatColor(v: number, max: number) {
  const t = Math.min(1, Math.max(0, v / max));
  const r = Math.round(66 + t * (234 - 66));
  const g = Math.round(133 - t * (133 - 67));
  const b = Math.round(244 - t * (244 - 53));
  return `rgb(${r},${g},${b})`;
}

function buildScatterPoints(w: RenderedWidget) {
  if (w.series.length >= 2) {
    const a = w.series[0].points;
    const b = w.series[1].points;
    const n = Math.min(a.length, b.length);
    return Array.from({ length: n }, (_, i) => ({
      x: a[i].value,
      y: b[i].value,
      z: 1,
    }));
  }
  const s = w.series[0];
  if (!s) return [];
  return s.points.map((p, i) => ({
    x: i,
    y: p.value,
    z: 1,
  }));
}

function HostmapMini({ w }: { w: RenderedWidget }) {
  const cells = w.series.map((s, i) => {
    const last = s.points[s.points.length - 1]?.value ?? 0;
    const label = tagStr(s.tags) || `host-${i}`;
    return { label, value: last, color: SERIES_COLORS[i % SERIES_COLORS.length] };
  });
  const max = Math.max(...cells.map((c) => c.value), 1);
  return (
    <div className="dw-hostmap">
      {cells.map((c) => {
        const intensity = 0.2 + (c.value / max) * 0.8;
        return (
          <div
            key={c.label}
            className="dw-host-cell"
            title={`${c.label}: ${fmt(c.value)}`}
            style={{
              background: c.color,
              opacity: intensity,
            }}
          >
            <span>{c.label.split(":").pop() || c.label}</span>
            <strong>{fmt(c.value, 1)}</strong>
          </div>
        );
      })}
      {!cells.length && <div className="dd-empty">No hosts</div>}
    </div>
  );
}

function SloGauge({ value }: { value: number | null | undefined }) {
  const pct = value == null ? 0 : value > 1 ? Math.min(100, value) : value * 100;
  const ok = pct >= 99;
  const warn = pct >= 95 && pct < 99;
  const color = ok ? "#34A853" : warn ? "#FBBC04" : "#EA4335";
  return (
    <div className="dw-slo">
      <div
        className="dw-slo-ring"
        style={{
          background: `conic-gradient(${color} ${pct}%, var(--surf-bar-track) ${pct}% 100%)`,
        }}
      >
        <div className="dw-slo-hole">
          <strong>{fmt(pct, 2)}%</strong>
          <span>SLO</span>
        </div>
      </div>
    </div>
  );
}

export function WidgetCard({
  w,
  onCorrelate,
  layoutOnRoot = true,
}: {
  w: RenderedWidget;
  onCorrelate?: () => void;
  /** When false, parent applies grid placement (preferred). */
  layoutOnRoot?: boolean;
}) {
  const style = layoutOnRoot ? gridStyle(w) : undefined;

  if (w.type === "group") {
    const slug = w.title.toLowerCase().replace(/[^a-z0-9]+/g, "-").replace(/^-|-$/g, "");
    return (
      <div className={`dw group-banner section-${slug}`} style={style}>
        <h2>{w.title}</h2>
      </div>
    );
  }

  if (w.type === "note" || w.type === "event_stream") {
    const lines = (w.text || "").split("\n").filter(Boolean);
    return (
      <div className={`dw note-stream ${w.type}`} style={style}>
        <div className="dw-title">{w.title}</div>
        {w.type === "event_stream" ? (
          <ul className="dw-events">
            {(lines.length ? lines : ["No recent events"]).map((line, i) => (
              <li key={i}>
                <time>{new Date(Date.now() - i * 120_000).toLocaleTimeString([], { hour: "2-digit", minute: "2-digit" })}</time>
                <span>{line}</span>
              </li>
            ))}
          </ul>
        ) : (
          <pre className="note-body">{w.text || ""}</pre>
        )}
      </div>
    );
  }

  if (w.type === "slo") {
    return (
      <div className="dw query-value slo" style={style}>
        <div className="dw-title">{w.title}</div>
        <SloGauge value={w.value} />
        <div className={`dw-change ${(w.change_pct ?? 0) >= 0 ? "up" : "down"}`}>
          {w.change_pct == null ? "Target window" : `${fmt(Math.abs(w.change_pct), 2)}% vs previous`}
        </div>
      </div>
    );
  }

  if (w.type === "query_value" || w.type === "change" || w.type === "check_status" || w.type === "alert_graph") {
    const up = (w.change_pct ?? 0) > 0;
    const down = (w.change_pct ?? 0) < 0;
    const accent = w.accent || inferAccent(w.title);
    const color = accentColor(accent);
    const displayValue =
      w.unit === "%" && w.value != null ? fmt(w.value > 1 ? w.value : w.value * 100, 1) : fmt(w.value);
    return (
      <div className={`dw query-value ${w.type}${accent ? ` accent-${accent}` : ""}`} style={style}>
        <div className="dw-kpi-head">
          <div className="dw-title">{w.title}</div>
          <div className={`dw-change ${up ? "up" : down ? "down" : ""}`}>
            {w.change_pct == null
              ? "—"
              : `${up ? "↑" : down ? "↓" : "→"} ${fmt(Math.abs(w.change_pct), 1)}%`}
          </div>
        </div>
        <div className="dw-value">
          {displayValue}
          {w.unit && w.unit !== "1" && w.unit !== "%" && <small>{w.unit}</small>}
          {w.unit === "%" && <small>%</small>}
        </div>
        <Sparkline points={w.sparkline} color={color} />
      </div>
    );
  }

  if (w.type === "table") {
    return (
      <div className="dw table" style={style}>
        <div className="dw-title">{w.title}</div>
        <div className="dw-table-wrap">
          <table className="dw-table">
            <thead>
              <tr>
                <th>Name</th>
                <th>Value</th>
                <th>Share</th>
              </tr>
            </thead>
            <tbody>
              {w.toplist.map((item, i) => {
                const max = Math.max(...w.toplist.map((x) => x.value), 1);
                return (
                  <tr key={item.label}>
                    <td>
                      <i style={{ background: SERIES_COLORS[i % SERIES_COLORS.length] }} />
                      {item.label}
                    </td>
                    <td>
                      {fmt(item.value)}
                      {w.unit ? ` ${w.unit}` : ""}
                    </td>
                    <td>
                      <div className="top-bar">
                        <b
                          style={{
                            width: `${(item.value / max) * 100}%`,
                            background: SERIES_COLORS[i % SERIES_COLORS.length],
                          }}
                        />
                      </div>
                    </td>
                  </tr>
                );
              })}
              {!w.toplist.length && (
                <tr>
                  <td colSpan={3} className="muted">
                    No rows
                  </td>
                </tr>
              )}
            </tbody>
          </table>
        </div>
      </div>
    );
  }

  if (w.type === "toplist" || w.type === "funnel" || w.type === "list_stream") {
    const max = Math.max(...w.toplist.map((i) => i.value), 1);
    return (
      <div className={`dw toplist ${w.type}`} style={style}>
        <div className="dw-title">{w.title}</div>
        <ul>
          {w.toplist.map((item, i) => (
            <li key={item.label}>
              <div className="top-row">
                <span>
                  <i style={{ background: SERIES_COLORS[i % SERIES_COLORS.length] }} />
                  {item.label}
                </span>
                <strong>
                  {fmt(item.value)}
                  {w.unit ? ` ${w.unit}` : ""}
                </strong>
              </div>
              <div className="top-bar">
                <b
                  style={{
                    width: `${(item.value / max) * 100}%`,
                    background: SERIES_COLORS[i % SERIES_COLORS.length],
                  }}
                />
              </div>
            </li>
          ))}
          {!w.toplist.length && <li className="muted">No series</li>}
        </ul>
      </div>
    );
  }

  if (w.type === "pie_chart") {
    const total = w.toplist.reduce((s, i) => s + i.value, 0) || 1;
    let acc = 0;
    const stops = w.toplist.map((item, i) => {
      const start = (acc / total) * 100;
      acc += item.value;
      const end = (acc / total) * 100;
      return `${SERIES_COLORS[i % SERIES_COLORS.length]} ${start}% ${end}%`;
    });
    return (
      <div className="dw pie" style={style}>
        <div className="dw-title">{w.title}</div>
        <div className="pie-body">
          <div className="pie-ring" style={{ background: `conic-gradient(${stops.join(", ") || "#4285F4 0 100%"})` }} />
          <ul>
            {w.toplist.map((item, i) => (
              <li key={item.label}>
                <i style={{ background: SERIES_COLORS[i % SERIES_COLORS.length] }} />
                {item.label}
                <strong>{fmt(item.value)}</strong>
              </li>
            ))}
          </ul>
        </div>
      </div>
    );
  }

  if (w.type === "hostmap") {
    return (
      <div className="dw timeseries hostmap" style={style}>
        <div className="dw-title">
          {w.title}
          <span>{w.series.length} hosts</span>
        </div>
        <HostmapMini w={w} />
      </div>
    );
  }

  // timeseries / heatmap / distribution / scatter
  const { keys, data } = buildSeriesData(w);
  const display = normalizeDisplay(w.display ?? undefined, w.type);
  const overlays = w.overlays ?? [];
  const anomalies = w.anomalies ?? [];

  if (w.type === "heatmap") {
    const buckets = buildHeatmapBuckets(w);
    return (
      <div className={`dw timeseries heatmap ${anomalies.length ? "has-anomaly" : ""}`} style={style}>
        <div className="dw-title">
          {w.title}
          <span>{w.series.length} series · heatmap</span>
        </div>
        <div className="dw-heatmap">
          {buckets.map((row) => (
            <div key={row.label} className="dw-heatmap-row">
              <span title={row.label}>{row.label}</span>
              <div className="dw-heatmap-cells">
                {row.cells.map((c, i) => (
                  <i
                    key={i}
                    title={`${row.label}: ${fmt(c, 2)}`}
                    style={{ background: heatColor(c, row.max) }}
                  />
                ))}
              </div>
            </div>
          ))}
          {!buckets.length && <div className="dd-empty">No heatmap data</div>}
        </div>
      </div>
    );
  }

  if (w.type === "scatter_plot") {
    const pts = buildScatterPoints(w);
    return (
      <div className={`dw timeseries scatter ${anomalies.length ? "has-anomaly" : ""}`} style={style}>
        <div className="dw-title">
          {w.title}
          <span>{pts.length} points</span>
        </div>
        <ChartShell>
          {pts.length ? (
            <ResponsiveContainer width="100%" height="100%">
              <ScatterChart margin={{ top: 10, right: 10, left: 0, bottom: 4 }}>
                <CartesianGrid stroke={chartGrid} strokeDasharray="3 6" />
                <XAxis type="number" dataKey="x" name="x" tick={tick} axisLine={false} tickLine={false} />
                <YAxis type="number" dataKey="y" name="y" tick={tick} width={44} axisLine={false} tickLine={false} />
                <Tooltip contentStyle={tip} cursor={{ strokeDasharray: "3 3" }} />
                <Scatter data={pts} fill="#4285F4">
                  {pts.map((_, i) => (
                    <Cell key={i} fill={SERIES_COLORS[i % SERIES_COLORS.length]} />
                  ))}
                </Scatter>
              </ScatterChart>
            </ResponsiveContainer>
          ) : (
            <div className="dd-empty">No scatter data</div>
          )}
        </ChartShell>
      </div>
    );
  }

  return (
    <div className={`dw timeseries ${w.type} ${anomalies.length ? "has-anomaly" : ""}`} style={style}>
      <div className="dw-title">
        {w.title}
        <span>
          {w.series.length} series{w.unit ? ` · ${w.unit}` : ""}
          {w.functions?.length ? ` · ${w.functions.join(",")}` : ""}
          {onCorrelate && (
            <button type="button" className="dw-corr" onClick={onCorrelate} title="Find correlated metrics">
              Correlations
            </button>
          )}
        </span>
      </div>
      <ChartShell>
        {anomalies.length > 0 && <div className="dw-anomaly-band" title={anomalies[0].summary} />}
        {!data.length ? (
          <div className="dd-empty">No data</div>
        ) : (
          <SeriesChart w={w} data={data} keys={keys} display={display} />
        )}
        {anomalies.length > 0 && (
          <div className="dw-anomaly-chip" title={anomalies[0].summary}>
            {anomalies[0].summary}
          </div>
        )}
        {overlays.length > 0 && (
          <div className="dw-overlays">
            {overlays.map((o) => (
              <span key={o.id} style={{ borderColor: o.color || "#FBBC04" }}>
                {o.label}
              </span>
            ))}
          </div>
        )}
      </ChartShell>
      {data.length > 0 && keys.length > 1 && (
        <div className="dw-legend">
          {keys.slice(0, 6).map((k, i) => (
            <span key={k}>
              <i style={{ background: SERIES_COLORS[i % SERIES_COLORS.length] }} />
              {k}
            </span>
          ))}
          {keys.length > 6 && <span className="muted">+{keys.length - 6}</span>}
        </div>
      )}
    </div>
  );
}

function inferAccent(title: string): string | null {
  const t = title.toLowerCase();
  if (/(revenue|uptime|ok|healthy|sites up)/.test(t)) return "green";
  if (/(alert|error|apache|request \/ sec)/.test(t)) return "red";
  if (/(latency|warn)/.test(t)) return "orange";
  return null;
}
