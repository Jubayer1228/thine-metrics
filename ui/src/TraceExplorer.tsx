import { useEffect, useMemo, useState } from "react";
import { api, type ApmTraceSummary, type ApmTraceTree, type ServiceMap } from "./api";
import { fmt } from "./widgets";

const COLORS = ["#5B91EB", "#2EC4B6", "#A371E3", "#F4A261", "#F25F5C", "#4CC9F0", "#E9C46A"];

export function TraceExplorer({
  statsHint,
  initialView = "all",
}: {
  statsHint?: string;
  initialView?: "all" | "map" | "traces";
}) {
  const [traces, setTraces] = useState<ApmTraceSummary[]>([]);
  const [selected, setSelected] = useState<string | null>(null);
  const [tree, setTree] = useState<ApmTraceTree | null>(null);
  const [map, setMap] = useState<ServiceMap | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [filter, setFilter] = useState("");
  const showMap = initialView === "all" || initialView === "map";
  const showTraces = initialView === "all" || initialView === "traces";

  useEffect(() => {
    Promise.all([api.apmTraces(50), api.serviceMap()])
      .then(([t, m]) => {
        setTraces(t);
        setMap(m);
        setSelected((prev) => prev ?? t[0]?.trace_id ?? null);
      })
      .catch((e) => setError(e instanceof Error ? e.message : "Trace load failed"));
  }, []);

  useEffect(() => {
    if (!selected) {
      setTree(null);
      return;
    }
    api
      .apmTrace(selected)
      .then(setTree)
      .catch(() => setTree(null));
  }, [selected]);

  const filtered = useMemo(() => {
    const q = filter.trim().toLowerCase();
    if (!q) return traces;
    return traces.filter(
      (t) =>
        t.service.toLowerCase().includes(q) ||
        t.name.toLowerCase().includes(q) ||
        t.trace_id.toLowerCase().includes(q) ||
        t.status.toLowerCase().includes(q),
    );
  }, [traces, filter]);

  const maxDur = Math.max(1, ...(tree?.spans.map((s) => s.duration_ms) ?? [1]));
  const t0 = Math.min(...(tree?.spans.map((s) => s.timestamp_ms) ?? [0]));

  return (
    <div className="trace-explorer">
      {error && <div className="dd-banner">{error}</div>}
      <div className="obs-kpi-row">
        <div className="obs-kpi">
          <span>Recent traces</span>
          <strong>{traces.length}</strong>
          <small>{statsHint}</small>
        </div>
        <div className="obs-kpi">
          <span>Service map nodes</span>
          <strong>{map?.nodes.length ?? 0}</strong>
          <small>{map?.detected_via?.join(" · ")}</small>
        </div>
        <div className="obs-kpi">
          <span>Edges</span>
          <strong>{map?.edges.length ?? 0}</strong>
        </div>
        <div className="obs-kpi">
          <span>Erroring services</span>
          <strong>{map?.nodes.filter((n) => n.error_rate > 0).length ?? 0}</strong>
        </div>
      </div>

      {showMap && map && map.nodes.length > 0 && (
        <div className="obs-card svc-map">
          <h3>Live service map</h3>
          <svg viewBox="0 0 640 220" className="svc-map-svg">
            {map.nodes.map((n, i) => {
              const angle = (i / map.nodes.length) * Math.PI * 2 - Math.PI / 2;
              const cx = 320 + Math.cos(angle) * 120;
              const cy = 110 + Math.sin(angle) * 70;
              return (
                <g key={n.id}>
                  <circle cx={cx} cy={cy} r={18 + Math.min(14, n.requests / 2)} fill={COLORS[i % COLORS.length]} opacity={0.85} />
                  <text x={cx} y={cy + 4} textAnchor="middle" fill="#fff" fontSize="10" fontWeight="700">
                    {n.id.slice(0, 8)}
                  </text>
                  <title>
                    {n.id} · p95 {fmt(n.p95_ms, 0)}ms · err {(n.error_rate * 100).toFixed(1)}%
                  </title>
                </g>
              );
            })}
            {map.edges.map((e, i) => {
              const from = map.nodes.findIndex((n) => n.id === e.from);
              const to = map.nodes.findIndex((n) => n.id === e.to);
              if (from < 0 || to < 0) return null;
              const a1 = (from / map.nodes.length) * Math.PI * 2 - Math.PI / 2;
              const a2 = (to / map.nodes.length) * Math.PI * 2 - Math.PI / 2;
              const x1 = 320 + Math.cos(a1) * 120;
              const y1 = 110 + Math.sin(a1) * 70;
              const x2 = 320 + Math.cos(a2) * 120;
              const y2 = 110 + Math.sin(a2) * 70;
              return (
                <line
                  key={i}
                  x1={x1}
                  y1={y1}
                  x2={x2}
                  y2={y2}
                  stroke="rgba(232,237,245,0.25)"
                  strokeWidth={1 + Math.min(4, e.bytes / 2_000_000)}
                />
              );
            })}
          </svg>
          <div className="svc-map-legend">
            {map.edges.slice(0, 6).map((e, i) => (
              <code key={i}>
                {e.from}→{e.to} ({e.protocol})
              </code>
            ))}
          </div>
        </div>
      )}

      {showTraces && (
      <div className="trace-split">
        <div className="obs-card">
          <div className="trace-list-head">
            <h3>Traces</h3>
            <input placeholder="Filter service / id · status:error" value={filter} onChange={(e) => setFilter(e.target.value)} />
          </div>
          <table className="dd-table compact">
            <thead>
              <tr>
                <th>Trace</th>
                <th>Service</th>
                <th>Op</th>
                <th>Dur</th>
                <th>Status</th>
              </tr>
            </thead>
            <tbody>
              {filtered.map((t) => (
                <tr
                  key={t.trace_id + t.span_id}
                  className={selected === t.trace_id ? "row-on" : ""}
                  onClick={() => setSelected(t.trace_id)}
                  style={{ cursor: "pointer" }}
                >
                  <td>
                    <code>{t.trace_id}</code>
                  </td>
                  <td>{t.service}</td>
                  <td>{t.name}</td>
                  <td>{fmt(t.duration_ms, 1)}ms</td>
                  <td>
                    <span className={`sev ${t.status === "error" || t.status === "erroring" ? "alert" : "ok"}`}>{t.status}</span>
                  </td>
                </tr>
              ))}
              {!filtered.length && (
                <tr>
                  <td colSpan={5} className="tags">
                    No traces
                  </td>
                </tr>
              )}
            </tbody>
          </table>
        </div>

        <div className="obs-card">
          <h3>Waterfall {tree ? `· ${tree.trace_id}` : ""}</h3>
          {tree ? (
            <>
              <p className="muted">
                {tree.spans.length} spans · {tree.service_count} services · total {fmt(tree.total_duration_ms, 1)}ms · root{" "}
                <code>{tree.root}</code>
              </p>
              <div className="waterfall">
                {tree.spans.map((s, i) => {
                  const left = ((s.timestamp_ms - t0) / Math.max(1, tree.total_duration_ms)) * 100;
                  const width = Math.max(4, (s.duration_ms / maxDur) * 100);
                  return (
                    <div key={s.span_id} className="waterfall-row">
                      <div className="waterfall-meta">
                        <code>{s.service}</code>
                        <span>{s.name}</span>
                        <em>{fmt(s.duration_ms, 1)}ms</em>
                      </div>
                      <div className="waterfall-track">
                        <div
                          className="waterfall-bar"
                          style={{
                            marginLeft: `${Math.min(90, Math.max(0, left))}%`,
                            width: `${Math.min(100, width)}%`,
                            background: COLORS[i % COLORS.length],
                          }}
                        />
                      </div>
                    </div>
                  );
                })}
              </div>
            </>
          ) : (
            <p className="muted">Select a trace to inspect spans</p>
          )}
        </div>
      </div>
      )}
    </div>
  );
}
