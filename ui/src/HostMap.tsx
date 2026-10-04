import { useCallback, useEffect, useMemo, useState, type ReactNode } from "react";
import {
  api,
  type HostmapCell,
  type HostmapChild,
  type HostmapSuggestedQuery,
  type InfraHostmap,
} from "./api";

type Resource = "host" | "pod" | "container" | "cluster";
type Secondary = "none" | "pod" | "container";

const PASTELS = [
  { bg: "rgba(91, 145, 235, 0.12)", label: "#3d6bb3", border: "rgba(91, 145, 235, 0.28)" },
  { bg: "rgba(232, 120, 140, 0.12)", label: "#c44a62", border: "rgba(232, 120, 140, 0.28)" },
  { bg: "rgba(155, 120, 220, 0.12)", label: "#6b4aaf", border: "rgba(155, 120, 220, 0.28)" },
  { bg: "rgba(80, 190, 180, 0.14)", label: "#2a8f86", border: "rgba(80, 190, 180, 0.3)" },
  { bg: "rgba(240, 190, 80, 0.16)", label: "#b8860b", border: "rgba(240, 190, 80, 0.35)" },
  { bg: "rgba(100, 180, 100, 0.14)", label: "#3d8b3d", border: "rgba(100, 180, 100, 0.3)" },
  { bg: "rgba(230, 140, 90, 0.14)", label: "#c45c2a", border: "rgba(230, 140, 90, 0.3)" },
  { bg: "rgba(120, 160, 220, 0.14)", label: "#3a6aa8", border: "rgba(120, 160, 220, 0.3)" },
];

const DEFAULT_QUERY: HostmapSuggestedQuery = {
  id: "cpu-hosts",
  title: "What is the CPU usage across my infrastructure?",
  resource: "host",
  secondary: "none",
  fill_by: "CPU usage",
  size_by: "—",
  group_by: ["availability-zone"],
  filter: "",
};

function fillMetric(h: HostmapCell, fillBy: string): number {
  switch (fillBy) {
    case "Memory usage":
      return h.mem_pct ?? 0;
    case "Disk usage":
      return h.disk_pct ?? 0;
    case "Load 15":
      return Math.min(100, (h.load_15 ?? 0) * 25);
    case "Error logs":
      return h.error_logs ?? 0;
    case "Readiness":
      return h.readiness ?? 100;
    case "Cost score":
      return h.cost_score ?? 0;
    case "Agent outdated":
      return h.agent_outdated ?? 0;
    default:
      return h.cpu_pct ?? h.fill ?? 0;
  }
}

function childFill(c: HostmapChild, fillBy: string): number {
  if (fillBy === "Readiness") return c.readiness ?? 0;
  if (fillBy === "Memory usage") return c.mem_pct ?? 0;
  if (fillBy === "Error logs") return Math.min(100, (c.restarts ?? 0) * 20);
  return c.cpu_pct ?? 0;
}

function metricColor(pct: number, invert = false): string {
  const t = Math.max(0, Math.min(100, invert ? 100 - pct : pct)) / 100;
  if (t < 0.35) return lerp("#b8e6c4", "#7dcea0", t / 0.35);
  if (t < 0.55) return lerp("#7dcea0", "#52be80", (t - 0.35) / 0.2);
  if (t < 0.72) return lerp("#52be80", "#f0c94a", (t - 0.55) / 0.17);
  if (t < 0.88) return lerp("#f0c94a", "#e89050", (t - 0.72) / 0.16);
  return lerp("#e89050", "#e04840", (t - 0.88) / 0.12);
}

function lerp(a: string, b: string, t: number): string {
  const pa = parse(a);
  const pb = parse(b);
  return `rgb(${Math.round(pa[0] + (pb[0] - pa[0]) * t)},${Math.round(pa[1] + (pb[1] - pa[1]) * t)},${Math.round(pa[2] + (pb[2] - pa[2]) * t)})`;
}

function parse(c: string): [number, number, number] {
  const n = c.replace("#", "");
  return [parseInt(n.slice(0, 2), 16), parseInt(n.slice(2, 4), 16), parseInt(n.slice(4, 6), 16)];
}

function flatHex(cx: number, cy: number, size: number): string {
  return Array.from({ length: 6 }, (_, i) => {
    const a = (Math.PI / 180) * (60 * i);
    return `${cx + size * Math.cos(a)},${cy + size * Math.sin(a)}`;
  }).join(" ");
}

function packHexes(n: number, size: number) {
  const h = Math.sqrt(3) * size;
  const horiz = size * 1.5;
  const cols = Math.max(1, Math.ceil(Math.sqrt(n * 1.25)));
  return Array.from({ length: n }, (_, i) => {
    const col = i % cols;
    const row = Math.floor(i / cols);
    return { i, x: col * horiz + size, y: row * h + (col % 2) * (h / 2) + size };
  });
}

function hostField(h: HostmapCell, key: string): string {
  switch (key) {
    case "availability-zone":
    case "az":
      return h.az || "no availability-zone";
    case "env":
      return h.env || "untagged";
    case "service":
      return h.service || "untagged";
    case "instance-type":
      return h.instance_type || "unknown";
    case "kube_namespace":
      return h.tags?.kube_namespace || h.kube_namespace || "none";
    case "cloud_provider":
      return h.tags?.cloud_provider || "unknown";
    default:
      return h.tags?.[key] || h.group || "ungrouped";
  }
}

function wildcardMatch(value: string, pattern: string): boolean {
  const v = value.toLowerCase();
  const p = pattern.toLowerCase();
  if (!p.includes("*")) return v === p || v.includes(p);
  const re = new RegExp("^" + p.split("*").map((s) => s.replace(/[.*+?^${}()|[\]\\]/g, "\\$&")).join(".*") + "$");
  return re.test(v);
}

function matchesFilter(h: HostmapCell, query: string): boolean {
  const q = query.trim();
  if (!q) return true;
  if (/\s+OR\s+/i.test(q) && !q.includes("(")) {
    return q.split(/\s+OR\s+/i).some((p) => matchesFilter(h, p.trim()));
  }
  if (q.startsWith("(") && q.endsWith(")")) return matchesFilter(h, q.slice(1, -1));
  return q.split(/\s+AND\s+/i).every((part) => matchAtom(h, part.trim()));
}

function matchAtom(h: HostmapCell, raw: string): boolean {
  let q = raw.trim();
  if (!q) return true;
  let negate = false;
  if (/^NOT\s+/i.test(q) || q.startsWith("!")) {
    negate = true;
    q = q.replace(/^NOT\s+/i, "").replace(/^!/, "").trim();
  }
  if (q.startsWith("tags.")) {
    const rest = q.slice(5);
    const [k, ...vr] = rest.split(":");
    const val = vr.join(":");
    const actual =
      k === "availability-zone"
        ? h.az
        : k === "instance-type"
          ? h.instance_type
          : k === "env"
            ? h.env
            : k === "service"
              ? h.service
              : k === "agent_version"
                ? h.agent_version
                : (h.tags?.[k] ?? "");
    if (val === "*" || val === "") {
      const present = Boolean(actual);
      return negate ? !present : present;
    }
    const hit = wildcardMatch(actual || "", val);
    return negate ? !hit : hit;
  }
  if (q.includes(":")) {
    const [k, ...vr] = q.split(":");
    const val = vr.join(":");
    const actual =
      k === "env" ? h.env : k === "service" ? h.service : k === "availability-zone" || k === "az" ? h.az : (h.tags?.[k] ?? "");
    const hit = wildcardMatch(actual || "", val);
    return negate ? !hit : hit;
  }
  const blob = [h.name, h.alias, h.service, h.env, h.az, ...(h.apps ?? [])].join(" ").toLowerCase();
  const hit = blob.includes(q.toLowerCase());
  return negate ? !hit : hit;
}

type NestedGroup = { key: string; depth: number; cells: HostmapCell[]; children: NestedGroup[] };

function nestGroups(cells: HostmapCell[], keys: string[], depth = 0): NestedGroup[] {
  if (!keys.length) return cells.length ? [{ key: "", depth, cells, children: [] }] : [];
  const [head, ...rest] = keys;
  const map = new Map<string, HostmapCell[]>();
  for (const c of cells) {
    const k = hostField(c, head);
    if (!map.has(k)) map.set(k, []);
    map.get(k)!.push(c);
  }
  return Array.from(map.entries())
    .sort((a, b) => b[1].length - a[1].length || a[0].localeCompare(b[0]))
    .map(([key, groupCells]) => ({
      key,
      depth,
      cells: rest.length ? [] : groupCells,
      children: rest.length ? nestGroups(groupCells, rest, depth + 1) : [],
    }));
}

function fmtPct(n: number): string {
  if (n < 1) return n.toFixed(3) + "%";
  if (n < 10) return n.toFixed(2) + "%";
  return n.toFixed(2) + "%";
}

function timeLabel(): string {
  const end = new Date();
  const start = new Date(end.getTime() - 15 * 60_000);
  const fmt = (d: Date) =>
    d.toLocaleString(undefined, { month: "short", day: "numeric", hour: "numeric", minute: "2-digit" });
  return `15m ${fmt(start)} – ${fmt(end)}`;
}

export function HostMap({ forceResource }: { forceResource?: Resource } = {}) {
  const [data, setData] = useState<InfraHostmap | null>(null);
  const [resource, setResource] = useState<Resource>(forceResource || "host");
  const [secondary, setSecondary] = useState<Secondary>("none");
  const [secondaryFill, setSecondaryFill] = useState("Readiness");
  const [fillBy, setFillBy] = useState("CPU usage");
  const [sizeBy, setSizeBy] = useState("—");
  const [groupByTags, setGroupByTags] = useState<string[]>(forceResource === "container" ? ["kube_namespace"] : ["availability-zone"]);
  const [filter, setFilter] = useState("");
  const [filterDraft, setFilterDraft] = useState("");
  const [activeQueryId, setActiveQueryId] = useState(forceResource === "container" ? "cpu-containers" : "cpu-hosts");
  const [showQueries, setShowQueries] = useState(false);
  const [editing, setEditing] = useState(false);
  const [groupDraft, setGroupDraft] = useState("");
  const [showGroupSuggest, setShowGroupSuggest] = useState(false);
  const [hover, setHover] = useState<{ id: string; x: number; y: number; value: number } | null>(null);
  const [selected, setSelected] = useState<HostmapCell | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [hexSize, setHexSize] = useState(14);
  const [rangeLabel] = useState(timeLabel);

  const load = useCallback(async () => {
    try {
      const hm = await api.infraHostmap(groupByTags[0] || "availability-zone", resource);
      setData(hm);
      setError(null);
    } catch (e) {
      setError(e instanceof Error ? e.message : "Host map failed");
    }
  }, [groupByTags, resource]);

  useEffect(() => {
    if (forceResource) {
      setResource(forceResource);
      if (forceResource === "container") {
        setGroupByTags((g) => (g[0] === "availability-zone" ? ["kube_namespace"] : g));
        setActiveQueryId((id) => (id === "cpu-hosts" ? "cpu-containers" : id));
      }
    }
  }, [forceResource]);

  useEffect(() => {
    void load();
    const id = setInterval(() => void load(), 12_000);
    return () => clearInterval(id);
  }, [load]);

  // Prefer the docs-style title
  useEffect(() => {
    const q = data?.suggested_queries?.find((s) => s.id === "cpu-hosts");
    if (q && activeQueryId === "cpu-hosts") {
      /* keep */
    }
  }, [data, activeQueryId]);

  const suggested = useMemo(() => {
    const list = data?.suggested_queries ?? [DEFAULT_QUERY];
    return list.map((q) =>
      q.id === "cpu-hosts"
        ? { ...q, title: "What is the CPU usage across my infrastructure?" }
        : q,
    );
  }, [data]);

  const fillOpts = data?.fill_options ?? ["CPU usage", "Memory usage", "Error logs", "Readiness", "Cost score"];
  const groupOpts = data?.group_options ?? ["availability-zone", "instance-type", "env", "service"];

  const filtered = useMemo(() => {
    let cells = data?.hosts ?? [];
    const f = filter || filterDraft;
    if (f.trim()) cells = cells.filter((h) => matchesFilter(h, f));
    return cells;
  }, [data, filter, filterDraft]);

  const nested = useMemo(() => nestGroups(filtered, groupByTags), [filtered, groupByTags]);

  const fillRange = useMemo(() => {
    if (!filtered.length) return { min: 0.065, max: 98.05 };
    const vals = filtered.map((h) => fillMetric(h, fillBy));
    return { min: Math.min(...vals), max: Math.max(...vals) };
  }, [filtered, fillBy]);

  const activeTitle =
    suggested.find((s) => s.id === activeQueryId)?.title ?? "What is the CPU usage across my infrastructure?";

  function applySuggested(q: HostmapSuggestedQuery) {
    setActiveQueryId(q.id);
    setResource((q.resource as Resource) || "host");
    setSecondary((q.secondary as Secondary) || "none");
    setFillBy(q.fill_by);
    setSizeBy(q.size_by || "—");
    setGroupByTags(q.group_by?.length ? [...q.group_by] : ["availability-zone"]);
    setFilter(q.filter || "");
    setFilterDraft("");
    if (q.secondary_fill) setSecondaryFill(q.secondary_fill);
    setShowQueries(false);
  }

  let pastelIdx = 0;

  function renderCluster(g: NestedGroup): ReactNode {
    if (g.children.length) {
      return (
        <div key={`n-${g.depth}-${g.key}`} className="hmv-nest">
          {g.children.map((c) => renderCluster(c))}
        </div>
      );
    }
    const tone = PASTELS[pastelIdx++ % PASTELS.length];
    const n = g.cells.length;
    const size = hexSize;
    const positions = packHexes(n, size);
    const pad = size * 0.55;
    const maxX = Math.max(...positions.map((p) => p.x), size) + size + pad;
    const maxY = Math.max(...positions.map((p) => p.y), size) + size + pad;
    const label = g.key || "ungrouped";
    const unit = resource === "host" ? "hosts" : `${resource}s`;

    return (
      <div
        key={`c-${label}`}
        className="hmv-group"
        style={{ background: tone.bg, borderColor: tone.border }}
      >
        <div className="hmv-group-label" style={{ color: tone.label }}>
          {label} <strong>{n}</strong> {n === 1 ? unit.slice(0, -1) : unit}
        </div>
        <svg width={maxX} height={maxY} viewBox={`0 0 ${maxX} ${maxY}`} className="hmv-svg">
          {g.cells.map((h, idx) => {
            const pos = positions[idx];
            const val = fillMetric(h, fillBy);
            const color = metricColor(val, fillBy === "Readiness");
            const kids =
              secondary === "none"
                ? []
                : (h.children ?? []).filter((c) => (secondary === "pod" ? c.kind === "pod" || c.kube_namespace : true));
            return (
              <g
                key={h.id}
                onMouseEnter={(e) => {
                  const canvas = e.currentTarget.closest(".hmv-canvas") as HTMLElement;
                  const rect = canvas.getBoundingClientRect();
                  setHover({ id: h.id, x: e.clientX - rect.left, y: e.clientY - rect.top, value: val });
                }}
                onMouseLeave={() => setHover(null)}
                onClick={(e) => {
                  e.stopPropagation();
                  setSelected(h);
                }}
                style={{ cursor: "pointer" }}
              >
                <polygon
                  points={flatHex(pos.x, pos.y, size * 0.96)}
                  fill={color}
                  stroke="#ffffff"
                  strokeWidth={0.8}
                />
                {kids.slice(0, 3).map((c, ci) => {
                  const ang = (Math.PI / 180) * (ci * 40 - 40);
                  const r = size * 0.35;
                  return (
                    <circle
                      key={c.id}
                      cx={pos.x + Math.cos(ang) * r * 0.2}
                      cy={pos.y + Math.sin(ang) * r * 0.15}
                      r={1.6}
                      fill={metricColor(childFill(c, secondaryFill), secondaryFill === "Readiness")}
                      stroke="#fff"
                      strokeWidth={0.4}
                    />
                  );
                })}
                {(h.error_logs ?? 0) > 40 && (
                  <rect x={pos.x - 2} y={pos.y - 2} width={4} height={2.2} rx={0.4} fill="#5b91eb" />
                )}
              </g>
            );
          })}
        </svg>
      </div>
    );
  }

  return (
    <div className="hmv">
      <header className="hmv-top">
        <div className="hmv-top-left">
          <button type="button" className="hmv-brand">
            Host Map <span className="chev">▾</span>
          </button>
          <div className="hmv-query-wrap">
            <button type="button" className="hmv-query" onClick={() => setShowQueries((v) => !v)}>
              {activeTitle} <span className="chev">▾</span>
            </button>
            {showQueries && (
              <div className="hmv-query-menu">
                <div className="hmv-query-menu-head">
                  <span>Suggested queries</span>
                  <button
                    type="button"
                    onClick={() => {
                      setActiveQueryId("custom");
                      setEditing(true);
                      setShowQueries(false);
                    }}
                  >
                    Create
                  </button>
                </div>
                <ul>
                  {suggested.map((q) => (
                    <li key={q.id}>
                      <button type="button" className={q.id === activeQueryId ? "on" : ""} onClick={() => applySuggested(q)}>
                        {q.title}
                      </button>
                    </li>
                  ))}
                </ul>
              </div>
            )}
          </div>
        </div>
        <div className="hmv-top-right">
          <button type="button" className="hmv-range">
            {rangeLabel}
          </button>
          <button type="button" className="hmv-edit" onClick={() => setEditing((v) => !v)}>
            ✎ Edit
          </button>
        </div>
      </header>

      <div className="hmv-controls">
        <label>
          Main resource
          <select
            value={resource}
            disabled={!!forceResource}
            onChange={(e) => {
              setResource(e.target.value as Resource);
              setActiveQueryId("custom");
            }}
          >
            {(data?.resource_options ?? ["host", "pod", "container", "cluster"]).map((r) => (
              <option key={r} value={r}>
                {r.charAt(0).toUpperCase() + r.slice(1)}
              </option>
            ))}
          </select>
        </label>
        <label>
          Fill by
          <select
            value={fillBy}
            onChange={(e) => {
              setFillBy(e.target.value);
              setActiveQueryId("custom");
            }}
          >
            {fillOpts.map((o) => (
              <option key={o} value={o}>
                {o}
              </option>
            ))}
          </select>
        </label>
        <div className="hmv-scale" title={`${fillBy} scale`}>
          <span>{fmtPct(fillRange.min)}</span>
          <i />
          <span>{fmtPct(fillRange.max)}</span>
        </div>
        <div className="hmv-groupby">
          <span className="lbl">Group by</span>
          {groupByTags.map((g) => (
            <span key={g} className="hmv-chip">
              tags.{g}
              <button type="button" onClick={() => setGroupByTags((p) => p.filter((x) => x !== g))}>
                ×
              </button>
            </span>
          ))}
          <div className="hmv-suggest-wrap">
            <input
              placeholder="Add…"
              value={groupDraft}
              onChange={(e) => {
                setGroupDraft(e.target.value);
                setShowGroupSuggest(true);
              }}
              onFocus={() => setShowGroupSuggest(true)}
            />
            {showGroupSuggest && (
              <ul className="hmv-suggest">
                {groupOpts
                  .filter((t) => !groupByTags.includes(t))
                  .filter((t) => !groupDraft || t.includes(groupDraft.toLowerCase()))
                  .map((t) => (
                    <li key={t}>
                      <button
                        type="button"
                        onClick={() => {
                          setGroupByTags((p) => [...p, t]);
                          setGroupDraft("");
                          setShowGroupSuggest(false);
                        }}
                      >
                        tags.{t}
                      </button>
                    </li>
                  ))}
              </ul>
            )}
          </div>
        </div>
        <label className="hmv-filter">
          Filter
          <input
            placeholder="None"
            value={filterDraft || filter}
            onChange={(e) => setFilterDraft(e.target.value)}
            onKeyDown={(e) => {
              if (e.key === "Enter") setFilter(filterDraft.trim());
            }}
            onBlur={() => {
              if (filterDraft.trim()) setFilter(filterDraft.trim());
            }}
          />
        </label>
        {secondary !== "none" && (
          <label>
            Secondary
            <select value={secondary} onChange={(e) => setSecondary(e.target.value as Secondary)}>
              <option value="none">None</option>
              <option value="pod">Pod</option>
              <option value="container">Container</option>
            </select>
          </label>
        )}
      </div>

      {editing && (
        <div className="hmv-draft">
          <label>
            Secondary resource
            <select
              value={secondary}
              onChange={(e) => {
                setSecondary(e.target.value as Secondary);
                if (e.target.value !== "none") setSizeBy("—");
              }}
            >
              <option value="none">— None —</option>
              <option value="pod">Pod</option>
              <option value="container">Container</option>
            </select>
          </label>
          {secondary !== "none" && (
            <label>
              Secondary fill
              <select value={secondaryFill} onChange={(e) => setSecondaryFill(e.target.value)}>
                {fillOpts.map((o) => (
                  <option key={o} value={o}>
                    {o}
                  </option>
                ))}
              </select>
            </label>
          )}
          {secondary === "none" && (
            <label>
              Size by
              <select value={sizeBy} onChange={(e) => setSizeBy(e.target.value)}>
                {(data?.size_options ?? ["—", "CPU usage", "Memory usage", "Error logs", "Cost score"]).map((o) => (
                  <option key={o} value={o}>
                    {o}
                  </option>
                ))}
              </select>
            </label>
          )}
          <button type="button" onClick={() => setEditing(false)}>
            Done
          </button>
        </div>
      )}

      {error && (
        <div className="dd-banner">
          <strong>Host map failed</strong>
          <span>{error}</span>
        </div>
      )}

      <div
        className="hmv-canvas"
        onClick={() => {
          setShowQueries(false);
          setShowGroupSuggest(false);
        }}
      >
        <div className="hmv-groups">
          {nested.map((g) => renderCluster(g))}
          {!filtered.length && <p className="hmv-empty">No {resource}s match this query</p>}
        </div>

        {hover && (
          <div className="hmv-tip" style={{ left: hover.x + 12, top: hover.y - 8 }}>
            <strong>{hover.id}</strong>
            <span>
              {fillBy} {hover.value.toFixed(1)}%
            </span>
          </div>
        )}

        <aside className="hmv-rail">
          <button type="button" title="Zoom in" onClick={() => setHexSize((s) => Math.min(28, s + 2))}>
            +
          </button>
          <button type="button" title="Zoom out" onClick={() => setHexSize((s) => Math.max(8, s - 2))}>
            −
          </button>
          <button type="button" title="Fit" onClick={() => setHexSize(14)}>
            ▢
          </button>
          <button type="button" title="Settings" onClick={() => setEditing(true)}>
            ⚙
          </button>
          <div className="hmv-slider">
            <input
              type="range"
              min={8}
              max={28}
              value={hexSize}
              onChange={(e) => setHexSize(Number(e.target.value))}
              aria-label="Zoom"
            />
          </div>
        </aside>
      </div>

      {selected && (
        <footer className="hmv-footer">
          <div>
            <strong>{selected.id}</strong>
            <span>
              {selected.alias} · {fillBy} {fillMetric(selected, fillBy).toFixed(1)}% · {selected.az}
            </span>
          </div>
          <button type="button" onClick={() => setSelected(null)}>
            ×
          </button>
        </footer>
      )}
    </div>
  );
}
