import { useCallback, useEffect, useMemo, useState } from "react";
import {
  Area,
  AreaChart,
  CartesianGrid,
  ResponsiveContainer,
  Tooltip,
  XAxis,
  YAxis,
} from "recharts";
import { api, type LogEvent, type Notebook, type NotebookCell, type QueryResult } from "./api";
import { chartTip } from "./theme";
import { fmt } from "./widgets";

const tip = chartTip;

function parseMetricQuery(content: string): { metric: string; tags?: string; agg: string; groupBy?: string } {
  // avg:http.server.duration{env:prod} by {service}
  const m = content.trim().match(/^(avg|sum|min|max|p95|p99)?:?([a-zA-Z0-9._-]+)(?:\{([^}]*)\})?(?:\s*by\s*\{([^}]*)\})?/i);
  if (!m) return { metric: content.trim() || "http.server.duration", agg: "avg" };
  return {
    agg: (m[1] || "avg").toLowerCase(),
    metric: m[2],
    tags: m[3]?.trim() || undefined,
    groupBy: m[4]?.trim() || undefined,
  };
}

function CellChart({ series }: { series: QueryResult[] }) {
  const data = useMemo(() => {
    const map = new Map<number, Record<string, number | string>>();
    series.forEach((s, i) => {
      const key = Object.entries(s.tags).map(([k, v]) => `${k}:${v}`).join(",") || `s${i}`;
      s.points.forEach((p) => {
        const row = map.get(p.timestamp_ms) ?? {
          t: p.timestamp_ms,
          label: new Date(p.timestamp_ms).toLocaleTimeString([], { hour: "2-digit", minute: "2-digit" }),
        };
        row[key] = p.value;
        map.set(p.timestamp_ms, row);
      });
    });
    return Array.from(map.values()).sort((a, b) => Number(a.t) - Number(b.t));
  }, [series]);
  const keys = series.map((s, i) => Object.entries(s.tags).map(([k, v]) => `${k}:${v}`).join(",") || `s${i}`);
  return (
    <div className="nb-chart">
      <ResponsiveContainer width="100%" height="100%">
        <AreaChart data={data} margin={{ top: 4, right: 8, left: 0, bottom: 0 }}>
          <CartesianGrid stroke="rgba(255,255,255,0.06)" vertical={false} />
          <XAxis dataKey="label" tick={{ fill: "#8B97A8", fontSize: 10 }} minTickGap={24} />
          <YAxis tick={{ fill: "#8B97A8", fontSize: 10 }} width={40} />
          <Tooltip contentStyle={tip} />
          {keys.map((k, i) => (
            <Area
              key={k}
              type="monotone"
              dataKey={k}
              stroke={["#5B91EB", "#2EC4B6", "#A371E3", "#F4A261"][i % 4]}
              fill={["#5B91EB33", "#2EC4B633", "#A371E333", "#F4A26133"][i % 4]}
              strokeWidth={2}
              isAnimationActive={false}
            />
          ))}
        </AreaChart>
      </ResponsiveContainer>
    </div>
  );
}

export function NotebooksPage({ initialNotebookId }: { initialNotebookId?: string | null } = {}) {
  const [notebooks, setNotebooks] = useState<Notebook[]>([]);
  const [activeId, setActiveId] = useState<string | null>(initialNotebookId ?? null);
  const [error, setError] = useState<string | null>(null);
  const [creating, setCreating] = useState(false);
  const [saving, setSaving] = useState(false);
  const [editingTitle, setEditingTitle] = useState(false);
  const [titleDraft, setTitleDraft] = useState("");
  const [cellResults, setCellResults] = useState<Record<string, QueryResult[]>>({});
  const [logResults, setLogResults] = useState<Record<string, LogEvent[]>>({});
  const [traceResults, setTraceResults] = useState<Record<string, { trace_id: string; service: string; name: string; duration_ms: number }[]>>({});
  const [shareUrl, setShareUrl] = useState<string | null>(null);
  const [running, setRunning] = useState<string | null>(null);
  const [editCellIdx, setEditCellIdx] = useState<number | null>(null);
  const [cellDraft, setCellDraft] = useState("");

  const refresh = useCallback(async () => {
    try {
      const list = await api.notebooks();
      setNotebooks(list);
      setActiveId((prev) => prev ?? initialNotebookId ?? list[0]?.id ?? null);
      setError(null);
    } catch (e) {
      setError(e instanceof Error ? e.message : "Failed to load notebooks");
    }
  }, [initialNotebookId]);

  useEffect(() => {
    void refresh();
  }, [refresh]);

  useEffect(() => {
    if (initialNotebookId) setActiveId(initialNotebookId);
  }, [initialNotebookId]);

  const active = notebooks.find((n) => n.id === activeId) ?? null;

  async function runCell(nbId: string, idx: number, cell: NotebookCell) {
    if (cell.kind === "markdown") return;
    const key = `${nbId}:${idx}`;
    setRunning(key);
    try {
      if (cell.kind === "logs") {
        const res = await api.logsSearch({ limit: 40, q: cell.content || undefined });
        setLogResults((prev) => ({ ...prev, [key]: res.logs }));
      } else if (cell.kind === "traces") {
        const traces = await api.apmTraces(30);
        const svc = cell.content.trim();
        setTraceResults((prev) => ({
          ...prev,
          [key]: svc ? traces.filter((t) => t.service === svc || t.name.includes(svc)) : traces,
        }));
      } else {
        const q = parseMetricQuery(cell.content);
        const end = Date.now();
        const series = await api.query({
          metric: q.metric,
          tags: q.tags,
          aggregation: q.agg,
          start_ms: end - 3_600_000,
          end_ms: end,
          step_ms: 30_000,
          group_by: q.groupBy,
        });
        setCellResults((prev) => ({ ...prev, [key]: series.filter((s) => s.points.length) }));
      }
    } catch (e) {
      setError(e instanceof Error ? e.message : "Cell query failed");
    } finally {
      setRunning(null);
    }
  }

  async function runAll() {
    if (!active) return;
    for (let i = 0; i < active.cells.length; i++) {
      await runCell(active.id, i, active.cells[i]);
    }
  }

  async function createInvestigation() {
    setCreating(true);
    try {
      const nb = await api.createNotebook({
        title: `RCA ${new Date().toLocaleString()}`,
        cells: [
          {
            kind: "markdown",
            content:
              "## Unified RCA notebook\nThine combines **metrics + APM + AI evals** in one cell stream — beyond Datadog notebooks (no LLM evals) and LangSmith (no infra metrics).",
          },
          { kind: "metric", content: "avg:http.server.duration{env:prod} by {service}" },
          { kind: "metric", content: "avg:process.runtime.cpu.utilization{env:prod} by {service}" },
          {
            kind: "markdown",
            content: "### Next\nCorrelate with APM traces and AI pass-rate in Observability → AI / Agents.",
          },
        ],
      });
      await refresh();
      setActiveId(nb.id);
    } catch (e) {
      setError(e instanceof Error ? e.message : "Create failed");
    } finally {
      setCreating(false);
    }
  }

  async function saveTitle() {
    if (!active || !titleDraft.trim()) return;
    setSaving(true);
    try {
      const nb = await api.updateNotebook(active.id, { title: titleDraft.trim() });
      setNotebooks((list) => list.map((n) => (n.id === nb.id ? nb : n)));
      setEditingTitle(false);
    } catch (e) {
      setError(e instanceof Error ? e.message : "Rename failed");
    } finally {
      setSaving(false);
    }
  }

  async function saveCell(idx: number) {
    if (!active) return;
    setSaving(true);
    try {
      const cells = active.cells.map((c, i) => (i === idx ? { ...c, content: cellDraft } : c));
      const nb = await api.updateNotebook(active.id, { cells });
      setNotebooks((list) => list.map((n) => (n.id === nb.id ? nb : n)));
      setEditCellIdx(null);
    } catch (e) {
      setError(e instanceof Error ? e.message : "Save cell failed");
    } finally {
      setSaving(false);
    }
  }

  async function addCell(kind: "markdown" | "metric" | "logs" | "traces") {
    if (!active) return;
    setSaving(true);
    try {
      const content =
        kind === "markdown"
          ? "## Notes"
          : kind === "logs"
            ? "service:api status:error"
            : kind === "traces"
              ? "api"
              : "avg:http.server.duration{*} by {service}";
      const cells = [
        ...active.cells,
        {
          kind,
          content,
        },
      ];
      const nb = await api.updateNotebook(active.id, { cells });
      setNotebooks((list) => list.map((n) => (n.id === nb.id ? nb : n)));
    } catch (e) {
      setError(e instanceof Error ? e.message : "Add cell failed");
    } finally {
      setSaving(false);
    }
  }

  async function removeCell(idx: number) {
    if (!active) return;
    setSaving(true);
    try {
      const cells = active.cells.filter((_, i) => i !== idx);
      const nb = await api.updateNotebook(active.id, { cells });
      setNotebooks((list) => list.map((n) => (n.id === nb.id ? nb : n)));
    } catch (e) {
      setError(e instanceof Error ? e.message : "Remove cell failed");
    } finally {
      setSaving(false);
    }
  }

  async function deleteNotebook() {
    if (!active) return;
    if (!window.confirm(`Delete notebook “${active.title}”?`)) return;
    try {
      await api.deleteNotebook(active.id);
      setActiveId(null);
      await refresh();
    } catch (e) {
      setError(e instanceof Error ? e.message : "Delete failed");
    }
  }

  return (
    <section className="dd-panel nb-page">
      {error && <div className="dd-banner">{error}</div>}
      <div className="nb-layout">
        <aside className="nb-list">
          <div className="nb-list-head">
            <h3>Notebooks</h3>
            <button type="button" onClick={() => void createInvestigation()} disabled={creating}>
              {creating ? "…" : "New +"}
            </button>
          </div>
          {notebooks.map((n) => (
            <button
              key={n.id}
              type="button"
              className={`nb-item${n.id === activeId ? " on" : ""}`}
              onClick={() => setActiveId(n.id)}
            >
              <strong>{n.title}</strong>
              <small>
                {n.cells.length} cells · {new Date(n.updated_at_ms).toLocaleString()}
              </small>
            </button>
          ))}
          {!notebooks.length && <p className="muted">No notebooks yet</p>}
        </aside>
        <div className="nb-body">
          {active ? (
            <>
              <header className="nb-head">
                <div>
                  {editingTitle ? (
                    <div className="bits-input">
                      <input value={titleDraft} onChange={(e) => setTitleDraft(e.target.value)} />
                      <button type="button" disabled={saving} onClick={() => void saveTitle()}>
                        Save
                      </button>
                      <button type="button" className="ghost" onClick={() => setEditingTitle(false)}>
                        Cancel
                      </button>
                    </div>
                  ) : (
                    <h2
                      style={{ cursor: "pointer" }}
                      title="Click to rename"
                      onClick={() => {
                        setTitleDraft(active.title);
                        setEditingTitle(true);
                      }}
                    >
                      {active.title}
                    </h2>
                  )}
                  <p className="muted">Executable investigation — edit cells, run queries, delete when done</p>
                </div>
                <div style={{ display: "flex", gap: "0.5rem", flexWrap: "wrap" }}>
                  <button type="button" className="ghost" onClick={() => void addCell("metric")} disabled={saving}>
                    + Metric
                  </button>
                  <button type="button" className="ghost" onClick={() => void addCell("logs")} disabled={saving}>
                    + Logs
                  </button>
                  <button type="button" className="ghost" onClick={() => void addCell("traces")} disabled={saving}>
                    + Traces
                  </button>
                  <button type="button" className="ghost" onClick={() => void addCell("markdown")} disabled={saving}>
                    + Markdown
                  </button>
                  <button type="button" className="ghost" onClick={() => void runAll()}>
                    Run all
                  </button>
                  <button
                    type="button"
                    className="ghost"
                    onClick={() => {
                      void api
                        .shareNotebook(active.id)
                        .then((nb) => {
                          const url = `${window.location.origin}/api/v1/shared/notebooks/${nb.share_token}`;
                          setShareUrl(url);
                          void navigator.clipboard?.writeText(url);
                        })
                        .catch((e) => setError(e instanceof Error ? e.message : "Share failed"));
                    }}
                  >
                    Share
                  </button>
                  <button type="button" className="ghost" onClick={() => void deleteNotebook()}>
                    Delete
                  </button>
                </div>
                {shareUrl && (
                  <p className="muted" style={{ width: "100%" }}>
                    Share link: <code>{shareUrl}</code>
                  </p>
                )}
              </header>
              <div className="nb-cells">
                {active.cells.map((cell, i) => {
                  const key = `${active.id}:${i}`;
                  const series = cellResults[key];
                  const logs = logResults[key];
                  const traces = traceResults[key];
                  const editing = editCellIdx === i;
                  return (
                    <article key={key} className={`nb-cell kind-${cell.kind}`}>
                      <div className="nb-cell-meta">
                        <span>{cell.kind}</span>
                        <div style={{ display: "flex", gap: "0.35rem" }}>
                          {editing ? (
                            <>
                              <button type="button" disabled={saving} onClick={() => void saveCell(i)}>
                                Save
                              </button>
                              <button type="button" className="ghost" onClick={() => setEditCellIdx(null)}>
                                Cancel
                              </button>
                            </>
                          ) : (
                            <>
                              <button
                                type="button"
                                className="ghost"
                                onClick={() => {
                                  setEditCellIdx(i);
                                  setCellDraft(cell.content);
                                }}
                              >
                                Edit
                              </button>
                              <button type="button" className="ghost" onClick={() => void removeCell(i)}>
                                Remove
                              </button>
                              {cell.kind !== "markdown" && (
                                <button
                                  type="button"
                                  className="ghost"
                                  disabled={running === key}
                                  onClick={() => void runCell(active.id, i, cell)}
                                >
                                  {running === key ? "Running…" : "Run"}
                                </button>
                              )}
                            </>
                          )}
                        </div>
                      </div>
                      {editing ? (
                        <textarea
                          value={cellDraft}
                          onChange={(e) => setCellDraft(e.target.value)}
                          rows={cell.kind === "markdown" ? 6 : 3}
                          style={{ width: "100%", fontFamily: "inherit" }}
                        />
                      ) : cell.kind === "markdown" ? (
                        <div className="nb-md">
                          {cell.content.split("\n").map((line, li) => (
                            <p key={li}>{line.replace(/^#+\s*/, "")}</p>
                          ))}
                        </div>
                      ) : cell.kind === "logs" ? (
                        <>
                          <code className="nb-query">{cell.content}</code>
                          {logs ? (
                            <div className="log-stream">
                              {logs.slice(0, 20).map((l, li) => (
                                <div key={li} className={`log-line ${l.level}`}>
                                  <b>{l.level}</b> <code>{l.service}</code> <em>{l.message}</em>
                                </div>
                              ))}
                              {!logs.length && <p className="muted">No matching logs</p>}
                            </div>
                          ) : (
                            <p className="muted">Run cell to fetch logs</p>
                          )}
                        </>
                      ) : cell.kind === "traces" ? (
                        <>
                          <code className="nb-query">{cell.content || "(all services)"}</code>
                          {traces ? (
                            <table className="dd-table compact">
                              <thead>
                                <tr>
                                  <th>Trace</th>
                                  <th>Service</th>
                                  <th>Name</th>
                                  <th>Duration</th>
                                </tr>
                              </thead>
                              <tbody>
                                {traces.slice(0, 12).map((t) => (
                                  <tr key={t.trace_id}>
                                    <td>
                                      <code>{t.trace_id.slice(0, 8)}</code>
                                    </td>
                                    <td>{t.service}</td>
                                    <td>{t.name}</td>
                                    <td>{fmt(t.duration_ms, 1)}ms</td>
                                  </tr>
                                ))}
                              </tbody>
                            </table>
                          ) : (
                            <p className="muted">Run cell to fetch traces</p>
                          )}
                        </>
                      ) : (
                        <>
                          <code className="nb-query">{cell.content}</code>
                          {series ? (
                            series.length ? (
                              <>
                                <CellChart series={series} />
                                <small className="muted">
                                  {series.length} series · last {fmt(series[0].points.at(-1)?.value ?? 0, 2)}
                                </small>
                              </>
                            ) : (
                              <p className="muted">No points for this query</p>
                            )
                          ) : (
                            <p className="muted">Run cell to graph live metrics</p>
                          )}
                        </>
                      )}
                    </article>
                  );
                })}
              </div>
            </>
          ) : (
            <p className="muted">Select or create a notebook</p>
          )}
        </div>
      </div>
    </section>
  );
}
