import { useCallback, useEffect, useMemo, useState } from "react";
import {
  Bar,
  BarChart,
  CartesianGrid,
  ResponsiveContainer,
  Tooltip,
  XAxis,
  YAxis,
} from "recharts";
import { api, type ServerlessFunction, type ServerlessOverview } from "./api";
import { fmt } from "./widgets";

const tip = {
  background: "var(--surf-elevated)",
  border: "1px solid var(--surf-border)",
  borderRadius: 4,
  color: "var(--surf-text)",
  fontSize: 11,
};

type Cloud = "all" | "aws" | "azure" | "gcp";

export function ServerlessPanel() {
  const [data, setData] = useState<ServerlessOverview | null>(null);
  const [cloud, setCloud] = useState<Cloud>("all");
  const [env, setEnv] = useState("all");
  const [kind, setKind] = useState("all");
  const [q, setQ] = useState("");
  const [selected, setSelected] = useState<ServerlessFunction | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [liveAt] = useState(() =>
    new Date().toLocaleString(undefined, { month: "short", day: "numeric", hour: "numeric", minute: "2-digit" }),
  );

  const load = useCallback(async () => {
    try {
      const ov = await api.serverlessOverview();
      setData(ov);
      setError(null);
    } catch (e) {
      setError(e instanceof Error ? e.message : "Serverless load failed");
    }
  }, []);

  useEffect(() => {
    void load();
    const id = setInterval(() => void load(), 12_000);
    return () => clearInterval(id);
  }, [load]);

  const functions = useMemo(() => {
    let rows = data?.functions ?? [];
    if (cloud !== "all") rows = rows.filter((f) => f.cloud === cloud);
    if (env !== "all") rows = rows.filter((f) => f.env === env);
    if (kind !== "all") rows = rows.filter((f) => (f.kind || "lambda") === kind);
    if (q.trim()) {
      const s = q.toLowerCase();
      rows = rows.filter(
        (f) =>
          f.name.toLowerCase().includes(s) ||
          f.service?.toLowerCase().includes(s) ||
          f.runtime.toLowerCase().includes(s),
      );
    }
    return rows;
  }, [data, cloud, env, kind, q]);

  const invChart = useMemo(
    () =>
      functions.slice(0, 10).map((f) => ({
        name: f.name.length > 14 ? `${f.name.slice(0, 12)}…` : f.name,
        invocations: f.invocations_24h,
        errors: f.errors_24h,
        cold: f.cold_starts_24h,
      })),
    [functions],
  );

  const totals = data?.totals;
  const envs = useMemo(
    () => Array.from(new Set((data?.functions ?? []).map((f) => f.env).filter(Boolean))).sort(),
    [data],
  );
  const kinds = useMemo(
    () =>
      Array.from(new Set((data?.functions ?? []).map((f) => f.kind || "lambda"))).sort(),
    [data],
  );

  return (
    <div className="sv">
      <header className="sv-top">
        <nav className="sv-infra-nav">
          <a href="#hostmap">Host Map</a>
          <a href="#containers">Containers</a>
          <a href="#processes">Processes</a>
          <a className="on" href="#serverless">
            Serverless
          </a>
          <a href="#network">Network</a>
        </nav>
        <div className="sv-top-right">
          <span className="sv-live">LIVE</span>
          <span className="sv-clock">{liveAt}</span>
        </div>
      </header>

      <div className="sv-hero">
        <div>
          <h2>Serverless Monitoring</h2>
          <p>
            Full visibility into Lambda, Azure App Service, Cloud Run, and Step Functions — cold starts,
            cost, errors, and enhanced metrics.{" "}
            <a href="https://docs.datadoghq.com/serverless/" target="_blank" rel="noreferrer">
              Docs
            </a>
          </p>
        </div>
        <div className="sv-clouds">
          {(["all", "aws", "azure", "gcp"] as Cloud[]).map((c) => (
            <button key={c} type="button" className={cloud === c ? "on" : ""} onClick={() => setCloud(c)}>
              {c === "all" ? "All clouds" : c.toUpperCase()}
              {c !== "all" && data?.by_cloud?.[c] != null ? ` (${data.by_cloud[c]})` : ""}
            </button>
          ))}
        </div>
      </div>

      {error && (
        <div className="dd-banner">
          <strong>Serverless failed</strong>
          <span>{error}</span>
        </div>
      )}

      <div className="sv-kpis">
        <div className="sv-kpi">
          <span>Functions</span>
          <strong>{totals?.functions ?? functions.length}</strong>
        </div>
        <div className="sv-kpi">
          <span>Invocations (24h)</span>
          <strong>{fmt(totals?.invocations_24h ?? 0, 0)}</strong>
        </div>
        <div className="sv-kpi">
          <span>Errors (24h)</span>
          <strong>{fmt(totals?.errors_24h ?? 0, 0)}</strong>
          <small>{fmt(totals?.error_rate ?? 0, 2)}% rate</small>
        </div>
        <div className="sv-kpi">
          <span>Cold starts (24h)</span>
          <strong>{fmt(totals?.cold_starts_24h ?? 0, 0)}</strong>
        </div>
        <div className="sv-kpi">
          <span>Est. cost (24h)</span>
          <strong>${fmt(totals?.estimated_cost_24h ?? 0, 2)}</strong>
        </div>
      </div>

      <div className="sv-toolbar">
        <input
          placeholder="Filter function / service / runtime…"
          value={q}
          onChange={(e) => setQ(e.target.value)}
        />
        <select value={env} onChange={(e) => setEnv(e.target.value)}>
          <option value="all">All envs</option>
          {envs.map((e) => (
            <option key={e} value={e}>
              {e}
            </option>
          ))}
        </select>
        <select value={kind} onChange={(e) => setKind(e.target.value)}>
          <option value="all">All kinds</option>
          {kinds.map((k) => (
            <option key={k} value={k}>
              {k}
            </option>
          ))}
        </select>
      </div>

      <div className="sv-chart">
        <h3>Invocations · errors · cold starts</h3>
        <div style={{ height: 200 }}>
          <ResponsiveContainer width="100%" height="100%">
            <BarChart data={invChart}>
              <CartesianGrid stroke="var(--surf-border)" vertical={false} />
              <XAxis dataKey="name" tick={{ fill: "var(--surf-muted)", fontSize: 10 }} />
              <YAxis tick={{ fill: "var(--surf-muted)", fontSize: 10 }} width={40} />
              <Tooltip contentStyle={tip} />
              <Bar dataKey="invocations" fill="#5b91eb" name="invocations" />
              <Bar dataKey="errors" fill="#e05a4a" name="errors" />
              <Bar dataKey="cold" fill="#f0c94a" name="cold starts" />
            </BarChart>
          </ResponsiveContainer>
        </div>
      </div>

      <div className="sv-table-wrap">
        <table className="sv-table">
          <thead>
            <tr>
              <th>Function</th>
              <th>Cloud / Kind</th>
              <th>Runtime</th>
              <th>Invocations</th>
              <th>Errors</th>
              <th>Cold starts</th>
              <th>Duration</th>
              <th>Mem used</th>
              <th>Cost 24h</th>
            </tr>
          </thead>
          <tbody>
            {functions.map((f) => (
              <tr
                key={f.name}
                className={selected?.name === f.name ? "on" : ""}
                onClick={() => setSelected(f)}
              >
                <td>
                  <strong>{f.name}</strong>
                  <div className="sv-sub">
                    {f.service}:{f.env} · {f.region}
                  </div>
                </td>
                <td>
                  <span className="sv-pill">{f.cloud}</span>{" "}
                  <span className="sv-pill muted">{f.kind || "lambda"}</span>
                </td>
                <td>{f.runtime}</td>
                <td>{fmt(f.invocations_24h, 0)}</td>
                <td className={f.errors_24h > 10 ? "bad" : ""}>
                  {f.errors_24h}
                  {(f.timeout_errors_24h ?? 0) > 0 && (
                    <small> · {f.timeout_errors_24h} timeout</small>
                  )}
                  {(f.oom_errors_24h ?? 0) > 0 && <small> · {f.oom_errors_24h} OOM</small>}
                </td>
                <td>{f.cold_starts_24h}</td>
                <td>{fmt(f.avg_duration_ms, 0)} ms</td>
                <td>
                  {fmt(f.memory_used_pct ?? 0, 0)}%
                  <span className="sv-membar">
                    <i style={{ width: `${Math.min(100, f.memory_used_pct ?? 0)}%` }} />
                  </span>
                </td>
                <td>${fmt(f.estimated_cost_24h ?? 0, 2)}</td>
              </tr>
            ))}
            {!functions.length && (
              <tr>
                <td colSpan={9} className="empty">
                  No serverless functions match filters
                </td>
              </tr>
            )}
          </tbody>
        </table>
      </div>

      {selected && (
        <footer className="sv-footer">
          <div>
            <strong>{selected.name}</strong>
            <span>
              Enhanced metrics · cold starts {selected.cold_starts_24h} · memory {selected.memory_mb} MB ·
              est. ${fmt(selected.estimated_cost_24h ?? 0, 2)}/day
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
