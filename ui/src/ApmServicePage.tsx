import { useEffect, useState } from "react";
import {
  Bar,
  BarChart,
  CartesianGrid,
  ResponsiveContainer,
  Tooltip,
  XAxis,
  YAxis,
} from "recharts";
import { api } from "./api";
import { chartGrid, chartTip } from "./theme";
import { fmt } from "./widgets";

type Props = {
  service: string;
  onBack: () => void;
  onOpenTraces?: (service: string) => void;
};

/** Datadog APM Service Page — RED, resources, dependencies, catalog. */
export function ApmServicePage({ service, onBack, onOpenTraces }: Props) {
  const [data, setData] = useState<Record<string, unknown> | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [resource, setResource] = useState<string | null>(null);

  useEffect(() => {
    api
      .apmService(service)
      .then(setData)
      .catch((e) => setError(e instanceof Error ? e.message : "Load failed"));
  }, [service]);

  const stats = (data?.stats ?? null) as {
    request_count?: number;
    error_count?: number;
    error_rate?: number;
    p50_ms?: number;
    p95_ms?: number;
    p99_ms?: number;
    avg_ms?: number;
  } | null;
  const resources = (data?.resources ?? []) as {
    name: string;
    request_count: number;
    error_count: number;
    error_rate: number;
    p95_ms: number;
    avg_ms: number;
  }[];
  const deps = (data?.dependencies ?? []) as { from: string; to: string; protocol?: string }[];
  const catalog = data?.catalog as { name?: string; team?: string; tier?: string; links?: Record<string, string> } | null;
  const deploys = (data?.deploys ?? []) as { timestamp_ms: number; service: string; version?: string }[];
  const hosts = (data?.hosts ?? []) as {
    name: string;
    env: string;
    cpu: number;
    memory_mib: number;
    status: string;
    az?: string;
  }[];
  const profiler = data?.profiler as {
    id?: string;
    profile_type?: string;
    top_frames?: { frame: string; pct: number }[];
  } | null;

  return (
    <section className="obs-section apm-service-page">
      <div className="corr-hero">
        <div>
          <button type="button" className="ghost" onClick={onBack}>
            ← Services
          </button>
          <h2 style={{ margin: "0.5rem 0 0.25rem" }}>{service}</h2>
          <p className="muted">
            APM Service Page
            {catalog?.team ? ` · team ${catalog.team}` : ""}
            {catalog?.tier ? ` · tier ${catalog.tier}` : ""}
          </p>
        </div>
        <button type="button" onClick={() => onOpenTraces?.(service)}>
          View traces
        </button>
      </div>
      {error && <p className="error">{error}</p>}
      <div className="obs-kpi-row">
        <div className="obs-kpi">
          <span>Requests</span>
          <strong>{stats?.request_count ?? "—"}</strong>
        </div>
        <div className="obs-kpi">
          <span>Errors</span>
          <strong>{stats?.error_count ?? "—"}</strong>
        </div>
        <div className="obs-kpi">
          <span>Error rate</span>
          <strong>{stats ? `${((stats.error_rate ?? 0) * 100).toFixed(1)}%` : "—"}</strong>
        </div>
        <div className="obs-kpi">
          <span>p95</span>
          <strong>{stats ? `${fmt(stats.p95_ms, 0)}ms` : "—"}</strong>
        </div>
      </div>
      <div className="obs-chart tall">
        <ResponsiveContainer width="100%" height="100%">
          <BarChart
            data={[
              { name: "p50", v: stats?.p50_ms ?? 0 },
              { name: "avg", v: stats?.avg_ms ?? 0 },
              { name: "p95", v: stats?.p95_ms ?? 0 },
              { name: "p99", v: stats?.p99_ms ?? 0 },
            ]}
          >
            <CartesianGrid stroke={chartGrid} vertical={false} />
            <XAxis dataKey="name" tick={{ fill: "var(--surf-muted)", fontSize: 11 }} />
            <YAxis tick={{ fill: "var(--surf-muted)", fontSize: 11 }} />
            <Tooltip contentStyle={chartTip} />
            <Bar dataKey="v" fill="#4285F4" name="ms" radius={[4, 4, 0, 0]} />
          </BarChart>
        </ResponsiveContainer>
      </div>
      <div className="obs-grid-2">
        <div className="obs-card">
          <h3>Resources</h3>
          <table className="dd-table compact">
            <thead>
              <tr>
                <th>Resource</th>
                <th>Reqs</th>
                <th>Err%</th>
                <th>p95</th>
              </tr>
            </thead>
            <tbody>
              {resources.map((r) => (
                <tr
                  key={r.name}
                  className={resource === r.name ? "on" : ""}
                  style={{ cursor: "pointer" }}
                  onClick={() => setResource(r.name)}
                >
                  <td>
                    <code>{r.name}</code>
                  </td>
                  <td>{r.request_count}</td>
                  <td>{(r.error_rate * 100).toFixed(1)}%</td>
                  <td>{fmt(r.p95_ms, 0)}ms</td>
                </tr>
              ))}
              {!resources.length && (
                <tr>
                  <td colSpan={4} className="muted">
                    No resources
                  </td>
                </tr>
              )}
            </tbody>
          </table>
          {resource && (
            <p className="muted" style={{ marginTop: "0.65rem" }}>
              Resource page: <strong>{resource}</strong> — open Traces filtered to this operation.
            </p>
          )}
        </div>
        <div className="obs-card">
          <h3>Dependencies</h3>
          <ul className="corr-metric-list">
            {deps.map((d, i) => (
              <li key={i}>
                <code>
                  {d.from} → {d.to}
                </code>
                <span>{d.protocol || "rpc"}</span>
              </li>
            ))}
            {!deps.length && <li className="muted">No observed edges</li>}
          </ul>
        </div>
      </div>
      <div className="obs-grid-2" style={{ marginTop: "0.75rem" }}>
        <div className="obs-card">
          <h3>Deploys</h3>
          <ul className="corr-metric-list">
            {deploys.map((d, i) => (
              <li key={i}>
                <code>{d.version || d.service}</code>
                <span>{new Date(d.timestamp_ms).toLocaleString()}</span>
              </li>
            ))}
            {!deploys.length && <li className="muted">No recent deploys</li>}
          </ul>
        </div>
        <div className="obs-card">
          <h3>Infra hosts</h3>
          <table className="dd-table compact">
            <thead>
              <tr>
                <th>Host</th>
                <th>Env</th>
                <th>CPU</th>
                <th>Mem</th>
              </tr>
            </thead>
            <tbody>
              {hosts.map((h) => (
                <tr key={h.name}>
                  <td>
                    <code>{h.name}</code>
                  </td>
                  <td>{h.env}</td>
                  <td>{fmt(h.cpu, 0)}%</td>
                  <td>{fmt(h.memory_mib, 0)} MiB</td>
                </tr>
              ))}
              {!hosts.length && (
                <tr>
                  <td colSpan={4} className="muted">
                    No hosts tagged to this service
                  </td>
                </tr>
              )}
            </tbody>
          </table>
        </div>
      </div>
      <div className="obs-card" style={{ marginTop: "0.75rem" }}>
        <h3>Continuous Profiler {profiler?.profile_type ? `· ${profiler.profile_type}` : ""}</h3>
        {profiler?.top_frames?.length ? (
          <div className="flame-bars">
            {profiler.top_frames.map((f) => (
              <div key={f.frame} style={{ marginBottom: 6 }}>
                <div style={{ display: "flex", justifyContent: "space-between", fontSize: 12 }}>
                  <code>{f.frame}</code>
                  <span>{f.pct.toFixed(1)}%</span>
                </div>
                <div
                  style={{
                    height: 8,
                    borderRadius: 4,
                    background: "rgba(91,145,235,0.15)",
                    overflow: "hidden",
                  }}
                >
                  <div style={{ width: `${f.pct}%`, height: "100%", background: "#5B91EB" }} />
                </div>
              </div>
            ))}
          </div>
        ) : (
          <p className="muted">No profile attached — upload via /api/v1/profiler/profiles</p>
        )}
      </div>
    </section>
  );
}
