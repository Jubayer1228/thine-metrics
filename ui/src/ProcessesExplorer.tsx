import { useCallback, useEffect, useMemo, useState } from "react";
import {
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
  ZAxis,
} from "recharts";
import { api, type ProcessExplorer, type ProcessRow } from "./api";
import { fmt } from "./widgets";

type FacetKey = "command" | "user" | "service" | "env" | "team" | "host";

const FACET_LABELS: Record<FacetKey, string> = {
  command: "Command",
  user: "User",
  service: "Service",
  env: "Env",
  team: "Team",
  host: "Host",
};

function ago(ms: number): string {
  if (!ms) return "—";
  const sec = Math.max(0, (Date.now() - ms) / 1000);
  if (sec < 3600) return `${Math.max(1, Math.round(sec / 60))} minutes`;
  if (sec < 86400) return `${Math.round(sec / 3600)} hours`;
  return `${Math.round(sec / 86400)} days`;
}

function fmtRss(mb: number): string {
  if (mb >= 1024) return `${fmt(mb / 1024, 1)} GB`;
  return `${fmt(mb, 0)} MB`;
}

function Bar({ pct }: { pct: number }) {
  return (
    <span className="pe-bar">
      <i style={{ width: `${Math.max(2, Math.min(100, pct))}%` }} />
    </span>
  );
}

export function ProcessesExplorer() {
  const [tab, setTab] = useState<"overview" | "distribution">("overview");
  const [summaryTab, setSummaryTab] = useState<"scatter" | "timeseries">("scatter");
  const [q, setQ] = useState("");
  const [command, setCommand] = useState("all");
  const [user, setUser] = useState("all");
  const [service, setService] = useState("all");
  const [env, setEnv] = useState("all");
  const [team, setTeam] = useState("all");
  const [host, setHost] = useState("all");
  const [groupBy, setGroupBy] = useState("");
  const [facetSearch, setFacetSearch] = useState("");
  const [logX, setLogX] = useState(true);
  const [logY, setLogY] = useState(true);
  const [data, setData] = useState<ProcessExplorer | null>(null);
  const [selected, setSelected] = useState<ProcessRow | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [liveAt] = useState(() =>
    new Date().toLocaleString(undefined, { month: "short", day: "numeric", hour: "numeric", minute: "2-digit" }),
  );

  const load = useCallback(async () => {
    try {
      const ex = await api.processesExplorer({
        host: host === "all" ? undefined : host,
        user: user === "all" ? undefined : user,
        command: command === "all" ? undefined : command,
        service: service === "all" ? undefined : service,
        env: env === "all" ? undefined : env,
        q: q.trim() || undefined,
        limit: 200,
      });
      setData(ex);
      setError(null);
    } catch (e) {
      setError(e instanceof Error ? e.message : "Processes load failed");
    }
  }, [host, user, command, service, env, q]);

  useEffect(() => {
    void load();
    const id = setInterval(() => void load(), 5_000);
    return () => clearInterval(id);
  }, [load]);

  const facet = (key: FacetKey) => data?.facets?.[key] ?? [];
  const facetVals: Record<FacetKey, string> = { command, user, service, env, team, host };
  const facetSet: Record<FacetKey, (v: string) => void> = {
    command: setCommand,
    user: setUser,
    service: setService,
    env: setEnv,
    team: setTeam,
    host: setHost,
  };

  const rows = useMemo(() => {
    let r = data?.processes ?? [];
    if (team !== "all") r = r.filter((p) => p.team === team);
    return r;
  }, [data, team]);

  const groups = useMemo(() => {
    if (groupBy === "command" || !groupBy) {
      return (data?.command_groups ?? []).map((g) => ({
        key: g.command,
        count: g.count,
        x: Math.max(0.01, g.avg_cpu_pct),
        y: Math.max(1, g.avg_mem_rss_mb),
        z: Math.max(8, Math.min(60, g.count * 6)),
      }));
    }
    const m = new Map<string, ProcessRow[]>();
    for (const p of rows) {
      const k =
        groupBy === "user"
          ? p.user
          : groupBy === "host"
            ? p.host
            : groupBy === "service"
              ? p.service || "untagged"
              : groupBy === "env"
                ? p.env
                : p.command;
      if (!m.has(k)) m.set(k, []);
      m.get(k)!.push(p);
    }
    return Array.from(m.entries()).map(([key, list]) => ({
      key,
      count: list.length,
      x: Math.max(0.01, list.reduce((s, p) => s + p.cpu_pct, 0) / list.length),
      y: Math.max(1, list.reduce((s, p) => s + p.mem_rss_mb, 0) / list.length),
      z: Math.max(8, Math.min(60, list.length * 6)),
    }));
  }, [data, groupBy, rows]);

  const timeseries = useMemo(() => {
    const top = [...rows].sort((a, b) => b.cpu_pct - a.cpu_pct).slice(0, 5);
    return Array.from({ length: 24 }, (_, i) => {
      const row: Record<string, number> = { t: i };
      top.forEach((p, pi) => {
        row[`p${pi}`] = Math.max(0, p.cpu_pct + Math.sin(i / 2 + pi) * 4);
      });
      return row;
    });
  }, [rows]);

  const maxCpu = Math.max(1, ...rows.map((p) => p.cpu_pct));
  const maxRss = Math.max(1, ...rows.map((p) => p.mem_rss_mb));
  const tip = {
    background: "var(--surf-elevated)",
    border: "1px solid var(--surf-border)",
    borderRadius: 4,
    color: "var(--surf-text)",
    fontSize: 11,
  };

  return (
    <div className="pe">
      <header className="pe-top">
        <nav className="pe-infra-nav">
          <a href="#hostmap">Host Map</a>
          <a href="#infra-list">Infrastructure List</a>
          <a href="#containers">Containers</a>
          <a className="on" href="#processes">
            Processes
          </a>
          <a href="#serverless">Serverless</a>
          <a href="#network">Network</a>
        </nav>
        <div className="pe-top-right">
          <span className="pe-live">LIVE</span>
          <span className="pe-clock">{liveAt}</span>
          <div className="pe-playback">
            <button type="button">⏮</button>
            <button type="button">⏸</button>
            <button type="button">⏭</button>
          </div>
          <button type="button" className="pe-gear" title="Settings">
            ⚙
          </button>
        </div>
      </header>

      <div className="pe-subtabs">
        <button type="button" className={tab === "overview" ? "on" : ""} onClick={() => setTab("overview")}>
          Overview
        </button>
        <button
          type="button"
          className={tab === "distribution" ? "on" : ""}
          onClick={() => setTab("distribution")}
        >
          Distribution Metrics
        </button>
      </div>

      <div className="pe-filterbar">
        <label className="pe-search">
          <span>🔍</span>
          <input placeholder="Search processes…" value={q} onChange={(e) => setQ(e.target.value)} />
        </label>
        <label>
          Filter by
          <select value={env} onChange={(e) => setEnv(e.target.value)}>
            <option value="all">Select tags</option>
            {(data?.facets?.env ?? []).map((f) => (
              <option key={f.value} value={f.value}>
                env:{f.value}
              </option>
            ))}
          </select>
        </label>
        <label>
          Group by
          <select value={groupBy} onChange={(e) => setGroupBy(e.target.value)}>
            <option value="">Select tags</option>
            <option value="command">command</option>
            <option value="user">user</option>
            <option value="host">host</option>
            <option value="service">service</option>
            <option value="env">env</option>
          </select>
        </label>
        <button type="button" className="pe-create">
          Create Metric ▾
        </button>
      </div>

      {error && (
        <div className="dd-banner">
          <strong>Processes failed</strong>
          <span>{error}</span>
        </div>
      )}

      <div className="pe-body">
        <aside className="pe-side">
          <h4>Core</h4>
          <input
            className="pe-facet-search"
            placeholder="Search facets"
            value={facetSearch}
            onChange={(e) => setFacetSearch(e.target.value)}
          />
          {(["command", "user", "service", "env", "team"] as FacetKey[])
            .filter((k) => !facetSearch || FACET_LABELS[k].toLowerCase().includes(facetSearch.toLowerCase()))
            .map((key) => (
              <div key={key} className="pe-facet">
                <div className="pe-facet-head">
                  <strong>{FACET_LABELS[key]}</strong>
                  <button type="button" onClick={() => setGroupBy(key)}>
                    Group
                  </button>
                </div>
                <div className="pe-facet-list">
                  <label>
                    <input
                      type="checkbox"
                      checked={facetVals[key] === "all"}
                      onChange={() => facetSet[key]("all")}
                    />
                    all
                  </label>
                  {facet(key)
                    .slice(0, 10)
                    .map((f) => (
                      <label key={f.value}>
                        <input
                          type="checkbox"
                          checked={facetVals[key] === f.value}
                          onChange={() =>
                            facetSet[key](facetVals[key] === f.value ? "all" : f.value)
                          }
                        />
                        <span title={f.value}>{f.value}</span>
                        <em>{f.count}</em>
                      </label>
                    ))}
                </div>
              </div>
            ))}
        </aside>

        <main className="pe-main">
          <section className="pe-summary">
            <div className="pe-summary-head">
              <h3>
                Summary Graphs{" "}
                <small>
                  Showing {groups.length} {groupBy || "command"} groups by x-axis metric (Total CPU %)
                </small>
              </h3>
              <div className="pe-tabs">
                <button
                  type="button"
                  className={summaryTab === "scatter" ? "on" : ""}
                  onClick={() => setSummaryTab("scatter")}
                >
                  Scatter Plot
                </button>
                <button
                  type="button"
                  className={summaryTab === "timeseries" ? "on" : ""}
                  onClick={() => setSummaryTab("timeseries")}
                >
                  Timeseries
                </button>
              </div>
            </div>

            {summaryTab === "scatter" ? (
              <>
                <div className="pe-axis">
                  <label>
                    X: Avg · Total CPU %
                    <button type="button" className={logX ? "on" : ""} onClick={() => setLogX((v) => !v)}>
                      Log scale
                    </button>
                  </label>
                  <label>
                    Y: Avg · RSS Memory
                    <button type="button" className={logY ? "on" : ""} onClick={() => setLogY((v) => !v)}>
                      Log scale
                    </button>
                  </label>
                </div>
                <div style={{ height: 260 }}>
                  <ResponsiveContainer width="100%" height="100%">
                    <ScatterChart margin={{ top: 8, right: 12, bottom: 8, left: 8 }}>
                      <CartesianGrid stroke="var(--surf-border)" />
                      <XAxis
                        type="number"
                        dataKey="x"
                        name="CPU"
                        unit="%"
                        scale={logX ? "log" : "auto"}
                        domain={logX ? [0.1, "auto"] : [0, "auto"]}
                        allowDataOverflow
                        tick={{ fill: "var(--surf-muted)", fontSize: 10 }}
                      />
                      <YAxis
                        type="number"
                        dataKey="y"
                        name="RSS"
                        unit=" MB"
                        scale={logY ? "log" : "auto"}
                        domain={logY ? [1, "auto"] : [0, "auto"]}
                        allowDataOverflow
                        tick={{ fill: "var(--surf-muted)", fontSize: 10 }}
                        width={48}
                      />
                      <ZAxis type="number" dataKey="z" range={[40, 400]} />
                      <Tooltip
                        contentStyle={tip}
                        cursor={{ strokeDasharray: "3 3" }}
                        formatter={(v, name) => [fmt(Number(v), 1), String(name)]}
                        labelFormatter={(_, p) => (p?.[0]?.payload as { key?: string })?.key ?? ""}
                      />
                      <Scatter data={groups}>
                        {groups.map((g) => (
                          <Cell key={g.key} fill="#5b91eb" fillOpacity={0.75} />
                        ))}
                      </Scatter>
                    </ScatterChart>
                  </ResponsiveContainer>
                </div>
              </>
            ) : (
              <div style={{ height: 220 }}>
                <ResponsiveContainer width="100%" height="100%">
                  <LineChart data={timeseries}>
                    <CartesianGrid stroke="var(--surf-border)" vertical={false} />
                    <XAxis dataKey="t" hide />
                    <YAxis tick={{ fill: "var(--surf-muted)", fontSize: 10 }} width={36} />
                    <Tooltip contentStyle={tip} />
                    {[0, 1, 2, 3, 4].map((i) => (
                      <Line
                        key={i}
                        type="monotone"
                        dataKey={`p${i}`}
                        stroke={["#5b91eb", "#2ec4b6", "#f0c94a", "#e05a4a", "#7c3aed"][i]}
                        strokeWidth={1.6}
                        dot={false}
                      />
                    ))}
                  </LineChart>
                </ResponsiveContainer>
              </div>
            )}
          </section>

          {tab === "distribution" ? (
            <section className="pe-table-wrap">
              <p className="pe-count">Distribution of CPU / RSS across command groups</p>
              <table className="pe-table">
                <thead>
                  <tr>
                    <th>Command</th>
                    <th>Processes</th>
                    <th>Avg CPU %</th>
                    <th>Avg RSS</th>
                  </tr>
                </thead>
                <tbody>
                  {groups
                    .slice()
                    .sort((a, b) => b.x - a.x)
                    .map((g) => (
                      <tr key={g.key}>
                        <td className="pe-cmd">{g.key}</td>
                        <td>{g.count}</td>
                        <td>
                          {fmt(g.x, 1)}% <Bar pct={(g.x / maxCpu) * 100} />
                        </td>
                        <td>
                          {fmtRss(g.y)} <Bar pct={(g.y / maxRss) * 100} />
                        </td>
                      </tr>
                    ))}
                </tbody>
              </table>
            </section>
          ) : (
            <section className="pe-table-wrap">
              <div className="pe-table-toolbar">
                <span>
                  Showing 1–{Math.min(50, rows.length)} of around {data?.count ?? rows.length} matching
                  processes
                </span>
              </div>
              <table className="pe-table">
                <thead>
                  <tr>
                    <th>Process</th>
                    <th>Username</th>
                    <th>Host</th>
                    <th>CPU %</th>
                    <th>RSS Memory</th>
                    <th>Started (Ago)</th>
                  </tr>
                </thead>
                <tbody>
                  {rows.slice(0, 50).map((p) => (
                    <tr
                      key={`${p.host}-${p.pid}`}
                      className={selected?.pid === p.pid && selected.host === p.host ? "on" : ""}
                      onClick={() => setSelected(p)}
                    >
                      <td className="pe-cmd" title={p.cmdline}>
                        {p.cmdline.length > 72 ? `${p.cmdline.slice(0, 70)}…` : p.cmdline}
                      </td>
                      <td>{p.user}</td>
                      <td>
                        <code>{p.host}</code>
                      </td>
                      <td className="pe-metric">
                        <span>{fmt(p.cpu_pct, 1)}</span>
                        <Bar pct={(p.cpu_pct / maxCpu) * 100} />
                      </td>
                      <td className="pe-metric">
                        <span>{fmtRss(p.mem_rss_mb)}</span>
                        <Bar pct={(p.mem_rss_mb / maxRss) * 100} />
                      </td>
                      <td>{ago(p.started_ms)}</td>
                    </tr>
                  ))}
                  {!rows.length && (
                    <tr>
                      <td colSpan={6} className="empty">
                        No processes match filters
                      </td>
                    </tr>
                  )}
                </tbody>
              </table>
            </section>
          )}
        </main>
      </div>

      {selected && (
        <footer className="pe-footer">
          <div>
            <strong>
              pid {selected.pid} · {selected.command}
            </strong>
            <span>
              {selected.host} · {selected.user} · {selected.service || "untagged"} · CPU{" "}
              {fmt(selected.cpu_pct, 1)}% · RSS {fmtRss(selected.mem_rss_mb)}
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
