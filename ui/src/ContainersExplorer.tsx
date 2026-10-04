import { useCallback, useEffect, useMemo, useState } from "react";
import {
  CartesianGrid,
  Cell,
  Legend,
  Line,
  LineChart,
  ResponsiveContainer,
  Scatter,
  ScatterChart,
  Tooltip,
  XAxis,
  YAxis,
  ZAxis,
} from "recharts";
import {
  api,
  type ContainerExplorer,
  type ContainerExplorerRow,
  type LogEvent,
} from "./api";
import { fmt } from "./widgets";

type FacetKey = "env" | "kube_namespace" | "host" | "service" | "image" | "status";
type ResourceTab = "processes" | "containers" | "images" | "pods";
type Pivot = "none" | "service" | "host" | "kube_namespace";

const FACET_LABELS: Record<FacetKey, string> = {
  env: "Environment",
  kube_namespace: "Namespace",
  host: "Cluster Name / Host",
  service: "Service",
  image: "Image",
  status: "Status",
};

const WARM = ["#f0c94a", "#e89050", "#e05a4a", "#c44a62", "#d4a017", "#ff8c42"];
const COOL = ["#5b91eb", "#2ec4b6", "#7c3aed", "#3d8bcd", "#52be80", "#6b8cae"];

function matchQuery(blob: string, query: string): boolean {
  const q = query.trim();
  if (!q) return true;
  if (/\s+OR\s+/i.test(q) && !q.includes("(")) {
    return q.split(/\s+OR\s+/i).some((p) => matchQuery(blob, p.trim()));
  }
  const tokens: string[] = [];
  const re = /(?:NOT\s+|!)?(?:"([^"]+)"|(\S+))/gi;
  let m: RegExpExecArray | null;
  while ((m = re.exec(q))) {
    const raw = (m[1] ?? m[2] ?? "").trim();
    if (!raw || /^AND$/i.test(raw) || /^OR$/i.test(raw)) continue;
    const full = m[0].trim();
    if (/^NOT\s+/i.test(full) || full.startsWith("!")) tokens.push(`NOT:${raw.replace(/^!/, "").toLowerCase()}`);
    else tokens.push(raw.toLowerCase());
  }
  const lower = blob.toLowerCase();
  return tokens.every((t) => {
    if (t.startsWith("NOT:")) return !lower.includes(t.slice(4));
    return lower.includes(t);
  });
}

function blob(c: ContainerExplorerRow): string {
  return [c.id, c.name, c.image, c.service, c.host, c.env, c.version, c.status, c.pod_name, c.kube_namespace, c.kube_deployment]
    .filter(Boolean)
    .join(" ");
}

function ago(ms: number): string {
  if (!ms) return "—";
  const sec = Math.max(0, (Date.now() - ms) / 1000);
  if (sec < 3600) return `${Math.max(1, Math.round(sec / 60))} minutes`;
  if (sec < 86400) return `${Math.round(sec / 3600)} hours`;
  if (sec < 86400 * 45) return `${Math.round(sec / 86400)} days`;
  return `${Math.round(sec / (86400 * 30))} months`;
}

function fmtBytes(bps: number): string {
  if (bps < 1024) return `${fmt(bps, 0)} B`;
  if (bps < 1024 * 1024) return `${fmt(bps / 1024, 1)} KiB`;
  return `${fmt(bps / (1024 * 1024), 2)} MiB`;
}

function fmtRss(mb: number): string {
  if (mb >= 1024) return `${fmt(mb / 1024, 1)} GiB`;
  return `${fmt(mb, 0)} MiB`;
}

function Bar({ pct, color = "#5b91eb" }: { pct: number; color?: string }) {
  const w = Math.max(2, Math.min(100, pct));
  return (
    <span className="ce-bar">
      <i style={{ width: `${w}%`, background: color }} />
    </span>
  );
}

function KubeIcon() {
  return (
    <svg className="ce-k8s" width="14" height="14" viewBox="0 0 24 24" aria-hidden>
      <circle cx="12" cy="12" r="3" fill="#326ce5" />
      <path
        d="M12 2l2.2 3.8L18.5 7l-1.2 4.2L19 15.5l-4.2 1.2L12 22l-2.8-5.3L5 15.5l1.7-4.3L5.5 7l4.3-1.2L12 2z"
        fill="none"
        stroke="#326ce5"
        strokeWidth="1.4"
      />
    </svg>
  );
}

type Props = { initialEnv?: string };

export function ContainersExplorer({ initialEnv }: Props) {
  const [resource, setResource] = useState<ResourceTab>("containers");
  const [env, setEnv] = useState(initialEnv ?? "all");
  const [namespace, setNamespace] = useState("all");
  const [host, setHost] = useState("all");
  const [service, setService] = useState("all");
  const [image, setImage] = useState("all");
  const [status, setStatus] = useState("all");
  const [q, setQ] = useState("");
  const [groupBy, setGroupBy] = useState("");
  const [pivot, setPivot] = useState<Pivot>("none");
  const [summaryTab, setSummaryTab] = useState<"timeseries" | "scatter">("timeseries");
  const [selected, setSelected] = useState<ContainerExplorerRow | null>(null);
  const [explorer, setExplorer] = useState<ContainerExplorer | null>(null);
  const [logs, setLogs] = useState<LogEvent[]>([]);
  const [logPaused, setLogPaused] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [facetSearch, setFacetSearch] = useState("");
  const [myTeams, setMyTeams] = useState(false);
  const [liveAt] = useState(() =>
    new Date().toLocaleString(undefined, { month: "short", day: "numeric", hour: "numeric", minute: "2-digit" }),
  );

  useEffect(() => {
    if (initialEnv) setEnv(initialEnv);
  }, [initialEnv]);

  const load = useCallback(async () => {
    try {
      const ex = await api.containersExplorer({
        env: env === "all" ? undefined : env,
        namespace: namespace === "all" ? undefined : namespace,
        host: host === "all" ? undefined : host,
        service: service === "all" ? undefined : service,
      });
      setExplorer(ex);
      setError(null);
    } catch (e) {
      setError(e instanceof Error ? e.message : "Containers explorer failed");
    }
  }, [env, namespace, host, service]);

  useEffect(() => {
    void load();
    const id = setInterval(() => void load(), 5_000);
    return () => clearInterval(id);
  }, [load]);

  const facet = (key: FacetKey) =>
    (explorer?.facets?.[key] as { value: string; count: number }[] | undefined) ?? [];

  const facetVals: Record<FacetKey, string> = {
    env,
    kube_namespace: namespace,
    host,
    service,
    image,
    status,
  };
  const facetSet: Record<FacetKey, (v: string) => void> = {
    env: setEnv,
    kube_namespace: setNamespace,
    host: setHost,
    service: setService,
    image: setImage,
    status: setStatus,
  };

  const filtered = useMemo(() => {
    let rows = explorer?.containers ?? [];
    if (resource === "pods") rows = rows.filter((c) => c.kube_namespace);
    if (resource === "images") {
      /* show all; table pivots to images */
    }
    if (image !== "all") rows = rows.filter((c) => c.image === image);
    if (status !== "all") rows = rows.filter((c) => c.status === status);
    if (myTeams) rows = rows.filter((c) => c.env === "prod");
    if (q.trim()) rows = rows.filter((c) => matchQuery(blob(c), q));
    return rows;
  }, [explorer, image, status, q, resource, myTeams]);

  useEffect(() => {
    if (!groupBy) {
      setPivot("none");
      return;
    }
    if (groupBy === "service" || groupBy === "host" || groupBy === "kube_namespace") {
      setPivot(groupBy);
    }
  }, [groupBy]);

  const seriesKeys = useMemo(() => {
    const bySvc = new Map<string, ContainerExplorerRow[]>();
    for (const c of filtered) {
      const k = c.service || c.name;
      if (!bySvc.has(k)) bySvc.set(k, []);
      bySvc.get(k)!.push(c);
    }
    return Array.from(bySvc.entries())
      .sort((a, b) => b[1].reduce((s, r) => s + r.cpu_pct, 0) - a[1].reduce((s, r) => s + r.cpu_pct, 0))
      .slice(0, 6)
      .map(([k]) => k);
  }, [filtered]);

  const timeseries = useMemo(() => {
    const pts = 24;
    return Array.from({ length: pts }, (_, i) => {
      const row: Record<string, number> = { t: i };
      seriesKeys.forEach((k, ki) => {
        const base =
          filtered.filter((c) => (c.service || c.name) === k).reduce((s, c) => s + c.cpu_pct, 0) /
          Math.max(1, filtered.filter((c) => (c.service || c.name) === k).length);
        row[`cpu_${k}`] = Math.max(0, base + Math.sin(i / 2.5 + ki) * 8 + Math.cos(i / 4 + ki) * 4);
        const mem =
          filtered.filter((c) => (c.service || c.name) === k).reduce((s, c) => s + c.mem_rss_mb, 0) / 1024;
        row[`mem_${k}`] = Math.max(0, mem + Math.sin(i / 3 + ki) * 0.4);
      });
      return row;
    });
  }, [filtered, seriesKeys]);

  const scatterData = useMemo(
    () =>
      filtered.map((c) => ({
        x: c.cpu_pct,
        y: c.mem_rss_mb / 1024,
        z: Math.max(6, Math.min(40, c.mem_rss_mb / 30)),
        name: c.name,
        id: c.id,
      })),
    [filtered],
  );

  const pivoted = useMemo(() => {
    if (pivot === "none") return null;
    const m = new Map<string, ContainerExplorerRow[]>();
    for (const c of filtered) {
      const key =
        pivot === "service" ? c.service || "untagged" : pivot === "host" ? c.host : c.kube_namespace || "docker";
      if (!m.has(key)) m.set(key, []);
      m.get(key)!.push(c);
    }
    return Array.from(m.entries())
      .map(([key, rows]) => ({
        key,
        count: rows.length,
        cpu: rows.reduce((s, r) => s + r.cpu_pct, 0) / rows.length,
        rss: rows.reduce((s, r) => s + r.mem_rss_mb, 0),
      }))
      .sort((a, b) => b.cpu - a.cpu);
  }, [filtered, pivot]);

  useEffect(() => {
    if (!selected || logPaused) return;
    let cancelled = false;
    async function pull() {
      try {
        const res = await api.logsSearch({ limit: 60, live: true });
        if (cancelled) return;
        const all = res.logs;
        const svc = selected!.service;
        const name = selected!.name.toLowerCase();
        const matched = all.filter(
          (l) => l.service === svc || l.message.toLowerCase().includes(name) || l.message.includes(selected!.id.slice(0, 8)),
        );
        setLogs(matched.length ? matched : all.slice(0, 16));
      } catch {
        /* ignore */
      }
    }
    void pull();
    const id = setInterval(() => void pull(), 2500);
    return () => {
      cancelled = true;
      clearInterval(id);
    };
  }, [selected, logPaused]);

  const maxCpu = Math.max(1, ...filtered.map((c) => c.cpu_pct));
  const maxRss = Math.max(1, ...filtered.map((c) => c.mem_rss_mb));

  function groupFacet(key: FacetKey) {
    if (key === "service" || key === "host" || key === "kube_namespace") {
      setGroupBy(key);
      setPivot(key);
    }
  }

  const tip = {
    background: "var(--surf-elevated)",
    border: "1px solid var(--surf-border)",
    borderRadius: 4,
    color: "var(--surf-text)",
    fontSize: 11,
  };

  return (
    <div className="ce">
      <header className="ce-top">
        <div className="ce-top-left">
          <button type="button" className="ce-views">
            ▦ Views
          </button>
          <button type="button" className="ce-title">
            Containers <span>▾</span>
          </button>
          <button type="button" className="ce-save">
            + Save
          </button>
        </div>
        <div className="ce-top-right">
          <span className="ce-live">LIVE</span>
          <span className="ce-clock">{liveAt}</span>
          <div className="ce-playback">
            <button type="button" title="Rewind">
              ⏮
            </button>
            <button type="button" title="Pause">
              ⏸
            </button>
            <button type="button" title="Forward">
              ⏭
            </button>
          </div>
        </div>
      </header>

      <div className="ce-filterbar">
        <label className="ce-filter">
          <span>Filter by</span>
          <input
            placeholder="Filter your containers"
            value={q}
            onChange={(e) => setQ(e.target.value)}
          />
        </label>
        <span className="ce-code">&lt;/&gt;</span>
        <label className="ce-groupby">
          <span>Group by</span>
          <select
            value={groupBy}
            onChange={(e) => setGroupBy(e.target.value)}
          >
            <option value="">Select facets to group by</option>
            <option value="service">service</option>
            <option value="host">host</option>
            <option value="kube_namespace">kube_namespace</option>
          </select>
        </label>
      </div>

      {error && (
        <div className="dd-banner">
          <strong>Explorer failed</strong>
          <span>{error}</span>
        </div>
      )}

      <div className="ce-body">
        <aside className="ce-side">
          <div className="ce-teams">
            <span>My Teams</span>
            <button
              type="button"
              className={`ce-toggle${myTeams ? " on" : ""}`}
              onClick={() => setMyTeams((v) => !v)}
              aria-pressed={myTeams}
            />
          </div>

          <div className="ce-resources">
            <h4>Select Resources</h4>
            {(
              [
                ["processes", "Processes"],
                ["containers", "Containers"],
                ["images", "Container Images"],
                ["pods", "Kubernetes Pods"],
              ] as const
            ).map(([id, label]) => (
              <button
                key={id}
                type="button"
                className={resource === id ? "on" : ""}
                onClick={() => setResource(id)}
              >
                {label}
                {id === "images" && <em className="ce-new">NEW</em>}
              </button>
            ))}
          </div>

          <div className="ce-facets">
            <h4>Facets</h4>
            <input
              className="ce-facet-search"
              placeholder="Search facets"
              value={facetSearch}
              onChange={(e) => setFacetSearch(e.target.value)}
            />
            {(["host", "kube_namespace", "env", "service", "image", "status"] as FacetKey[])
              .filter((k) => !facetSearch || FACET_LABELS[k].toLowerCase().includes(facetSearch.toLowerCase()))
              .map((key) => (
                <div key={key} className="ce-facet">
                  <div className="ce-facet-head">
                    <strong>{FACET_LABELS[key]}</strong>
                    <button type="button" className="ce-group-btn" onClick={() => groupFacet(key)}>
                      Group
                    </button>
                  </div>
                  <div className="ce-facet-list">
                    <label className="ce-check">
                      <input
                        type="checkbox"
                        checked={facetVals[key] === "all"}
                        onChange={() => facetSet[key]("all")}
                      />
                      <span>all</span>
                    </label>
                    {facet(key)
                      .slice(0, 8)
                      .map((f) => (
                        <label key={f.value} className="ce-check">
                          <input
                            type="checkbox"
                            checked={facetVals[key] === f.value}
                            onChange={() =>
                              facetSet[key](facetVals[key] === f.value ? "all" : f.value)
                            }
                          />
                          <span className="ce-facet-val" title={f.value}>
                            {f.value.split("/").pop()}
                          </span>
                          <em>{f.count}</em>
                        </label>
                      ))}
                  </div>
                </div>
              ))}
          </div>
        </aside>

        <main className="ce-main">
          <section className="ce-summary">
            <div className="ce-summary-head">
              <h3>Summary Graphs</h3>
              <div className="ce-tabs">
                <button
                  type="button"
                  className={summaryTab === "timeseries" ? "on" : ""}
                  onClick={() => setSummaryTab("timeseries")}
                >
                  Timeseries
                </button>
                <button
                  type="button"
                  className={summaryTab === "scatter" ? "on" : ""}
                  onClick={() => setSummaryTab("scatter")}
                >
                  Scatter Plot
                </button>
              </div>
            </div>

            {summaryTab === "timeseries" ? (
              <div className="ce-charts">
                <div className="ce-chart">
                  <header>
                    <strong>Total CPU %</strong>
                    <span>Percent</span>
                  </header>
                  <div style={{ height: 160 }}>
                    <ResponsiveContainer width="100%" height="100%">
                      <LineChart data={timeseries}>
                        <CartesianGrid stroke="var(--surf-border)" vertical={false} />
                        <XAxis dataKey="t" hide />
                        <YAxis tick={{ fill: "var(--surf-muted)", fontSize: 10 }} width={32} domain={[0, "auto"]} />
                        <Tooltip contentStyle={tip} />
                        {seriesKeys.map((k, i) => (
                          <Line
                            key={k}
                            type="monotone"
                            dataKey={`cpu_${k}`}
                            name={k}
                            stroke={WARM[i % WARM.length]}
                            strokeWidth={1.6}
                            dot={false}
                          />
                        ))}
                        <Legend wrapperStyle={{ fontSize: 11 }} />
                      </LineChart>
                    </ResponsiveContainer>
                  </div>
                </div>
                <div className="ce-chart">
                  <header>
                    <strong>RSS Memory</strong>
                    <span>Gibibytes</span>
                  </header>
                  <div style={{ height: 160 }}>
                    <ResponsiveContainer width="100%" height="100%">
                      <LineChart data={timeseries}>
                        <CartesianGrid stroke="var(--surf-border)" vertical={false} />
                        <XAxis dataKey="t" hide />
                        <YAxis tick={{ fill: "var(--surf-muted)", fontSize: 10 }} width={32} domain={[0, "auto"]} />
                        <Tooltip contentStyle={tip} />
                        {seriesKeys.map((k, i) => (
                          <Line
                            key={k}
                            type="monotone"
                            dataKey={`mem_${k}`}
                            name={k}
                            stroke={COOL[i % COOL.length]}
                            strokeWidth={1.6}
                            dot={false}
                          />
                        ))}
                        <Legend wrapperStyle={{ fontSize: 11 }} />
                      </LineChart>
                    </ResponsiveContainer>
                  </div>
                </div>
              </div>
            ) : (
              <div className="ce-chart single">
                <header>
                  <strong>CPU % vs RSS (GiB)</strong>
                </header>
                <div style={{ height: 200 }}>
                  <ResponsiveContainer width="100%" height="100%">
                    <ScatterChart>
                      <CartesianGrid stroke="var(--surf-border)" />
                      <XAxis type="number" dataKey="x" name="CPU" unit="%" tick={{ fill: "var(--surf-muted)", fontSize: 10 }} />
                      <YAxis type="number" dataKey="y" name="RSS" unit=" GiB" tick={{ fill: "var(--surf-muted)", fontSize: 10 }} width={40} />
                      <ZAxis type="number" dataKey="z" range={[40, 200]} />
                      <Tooltip contentStyle={tip} cursor={{ strokeDasharray: "3 3" }} />
                      <Scatter data={scatterData}>
                        {scatterData.map((d) => (
                          <Cell key={d.id} fill="#5b91eb" />
                        ))}
                      </Scatter>
                    </ScatterChart>
                  </ResponsiveContainer>
                </div>
              </div>
            )}
          </section>

          <section className="ce-table-wrap">
            <div className="ce-table-toolbar">
              <button type="button" className="linkish">
                Hide Controls
              </button>
              <span>
                Showing 1–{Math.min(50, filtered.length)} of {filtered.length} matching{" "}
                {resource === "pods" ? "pods" : resource === "images" ? "images" : "containers"}
              </span>
              <div className="ce-table-actions">
                <button type="button">Display Options ▾</button>
                <button type="button" title="Customize">
                  ⚙
                </button>
              </div>
            </div>

            {pivot !== "none" && pivoted ? (
              <table className="ce-table">
                <thead>
                  <tr>
                    <th>{pivot}</th>
                    <th>Containers</th>
                    <th>Avg CPU %</th>
                    <th>RSS Memory</th>
                  </tr>
                </thead>
                <tbody>
                  {pivoted.map((g) => (
                    <tr
                      key={g.key}
                      onClick={() => {
                        if (pivot === "service") setService(g.key);
                        if (pivot === "host") setHost(g.key);
                        if (pivot === "kube_namespace") setNamespace(g.key);
                        setGroupBy("");
                        setPivot("none");
                      }}
                    >
                      <td>
                        <code>{g.key}</code>
                      </td>
                      <td>{g.count}</td>
                      <td>
                        {fmt(g.cpu, 1)}% <Bar pct={(g.cpu / maxCpu) * 100} />
                      </td>
                      <td>
                        {fmtRss(g.rss)} <Bar pct={(g.rss / (maxRss * pivoted.length)) * 100} color="#2ec4b6" />
                      </td>
                    </tr>
                  ))}
                </tbody>
              </table>
            ) : (
              <table className="ce-table">
                <thead>
                  <tr>
                    <th>Container</th>
                    <th>Status</th>
                    <th>Image</th>
                    <th>Total CPU %</th>
                    <th>RSS Memory</th>
                    <th>Bytes Sent</th>
                    <th>Bytes Received</th>
                    <th>Started (Ago)</th>
                  </tr>
                </thead>
                <tbody>
                  {filtered.slice(0, 50).map((c) => (
                    <tr
                      key={c.id}
                      className={selected?.id === c.id ? "on" : ""}
                      onClick={() => {
                        setSelected(c);
                        setLogPaused(false);
                      }}
                    >
                      <td className="ce-name">
                        {c.kube_namespace ? <KubeIcon /> : <span className="ce-docker">⬡</span>}
                        <span>{c.name || c.id.slice(0, 12)}</span>
                      </td>
                      <td>
                        <span className={`ce-status ${c.status === "running" ? "up" : "down"}`}>
                          {c.status === "running" ? "UP" : c.status.toUpperCase()}
                        </span>
                      </td>
                      <td>
                        <span className="ce-image">{c.image}</span>
                      </td>
                      <td className="ce-metric">
                        <span>{fmt(c.cpu_pct, 1)}%</span>
                        <Bar pct={(c.cpu_pct / maxCpu) * 100} />
                      </td>
                      <td className="ce-metric">
                        <span>{fmtRss(c.mem_rss_mb)}</span>
                        <Bar pct={(c.mem_rss_mb / maxRss) * 100} color="#2ec4b6" />
                      </td>
                      <td>{fmtBytes(c.net_tx_bps)}</td>
                      <td>{fmtBytes(c.net_rx_bps)}</td>
                      <td>{ago(c.started_ms)}</td>
                    </tr>
                  ))}
                  {!filtered.length && (
                    <tr>
                      <td colSpan={8} className="empty">
                        No containers match filters
                      </td>
                    </tr>
                  )}
                </tbody>
              </table>
            )}
          </section>
        </main>
      </div>

      {selected && (
        <div className="ce-drawer">
          <div className="ce-drawer-head">
            <div>
              {selected.kube_namespace ? <KubeIcon /> : null}
              <strong>{selected.name}</strong>
              <code>{selected.id}</code>
            </div>
            <button type="button" onClick={() => setSelected(null)}>
              ×
            </button>
          </div>
          <p className="muted">
            env:{selected.env} service:{selected.service} version:{selected.version}
            {selected.pod_name ? ` · pod:${selected.pod_name}` : ""}
          </p>
          <div className="ce-logs-head">
            <h4>Logs</h4>
            <button type="button" onClick={() => setLogPaused((p) => !p)}>
              {logPaused ? "Resume" : "Pause"}
            </button>
          </div>
          <div className="ce-logs">
            {logs.map((l, i) => (
              <div key={`${l.timestamp_ms}-${i}`} className="ce-log">
                <time>{new Date(l.timestamp_ms).toLocaleTimeString()}</time>
                <span className="lvl">{l.level}</span>
                <span>{l.message}</span>
              </div>
            ))}
            {!logs.length && <p className="muted">Waiting for live tail…</p>}
          </div>
        </div>
      )}
    </div>
  );
}
