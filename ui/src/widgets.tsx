import type { CSSProperties } from "react";
import {
  Area,
  AreaChart,
  Bar,
  BarChart,
  CartesianGrid,
  Line,
  LineChart,
  ResponsiveContainer,
  Tooltip,
  XAxis,
  YAxis,
} from "recharts";
import type { RenderedWidget, Sample } from "./api";

const SERIES_COLORS = [
  "#5B91EB",
  "#A371E3",
  "#FF6B6B",
  "#F4A261",
  "#2EC4B6",
  "#E9C46A",
  "#F77FBE",
  "#4CC9F0",
];

const tip = {
  background: "#1C2333",
  border: "1px solid rgba(255,255,255,0.1)",
  borderRadius: 6,
  color: "#E8EDF5",
  fontSize: 12,
};

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

export function Sparkline({ points, color = "#5B91EB" }: { points: Sample[]; color?: string }) {
  if (!points.length) return <div className="spark empty" />;
  return (
    <div className="spark">
      <ResponsiveContainer width="100%" height="100%">
        <AreaChart data={sparkData(points)}>
          <Area
            type="monotone"
            dataKey="v"
            stroke={color}
            fill={color}
            fillOpacity={0.2}
            strokeWidth={1.5}
            dot={false}
            isAnimationActive={false}
          />
        </AreaChart>
      </ResponsiveContainer>
    </div>
  );
}

export function WidgetCard({ w }: { w: RenderedWidget }) {
  if (w.type === "group" || w.type === "note") {
    return (
      <div
        className={`dw group-banner section-${w.title.toLowerCase().replace(/\s+/g, "-")}`}
        style={gridStyle(w)}
      >
        <h2>{w.title}</h2>
        {w.text && w.text !== w.title && <p>{w.text}</p>}
      </div>
    );
  }

  if (w.type === "query_value") {
    const up = (w.change_pct ?? 0) > 0;
    const down = (w.change_pct ?? 0) < 0;
    const displayValue =
      w.unit === "%" && w.value != null ? fmt((w.value > 1 ? w.value : w.value * 100), 1) : fmt(w.value);
    return (
      <div className="dw query-value" style={gridStyle(w)}>
        <div className="dw-title">{w.title}</div>
        <div className="dw-value">
          {displayValue}
          {w.unit && w.unit !== "1" && w.unit !== "%" && <small>{w.unit}</small>}
          {w.unit === "%" && <small>%</small>}
        </div>
        <div className={`dw-change ${up ? "up" : down ? "down" : ""}`}>
          {w.change_pct == null ? "vs previous window" : `${up ? "↑" : down ? "↓" : "→"} ${fmt(Math.abs(w.change_pct), 2)}% vs previous`}
        </div>
        <Sparkline points={w.sparkline} color={up ? "#2EC4B6" : down ? "#FF6B6B" : "#5B91EB"} />
      </div>
    );
  }

  if (w.type === "toplist") {
    const max = Math.max(...w.toplist.map((i) => i.value), 1);
    return (
      <div className="dw toplist" style={gridStyle(w)}>
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
                <b style={{ width: `${(item.value / max) * 100}%`, background: SERIES_COLORS[i % SERIES_COLORS.length] }} />
              </div>
            </li>
          ))}
          {!w.toplist.length && <li className="muted">No series</li>}
        </ul>
      </div>
    );
  }

  // timeseries
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
  const display = w.display || "line";

  return (
    <div className="dw timeseries" style={gridStyle(w)}>
      <div className="dw-title">
        {w.title}
        <span>
          {w.series.length} series{w.unit ? ` · ${w.unit}` : ""}
        </span>
      </div>
      <div className="dw-chart">
        {!data.length ? (
          <div className="dd-empty">No data</div>
        ) : (
          <ResponsiveContainer width="100%" height="100%">
            {display === "bars" ? (
              <BarChart data={data} margin={{ top: 8, right: 8, left: 0, bottom: 0 }}>
                <CartesianGrid stroke="rgba(255,255,255,0.06)" vertical={false} />
                <XAxis dataKey="label" tick={{ fill: "#8B97A8", fontSize: 10 }} minTickGap={28} />
                <YAxis tick={{ fill: "#8B97A8", fontSize: 10 }} width={42} />
                <Tooltip contentStyle={tip} />
                {keys.map((k, i) => (
                  <Bar key={k} dataKey={k} fill={SERIES_COLORS[i % SERIES_COLORS.length]} />
                ))}
              </BarChart>
            ) : display === "area" ? (
              <AreaChart data={data} margin={{ top: 8, right: 8, left: 0, bottom: 0 }}>
                <CartesianGrid stroke="rgba(255,255,255,0.06)" vertical={false} />
                <XAxis dataKey="label" tick={{ fill: "#8B97A8", fontSize: 10 }} minTickGap={28} />
                <YAxis tick={{ fill: "#8B97A8", fontSize: 10 }} width={42} />
                <Tooltip contentStyle={tip} />
                {keys.map((k, i) => (
                  <Area
                    key={k}
                    type="monotone"
                    dataKey={k}
                    stroke={SERIES_COLORS[i % SERIES_COLORS.length]}
                    fill={SERIES_COLORS[i % SERIES_COLORS.length]}
                    fillOpacity={0.15}
                    strokeWidth={2}
                    dot={false}
                    isAnimationActive={false}
                  />
                ))}
              </AreaChart>
            ) : (
              <LineChart data={data} margin={{ top: 8, right: 8, left: 0, bottom: 0 }}>
                <CartesianGrid stroke="rgba(255,255,255,0.06)" vertical={false} />
                <XAxis dataKey="label" tick={{ fill: "#8B97A8", fontSize: 10 }} minTickGap={28} />
                <YAxis tick={{ fill: "#8B97A8", fontSize: 10 }} width={42} />
                <Tooltip contentStyle={tip} />
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
    </div>
  );
}

function gridStyle(w: RenderedWidget): CSSProperties {
  const { x, y, w: width, h } = w.layout;
  return {
    gridColumn: `${x + 1} / span ${Math.max(1, width)}`,
    gridRow: `${y + 1} / span ${Math.max(1, h)}`,
  };
}
