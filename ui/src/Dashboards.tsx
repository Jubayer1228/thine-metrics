import { useCallback, useEffect, useMemo, useState } from "react";
import {
  api,
  type BoardMeta,
  type DashboardAnomalyIssue,
  type DashboardList,
  type DashboardsGuide,
  type RenderedBoard,
  type TemplateVariable,
} from "./api";
import { EditGraphModal, type GraphDraft } from "./EditGraph";
import { GraphInsightsPanel } from "./GraphInsights";
import { WidgetCard, gridStyle } from "./widgets";

const ESSENTIAL_WIDGETS = [
  ["timeseries", "Timeseries", { metric: "http.server.duration", aggregation: "avg", group_by: "service" }],
  ["query_value", "Query Value", { metric: "http.server.request.count", aggregation: "sum", accent: "purple", unit: "1" }],
  ["toplist", "Top List", { metric: "http.server.duration", aggregation: "avg", group_by: "service" }],
  ["heatmap", "Heatmap", { metric: "http.server.duration", aggregation: "avg", group_by: "service" }],
  ["table", "Table", { metric: "http.server.duration", aggregation: "avg", group_by: "service" }],
  ["hostmap", "Host Map", { metric: "process.runtime.cpu.utilization", aggregation: "avg", group_by: "host" }],
  ["slo", "SLO", { metric: "http.server.duration", aggregation: "avg" }],
  ["event_stream", "Event Stream", { metric: "http.server.duration", aggregation: "sum" }],
] as const;

const GOLDEN_SIGNALS = [
  { type: "query_value", title: "Traffic", metric: "http.server.request.count", aggregation: "sum", unit: "1", accent: "purple" },
  { type: "query_value", title: "Errors", metric: "http.server.duration", aggregation: "avg", unit: "%", accent: "green" },
  { type: "query_value", title: "Latency", metric: "http.server.duration", aggregation: "avg", unit: "ms", accent: "orange" },
  { type: "query_value", title: "Saturation", metric: "process.runtime.cpu.utilization", aggregation: "avg", unit: "%", accent: "yellow" },
] as const;

export type DashNavAction =
  | { type: "open-board" }
  | { type: "open-list"; listId?: string | null; preset?: "all" | "deleted" }
  | { type: "open-guide" }
  | { type: "new-dashboard" };

type Props = {
  boards: BoardMeta[];
  activeBoardId: string | null;
  onSelectBoard: (id: string) => void;
  onBoardsChanged: () => void;
  rangeMs: number;
  paused: boolean;
  navAction?: DashNavAction | null;
  navActionSeq?: number;
  onActiveListChange?: (listId: string | null) => void;
};

export function DashboardsPage({
  boards,
  activeBoardId,
  onSelectBoard,
  onBoardsChanged,
  rangeMs,
  paused,
  navAction,
  navActionSeq = 0,
  onActiveListChange,
}: Props) {
  const [rendered, setRendered] = useState<RenderedBoard | null>(null);
  const [lists, setLists] = useState<DashboardList[]>([]);
  const [guide, setGuide] = useState<DashboardsGuide | null>(null);
  const [view, setView] = useState<"board" | "list" | "guide">("board");
  const [listPreset, setListPreset] = useState<"all" | "deleted">("all");
  const [deleted, setDeleted] = useState<BoardMeta[]>([]);
  const [selectedDeleted, setSelectedDeleted] = useState<Set<string>>(new Set());
  const [vars, setVars] = useState<Record<string, string>>({});
  const [error, setError] = useState<string | null>(null);
  const [creating, setCreating] = useState(false);
  const [showCreateModal, setShowCreateModal] = useState(false);
  const [newName, setNewName] = useState("");
  const [newLayout, setNewLayout] = useState<"timeboard" | "screenboard" | "dashboard">("dashboard");
  const [listFilter, setListFilter] = useState("");
  const [activeListId, setActiveListId] = useState<string | null>(null);
  const [autoDetect, setAutoDetect] = useState(true);
  const [issues, setIssues] = useState<DashboardAnomalyIssue[]>([]);
  const [showInsights, setShowInsights] = useState(false);
  const [focusWidgetId, setFocusWidgetId] = useState<string | null>(null);
  const [corrMetric, setCorrMetric] = useState<string | null>(null);
  const [showAddVar, setShowAddVar] = useState(false);
  const [varType, setVarType] = useState<"filter" | "groupby">("filter");
  const [varTag, setVarTag] = useState("env");
  const [varName, setVarName] = useState("env");
  const [varValues, setVarValues] = useState("prod,staging,dev,*");
  const [showEditGraph, setShowEditGraph] = useState(false);
  const [metricNames, setMetricNames] = useState<string[]>([]);
  const [showPalette, setShowPalette] = useState(true);

  const varsQs = useMemo(() => {
    return Object.entries(vars)
      .filter(([, v]) => v && v !== "*")
      .map(([k, v]) => `${k}:${v}`)
      .join(",");
  }, [vars]);

  const refreshDeleted = useCallback(async () => {
    try {
      setDeleted(await api.deletedBoards());
    } catch {
      setDeleted([]);
    }
  }, []);

  const refresh = useCallback(async () => {
    if (!activeBoardId) return;
    try {
      const board = await api.renderBoard(activeBoardId, rangeMs, {
        vars: varsQs || undefined,
      });
      setRendered(board);
      setVars((prev) => {
        const next = { ...prev };
        for (const tv of board.template_variables) {
          if (next[tv.name] == null) next[tv.name] = board.template_selections[tv.name] ?? tv.default;
        }
        return next;
      });
      setError(null);
    } catch (e) {
      setError(e instanceof Error ? e.message : "Board render failed");
    }
  }, [activeBoardId, rangeMs, varsQs]);

  const refreshAnomalies = useCallback(async () => {
    if (!activeBoardId || !autoDetect) {
      setIssues([]);
      return;
    }
    try {
      const r = await api.boardAnomalies(activeBoardId, rangeMs, true);
      setIssues(r.issues);
    } catch {
      setIssues([]);
    }
  }, [activeBoardId, rangeMs, autoDetect]);

  useEffect(() => {
    api.boardLists().then(setLists).catch(() => setLists([]));
    api.dashboardsGuide().then(setGuide).catch(() => setGuide(null));
    api
      .metrics()
      .then((m) => setMetricNames([...new Set(m.map((x) => x.name))].sort()))
      .catch(() => setMetricNames([]));
    void refreshDeleted();
  }, [refreshDeleted]);

  useEffect(() => {
    if (view !== "board") return;
    refresh();
    refreshAnomalies();
    if (paused) return;
    const ms =
      rangeMs <= 10 * 60_000
        ? 10_000
        : rangeMs <= 60 * 60_000
          ? 20_000
          : rangeMs <= 4 * 60 * 60_000
            ? 60_000
            : 180_000;
    const id = setInterval(() => {
      refresh();
      refreshAnomalies();
    }, ms);
    return () => clearInterval(id);
  }, [refresh, refreshAnomalies, paused, view, rangeMs]);

  useEffect(() => {
    if (view === "list" && listPreset === "deleted") void refreshDeleted();
  }, [view, listPreset, refreshDeleted]);

  const filteredBoards = useMemo(() => {
    let list = boards;
    if (activeListId) {
      const bl = lists.find((l) => l.id === activeListId);
      if (bl) list = boards.filter((b) => bl.board_ids.includes(b.id));
    }
    const q = listFilter.trim().toLowerCase();
    if (!q) return list;
    return list.filter(
      (b) =>
        b.name.toLowerCase().includes(q) ||
        (b.description || "").toLowerCase().includes(q) ||
        (b.tags || []).some((t) => t.toLowerCase().includes(q)),
    );
  }, [boards, lists, activeListId, listFilter]);

  const widgetsWithAnomalies = useMemo(() => {
    if (!rendered) return [];
    const byWidget = new Map<string, DashboardAnomalyIssue[]>();
    for (const issue of issues) {
      for (const wid of issue.widget_ids) {
        const arr = byWidget.get(wid) ?? [];
        arr.push(issue);
        byWidget.set(wid, arr);
      }
    }
    return rendered.widgets.map((w) => {
      const related = byWidget.get(w.id) ?? [];
      if (!related.length) return w;
      return {
        ...w,
        anomalies: related.map((i) => i.anomaly),
      };
    });
  }, [rendered, issues]);

  function openCreateModal() {
    setNewName("Platform-");
    setNewLayout("dashboard");
    setShowCreateModal(true);
  }

  useEffect(() => {
    onActiveListChange?.(activeListId);
  }, [activeListId, onActiveListChange]);

  useEffect(() => {
    if (!navAction || navActionSeq <= 0) return;
    if (navAction.type === "open-board") {
      setView("board");
      return;
    }
    if (navAction.type === "open-guide") {
      setView("guide");
      return;
    }
    if (navAction.type === "new-dashboard") {
      openCreateModal();
      return;
    }
    if (navAction.type === "open-list") {
      setView("list");
      setListPreset(navAction.preset ?? "all");
      setActiveListId(navAction.listId ?? null);
    }
  }, [navAction, navActionSeq]);

  async function createBoard(layout?: "timeboard" | "screenboard" | "dashboard") {
    const kind = layout ?? newLayout;
    if (!newName.trim()) return;
    setCreating(true);
    try {
      const isScreen = kind === "screenboard";
      const isTime = kind === "timeboard";
      const b = await api.createBoard({
        name: newName.trim(),
        description: isScreen
          ? "For status boards and sharing data — mix widgets and timeframes, custom drag-and-drop layout"
          : isTime
            ? "For troubleshooting and correlation — time-synchronized metrics and event graphs, automatic layout"
            : "Snap widgets into place on a grid — recommended starter dashboard",
        layout_type: kind,
        template_variables: [
          {
            name: "env",
            tag: "env",
            default: "prod",
            available_values: ["prod", "staging", "dev", "*"],
            prefix: "filter",
          },
          {
            name: "service",
            tag: "service",
            default: "*",
            available_values: ["*", "api", "web", "worker"],
            prefix: "filter",
          },
          ...(isScreen
            ? [
                {
                  name: "customer-id",
                  tag: "customer",
                  default: "*",
                  available_values: ["*", "acme", "globex"],
                  prefix: "filter",
                },
              ]
            : []),
        ],
        widgets: isScreen
          ? [
              {
                id: "sec-frontend",
                type: "group",
                title: "Frontend",
                metric: "",
                tags: {},
                aggregation: "avg",
                layout: { x: 0, y: 0, w: 12, h: 1 },
                text: "Frontend",
              },
              {
                id: "qv-starter",
                type: "query_value",
                title: "Requests",
                metric: "http.server.request.count",
                tags: {},
                aggregation: "avg",
                layout: { x: 0, y: 1, w: 3, h: 2 },
                unit: "1",
                accent: "green",
              },
            ]
          : [
              {
                id: "ts-starter",
                type: "timeseries",
                title: "Latency",
                metric: "http.server.duration",
                tags: {},
                aggregation: "avg",
                layout: { x: 0, y: 0, w: 12, h: 4 },
                unit: "ms",
                display: "line",
              },
            ],
      });
      setShowCreateModal(false);
      setNewName("");
      onBoardsChanged();
      onSelectBoard(b.id);
      setView("board");
      setListPreset("all");
    } catch (e) {
      setError(e instanceof Error ? e.message : "Create failed");
    } finally {
      setCreating(false);
    }
  }

  async function toggleShare() {
    if (!activeBoardId || !rendered) return;
    const next = !(rendered.share?.public ?? false);
    try {
      await api.shareBoard(activeBoardId, next);
      await refresh();
    } catch (e) {
      setError(e instanceof Error ? e.message : "Share failed");
    }
  }

  async function copyWidgets() {
    if (!activeBoardId) return;
    try {
      const res = await fetch(`/api/v1/boards/${activeBoardId}`);
      const full = await res.json();
      const r = await api.clipboardSet(full.widgets || []);
      setError(null);
      alert(`Copied ${r.count} widgets to clipboard`);
    } catch (e) {
      setError(e instanceof Error ? e.message : "Clipboard failed");
    }
  }

  async function addPaletteWidget(
    type: string,
    label: string,
    defaults: {
      metric?: string;
      aggregation?: string;
      group_by?: string;
      text?: string;
      accent?: string;
      unit?: string;
      display?: string;
    },
  ) {
    if (!activeBoardId) return;
    try {
      const full = (await api.getBoard(activeBoardId)) as {
        widgets?: Array<Record<string, unknown> & { layout?: { x: number; y: number; w: number; h: number } }>;
      };
      const existing = full.widgets ?? [];
      const maxY = existing.reduce((m, w) => Math.max(m, (w.layout?.y ?? 0) + (w.layout?.h ?? 2)), 0);
      const widget = {
        id: `w-${crypto.randomUUID().slice(0, 8)}`,
        type,
        title: label,
        metric: defaults.metric ?? "http.server.duration",
        aggregation: defaults.aggregation ?? "avg",
        group_by: defaults.group_by,
        text: defaults.text,
        accent: defaults.accent,
        unit: defaults.unit,
        display: defaults.display,
        layout: {
          x: 0,
          y: maxY,
          w: type === "note" || type === "group" ? 12 : type === "query_value" ? 3 : 4,
          h: type === "timeseries" || type === "heatmap" ? 3 : 2,
        },
      };
      await api.updateBoard(activeBoardId, { widgets: [...existing, widget] as never });
      await refresh();
      onBoardsChanged();
    } catch (e) {
      setError(e instanceof Error ? e.message : "Add widget failed");
    }
  }

  async function saveTemplateVariable() {
    if (!activeBoardId || !varName.trim() || !varTag.trim()) return;
    try {
      const full = await api.getBoard(activeBoardId);
      const existing = (full.template_variables ?? []) as TemplateVariable[];
      const values = varValues
        .split(",")
        .map((v) => v.trim())
        .filter(Boolean);
      const next: TemplateVariable = {
        name: varName.trim().replace(/^\$/, ""),
        tag: varTag.trim(),
        default: values[0] || "*",
        available_values: values.length ? values : ["*"],
        prefix: varType === "groupby" ? "groupby" : "filter",
      };
      const merged = [...existing.filter((t) => t.name !== next.name), next];
      await api.updateBoard(activeBoardId, { template_variables: merged });
      setShowAddVar(false);
      setVars((v) => ({ ...v, [next.name]: next.default }));
      await refresh();
      onBoardsChanged();
    } catch (e) {
      setError(e instanceof Error ? e.message : "Add variable failed");
    }
  }

  async function saveEditedGraph(draft: GraphDraft) {
    if (!activeBoardId) return;
    try {
      const full = (await api.getBoard(activeBoardId)) as {
        widgets?: Array<Record<string, unknown> & { layout?: { x: number; y: number; w: number; h: number } }>;
      };
      const existing = full.widgets ?? [];
      if (draft.id) {
        const widgets = existing.map((w) =>
          w.id === draft.id
            ? {
                ...w,
                type: draft.type,
                title: draft.title,
                metric: draft.metric,
                aggregation: draft.aggregation,
                group_by: draft.group_by,
                display: draft.display,
              }
            : w,
        );
        await api.updateBoard(activeBoardId, { widgets: widgets as never });
      } else {
        await addPaletteWidget(draft.type, draft.title, {
          metric: draft.metric,
          aggregation: draft.aggregation,
          group_by: draft.group_by,
          display: draft.display,
          unit: draft.unit,
          accent: draft.accent,
        });
        return;
      }
      await refresh();
      onBoardsChanged();
    } catch (e) {
      setError(e instanceof Error ? e.message : "Save graph failed");
    }
  }

  async function addGoldenSignals() {
    if (!activeBoardId) return;
    for (const g of GOLDEN_SIGNALS) {
      await addPaletteWidget(g.type, g.title, {
        metric: g.metric,
        aggregation: g.aggregation,
        unit: g.unit,
        accent: g.accent,
      });
    }
  }

  async function softDeleteActive() {
    if (!activeBoardId) return;
    if (!confirm("Move this dashboard to Recently Deleted (recoverable 30 days)?")) return;
    try {
      await api.deleteBoard(activeBoardId);
      onBoardsChanged();
      await refreshDeleted();
      setView("list");
      setListPreset("deleted");
    } catch (e) {
      setError(e instanceof Error ? e.message : "Delete failed");
    }
  }

  async function restoreSelected(listId?: string) {
    const ids = [...selectedDeleted];
    if (!ids.length) return;
    try {
      for (const id of ids) {
        await api.restoreBoard(id, listId);
      }
      setSelectedDeleted(new Set());
      onBoardsChanged();
      await refreshDeleted();
      setListPreset("all");
    } catch (e) {
      setError(e instanceof Error ? e.message : "Restore failed");
    }
  }

  function applySavedView(id: string) {
    const view = rendered?.saved_views.find((v) => v.id === id);
    if (!view) return;
    setVars((prev) => ({ ...prev, ...view.selections }));
  }

  function toggleDeletedSelect(id: string) {
    setSelectedDeleted((prev) => {
      const next = new Set(prev);
      if (next.has(id)) next.delete(id);
      else next.add(id);
      return next;
    });
  }

  return (
    <section className="dd-panel dash-panel">
      <div className="dash-tabs">
        <button className={view === "board" ? "on" : ""} onClick={() => setView("board")}>
          Board
        </button>
        <button className={view === "list" ? "on" : ""} onClick={() => setView("list")}>
          Dashboard List
        </button>
        <button className={view === "guide" ? "on" : ""} onClick={() => setView("guide")}>
          Configure / Guide
        </button>
      </div>

      {error && (
        <div className="dd-banner compact">
          <span>{error}</span>
        </div>
      )}

      {view === "list" && (
        <div className="dash-list-view">
          <div className="dash-list-layout">
            <aside className="dash-list-side">
              <div className="side-label">Preset lists</div>
              <button
                className={listPreset === "all" && !activeListId ? "on" : ""}
                onClick={() => {
                  setListPreset("all");
                  setActiveListId(null);
                }}
              >
                All Custom
              </button>
              {lists.map((l) => (
                <button
                  key={l.id}
                  className={activeListId === l.id ? "on" : ""}
                  onClick={() => {
                    setListPreset("all");
                    setActiveListId(l.id);
                  }}
                >
                  {l.name}
                </button>
              ))}
              <button
                className={listPreset === "deleted" ? "on" : ""}
                onClick={() => {
                  setListPreset("deleted");
                  setActiveListId(null);
                }}
              >
                Recently Deleted
              </button>
            </aside>

            <div className="dash-list-main">
              {listPreset === "deleted" ? (
                <>
                  <div className="dd-toolbar">
                    <h3 style={{ margin: 0 }}>Recently Deleted</h3>
                    <span className="muted">
                      Dashboards in this list are permanently deleted after 30 days · {deleted.length}{" "}
                      total
                    </span>
                    <div className="restore-to">
                      <label>Restore to</label>
                      <select
                        id="restore-list"
                        defaultValue=""
                        onChange={(e) => {
                          if (e.target.value === "__active__") void restoreSelected();
                          else if (e.target.value) void restoreSelected(e.target.value);
                          e.target.value = "";
                        }}
                      >
                        <option value="" disabled>
                          Restore to…
                        </option>
                        <option value="__active__">All dashboards</option>
                        {lists.map((l) => (
                          <option key={l.id} value={l.id}>
                            {l.name}
                          </option>
                        ))}
                      </select>
                    </div>
                  </div>
                  <table className="dd-table">
                    <thead>
                      <tr>
                        <th style={{ width: 36 }} />
                        <th>Name</th>
                        <th>Author</th>
                        <th>Recoverable until</th>
                      </tr>
                    </thead>
                    <tbody>
                      {deleted.map((b) => (
                        <tr key={b.id} className={selectedDeleted.has(b.id) ? "selected" : ""}>
                          <td>
                            <input
                              type="checkbox"
                              checked={selectedDeleted.has(b.id)}
                              onChange={() => toggleDeletedSelect(b.id)}
                            />
                          </td>
                          <td>
                            <strong>{b.name}</strong>
                            <div className="muted">{b.description}</div>
                          </td>
                          <td>{b.author || "—"}</td>
                          <td>
                            {b.recoverable_until
                              ? new Date(b.recoverable_until).toLocaleString()
                              : "—"}
                          </td>
                        </tr>
                      ))}
                      {!deleted.length && (
                        <tr>
                          <td colSpan={4} className="muted">
                            No recently deleted dashboards
                          </td>
                        </tr>
                      )}
                    </tbody>
                  </table>
                </>
              ) : (
                <>
                  <div className="dd-toolbar">
                    <input
                      placeholder="Filter dashboards"
                      value={listFilter}
                      onChange={(e) => setListFilter(e.target.value)}
                    />
                    <span>{filteredBoards.length} boards</span>
                    <button type="button" onClick={openCreateModal}>
                      New Dashboard
                    </button>
                  </div>
                  <table className="dd-table">
                    <thead>
                      <tr>
                        <th>Name</th>
                        <th>Layout</th>
                        <th>Widgets</th>
                        <th>Tags</th>
                        <th>Template vars</th>
                      </tr>
                    </thead>
                    <tbody>
                      {filteredBoards.map((b) => (
                        <tr
                          key={b.id}
                          className={b.id === activeBoardId ? "selected" : ""}
                          onClick={() => {
                            onSelectBoard(b.id);
                            setView("board");
                          }}
                        >
                          <td>
                            <strong>{b.name}</strong>
                            <div className="muted">{b.description}</div>
                          </td>
                          <td>
                            <code>{b.layout_type || "dashboard"}</code>
                          </td>
                          <td>{b.widgets?.length ?? 0}</td>
                          <td>{(b.tags || []).join(", ") || "—"}</td>
                          <td>
                            {(b.template_variables || []).map((t) => `$${t.name}`).join(" ") || "—"}
                          </td>
                        </tr>
                      ))}
                    </tbody>
                  </table>
                </>
              )}
            </div>
          </div>
        </div>
      )}

      {view === "guide" && guide && (
        <div className="dash-guide">
          <h2>{guide.title}</h2>
          <p>{guide.overview}</p>
          <p>
            Docs:{" "}
            <a href={guide.docs} target="_blank" rel="noreferrer">
              {guide.docs}
            </a>
            {" · "}
            <a
              href="https://docs.datadoghq.com/dashboards/graph_insights/"
              target="_blank"
              rel="noreferrer"
            >
              Graph Insights
            </a>
          </p>
          <div className="guide-grid">
            <div>
              <h3>Layouts</h3>
              <ul>
                {guide.layouts.map((l) => (
                  <li key={l.id}>
                    <strong>{l.label}</strong> — {l.desc}
                  </li>
                ))}
              </ul>
            </div>
            <div>
              <h3>Features</h3>
              <ul>
                {guide.features.map((f) => (
                  <li key={f.id}>
                    <code>{f.id}</code> — {f.desc}
                    <div className="muted">{f.path}</div>
                  </li>
                ))}
              </ul>
            </div>
            <div>
              <h3>Refresh rates</h3>
              <table className="dd-table compact">
                <thead>
                  <tr>
                    <th>Timeframe</th>
                    <th>Refresh</th>
                  </tr>
                </thead>
                <tbody>
                  {guide.refresh_rates.map((r) => (
                    <tr key={r.timeframe}>
                      <td>{r.timeframe}</td>
                      <td>{r.secs}s</td>
                    </tr>
                  ))}
                </tbody>
              </table>
            </div>
            <div>
              <h3>Widget types</h3>
              <div className="chip-row">
                {guide.widget_types.map((t) => (
                  <code key={t}>{t}</code>
                ))}
              </div>
            </div>
          </div>
        </div>
      )}

      {view === "board" && (
        <div className={`dash-board-layout ${showInsights ? "with-insights" : ""}`}>
          <div>
            <div className="dd-filter-bar dash-board-bar">
              <select value={activeBoardId ?? ""} onChange={(e) => onSelectBoard(e.target.value)}>
                {boards.map((b) => (
                  <option key={b.id} value={b.id}>
                    {b.name}
                  </option>
                ))}
              </select>
              <span className="layout-pill">{rendered?.layout_type || "dashboard"}</span>
              {(rendered?.template_variables || []).map((tv) => (
                <label key={tv.name} className="tpl-var">
                  <span>${tv.name}</span>
                  <select
                    value={vars[tv.name] ?? tv.default}
                    onChange={(e) => setVars((v) => ({ ...v, [tv.name]: e.target.value }))}
                  >
                    {(tv.available_values.length ? tv.available_values : [tv.default, "*"]).map(
                      (v) => (
                        <option key={v} value={v}>
                          {v}
                        </option>
                      ),
                    )}
                  </select>
                </label>
              ))}
              <div className="add-var-wrap">
                <button
                  type="button"
                  className="ghost add-var-btn"
                  onClick={() => {
                    setVarName("env");
                    setVarTag("env");
                    setVarValues("prod,staging,dev,*");
                    setVarType("filter");
                    setShowAddVar((v) => !v);
                  }}
                >
                  + Add Variable
                </button>
                {showAddVar && (
                  <div className="add-var-popover" role="dialog" aria-label="Add Variable">
                    <div className="add-var-type">
                      <span>Variable type</span>
                      <div className="seg">
                        <button
                          type="button"
                          className={varType === "filter" ? "on" : ""}
                          onClick={() => setVarType("filter")}
                        >
                          Filter
                        </button>
                        <button
                          type="button"
                          className={varType === "groupby" ? "on" : ""}
                          onClick={() => setVarType("groupby")}
                        >
                          Group by <em className="new-badge">NEW</em>
                        </button>
                      </div>
                    </div>
                    <label>
                      Variable tag
                      <input
                        value={varTag}
                        onChange={(e) => {
                          setVarTag(e.target.value);
                          if (!varName || varName === varTag) setVarName(e.target.value);
                        }}
                        placeholder="Any tag or attribute"
                      />
                    </label>
                    <label>
                      Variable name *
                      <input
                        value={varName}
                        onChange={(e) => setVarName(e.target.value)}
                        placeholder="Display name"
                      />
                      <span className="muted">
                        Used in queries as <code>{"$" + (varName || "Name")}</code>
                      </span>
                    </label>
                    <label>
                      Values
                      <input
                        value={varValues}
                        onChange={(e) => setVarValues(e.target.value)}
                        placeholder="prod,staging,dev,*"
                      />
                    </label>
                    <div className="add-var-actions">
                      <button type="button" className="ghost" onClick={() => setShowAddVar(false)}>
                        Cancel
                      </button>
                      <button type="button" onClick={() => void saveTemplateVariable()}>
                        Save
                      </button>
                    </div>
                  </div>
                )}
              </div>
              {(rendered?.saved_views?.length ?? 0) > 0 && (
                <select
                  defaultValue=""
                  onChange={(e) => e.target.value && applySavedView(e.target.value)}
                >
                  <option value="">Saved views…</option>
                  {rendered!.saved_views.map((v) => (
                    <option key={v.id} value={v.id}>
                      {v.name}
                    </option>
                  ))}
                </select>
              )}
              <button type="button" className="ghost" onClick={() => setShowEditGraph(true)}>
                + Add Widgets
              </button>
              <button
                className={showInsights ? "ghost on" : "ghost"}
                onClick={() => setShowInsights((v) => !v)}
              >
                Investigate{issues.length ? ` (${issues.length})` : ""}
              </button>
              <button className="ghost" onClick={toggleShare}>
                {rendered?.share?.public ? "Unshare" : "Share"}
              </button>
              <button className="ghost" onClick={copyWidgets}>
                Clipboard
              </button>
              <button className="ghost" onClick={() => void softDeleteActive()}>
                Delete
              </button>
              <span className="muted">
                {rendered
                  ? `${rendered.widgets.length} widgets · ${issues.length} issues · refresh ${rendered.refresh_secs}s`
                  : "Loading…"}
              </span>
            </div>
            {showPalette && (
              <div className="sb-widget-palette essential-widgets" title="Essential widgets from the tutorial">
                <span className="palette-label">Essential Widgets</span>
                {ESSENTIAL_WIDGETS.map(([type, label, defaults]) => (
                  <button
                    key={type}
                    type="button"
                    className="sb-palette-item"
                    onClick={() => void addPaletteWidget(type, label, defaults)}
                  >
                    {label}
                  </button>
                ))}
                <button
                  type="button"
                  className="sb-palette-item golden"
                  onClick={() => void addGoldenSignals()}
                >
                  Golden Signals ×4
                </button>
                <button type="button" className="sb-palette-item" onClick={() => setShowEditGraph(true)}>
                  Edit Graph…
                </button>
                {rendered?.layout_type === "screenboard" && (
                  <>
                    <button
                      type="button"
                      className="sb-palette-item"
                      onClick={() =>
                        void addPaletteWidget("note", "Notes", { text: "Investigation note" })
                      }
                    >
                      Notes
                    </button>
                    <button
                      type="button"
                      className="sb-palette-item"
                      onClick={() =>
                        void addPaletteWidget("check_status", "Check Status", {
                          metric: "http.server.duration",
                          aggregation: "avg",
                        })
                      }
                    >
                      Check Status
                    </button>
                  </>
                )}
                <button type="button" className="ghost palette-hide" onClick={() => setShowPalette(false)}>
                  Hide
                </button>
              </div>
            )}
            {!showPalette && (
              <button type="button" className="ghost palette-show" onClick={() => setShowPalette(true)}>
                Show Essential Widgets
              </button>
            )}
            {(rendered?.annotations?.length ?? 0) > 0 && (
              <div className="dash-annotations">
                <span>Overlays:</span>
                {rendered!.annotations.map((a) => (
                  <code key={a.id} style={{ borderColor: a.color || undefined }}>
                    {a.label} @ {new Date(a.timestamp_ms).toLocaleTimeString()}
                  </code>
                ))}
              </div>
            )}
            <div
              className={`dash-grid studio${
                rendered?.layout_type === "screenboard"
                  ? " screenboard"
                  : rendered?.layout_type === "timeboard"
                    ? " timeboard"
                    : ""
              }`}
            >
              {widgetsWithAnomalies.map((w) => (
                <div
                  key={w.id}
                  className={`dw-cell${focusWidgetId === w.id ? " dw-focus-wrap" : ""}${
                    w.type === "group" ? " is-group" : ""
                  }${
                    w.type === "query_value" ||
                    w.type === "change" ||
                    w.type === "slo" ||
                    w.type === "check_status" ||
                    w.type === "alert_graph"
                      ? " is-kpi"
                      : ""
                  }${
                    w.type === "timeseries" ||
                    w.type === "heatmap" ||
                    w.type === "distribution" ||
                    w.type === "scatter_plot" ||
                    w.type === "hostmap"
                      ? " is-chart"
                      : ""
                  }`}
                  id={`widget-${w.id}`}
                  style={gridStyle(w)}
                >
                  <WidgetCard
                    w={w}
                    layoutOnRoot={false}
                    onCorrelate={
                      w.type === "timeseries" || w.type === "heatmap" || w.type === "distribution"
                        ? () => {
                            setCorrMetric(w.series[0]?.metric || "http.server.duration");
                            setShowInsights(true);
                          }
                        : undefined
                    }
                  />
                </div>
              ))}
              {!rendered && <div className="dd-empty">Loading dashboard…</div>}
            </div>
          </div>
          {showInsights && rendered && (
            <GraphInsightsPanel
              boardId={rendered.id}
              rangeMs={rangeMs}
              startMs={rendered.start_ms}
              endMs={rendered.end_ms}
              issues={issues}
              autoDetect={autoDetect}
              onAutoDetectChange={(v) => {
                setAutoDetect(v);
                if (!v) setIssues([]);
              }}
              onFocusWidget={(id) => {
                setFocusWidgetId(id);
                document.getElementById(`widget-${id}`)?.scrollIntoView({ behavior: "smooth", block: "center" });
              }}
              selectedMetric={corrMetric}
            />
          )}
        </div>
      )}

      {showCreateModal && (
        <div className="dd-modal-backdrop" onClick={() => !creating && setShowCreateModal(false)}>
          <div
            className="dd-create-dash-modal wide"
            role="dialog"
            aria-labelledby="create-dash-title"
            onClick={(e) => e.stopPropagation()}
          >
            <header>
              <h2 id="create-dash-title">Create a Dashboard</h2>
              <button
                className="dd-modal-close"
                aria-label="Close"
                onClick={() => setShowCreateModal(false)}
              >
                ×
              </button>
            </header>
            <label className="dd-create-name">
              <span>Dashboard Name</span>
              <input
                autoFocus
                value={newName}
                onChange={(e) => setNewName(e.target.value)}
                placeholder="[Team] - [Service] - [Purpose]"
              />
              <span className="muted">
                Tip: Platform - API Gateway - Performance · Start with Timeboard for debugging
              </span>
            </label>
            <div className="dd-layout-cards three">
              <button
                type="button"
                className={`dd-layout-card primary${newLayout === "dashboard" ? " selected" : ""}`}
                disabled={creating || !newName.trim()}
                onClick={() => {
                  setNewLayout("dashboard");
                  void createBoard("dashboard");
                }}
              >
                <div className="dd-layout-preview dashboard" aria-hidden>
                  <i className="lg" />
                  <i />
                  <i />
                  <i />
                  <i />
                </div>
                <div className="dd-layout-copy">
                  <strong>Snap widgets into place on a grid</strong>
                </div>
                <div className="dd-layout-card-head">New Dashboard</div>
              </button>
              <button
                type="button"
                className={`dd-layout-card${newLayout === "timeboard" ? " selected" : ""}`}
                disabled={creating || !newName.trim()}
                onClick={() => {
                  setNewLayout("timeboard");
                  void createBoard("timeboard");
                }}
              >
                <div className="dd-layout-preview timeboard" aria-hidden>
                  <i />
                  <i />
                  <i />
                  <i />
                  <i />
                  <i />
                </div>
                <div className="dd-layout-copy">
                  <strong>Automatic layout that fits your browser</strong>
                </div>
                <div className="dd-layout-card-head alt">New Timeboard</div>
              </button>
              <button
                type="button"
                className={`dd-layout-card${newLayout === "screenboard" ? " selected" : ""}`}
                disabled={creating || !newName.trim()}
                onClick={() => {
                  setNewLayout("screenboard");
                  void createBoard("screenboard");
                }}
              >
                <div className="dd-layout-preview screenboard" aria-hidden>
                  <i className="a" />
                  <i className="b" />
                  <i className="c" />
                  <i className="d" />
                  <i className="e" />
                </div>
                <div className="dd-layout-copy">
                  <strong>Pixel-level precision on a scrollable canvas</strong>
                </div>
                <div className="dd-layout-card-head alt">New Screenboard</div>
              </button>
            </div>
            {creating && <p className="muted dd-create-status">Creating…</p>}
          </div>
        </div>
      )}

      <EditGraphModal
        open={showEditGraph}
        metrics={
          metricNames.length
            ? metricNames
            : ["http.server.duration", "http.server.request.count", "system.cpu.user", "system.load.1"]
        }
        rangeMs={rangeMs}
        onClose={() => setShowEditGraph(false)}
        onSave={(draft) => void saveEditedGraph(draft)}
      />
    </section>
  );
}
