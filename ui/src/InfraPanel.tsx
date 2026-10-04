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
import {
  api,
  type InfraEnvs,
  type InfraHostmap,
  type K8sUtilization,
  type LiveProcess,
} from "./api";
import { chartTip } from "./theme";
import { ContainerMap } from "./ContainerMap";
import { ContainersExplorer } from "./ContainersExplorer";
import { HostMap } from "./HostMap";
import { fmt } from "./widgets";

const tip = chartTip;

function Kpi({ label, value, hint }: { label: string; value: string; hint?: string }) {
  return (
    <div className="obs-kpi">
      <span>{label}</span>
      <strong>{value}</strong>
      {hint && <small>{hint}</small>}
    </div>
  );
}

export type InfraView = "overview" | "hostmap" | "containers" | "container-map" | "envs" | "k8s" | "processes";

type Props = {
  procs: LiveProcess[];
  view?: InfraView;
};

export function InfraPanel({ procs, view = "overview" }: Props) {
  const [envs, setEnvs] = useState<InfraEnvs | null>(null);
  const [hostmap, setHostmap] = useState<InfraHostmap | null>(null);
  const [util, setUtil] = useState<K8sUtilization | null>(null);
  const [stats, setStats] = useState<Record<string, unknown> | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [envFilter, setEnvFilter] = useState<string | undefined>();
  const [procHost, setProcHost] = useState("all");
  const [liveProcs, setLiveProcs] = useState<LiveProcess[]>(procs);

  const load = useCallback(async () => {
    try {
      const [hm, ev, ut, st] = await Promise.all([
        api.infraHostmap("availability-zone"),
        api.infraEnvs(),
        api.k8sUtilization(),
        api.containerStats(),
      ]);
      setHostmap(hm);
      setEnvs(ev);
      setUtil(ut);
      setStats(st);
      setError(null);
    } catch (e) {
      setError(e instanceof Error ? e.message : "Infra load failed");
    }
  }, []);

  useEffect(() => {
    void load();
    const id = setInterval(() => void load(), 12_000);
    return () => clearInterval(id);
  }, [load]);

  useEffect(() => {
    setLiveProcs(procs);
  }, [procs]);

  useEffect(() => {
    if (view !== "processes" && view !== "overview") return;
    let cancelled = false;
    async function pull() {
      try {
        const rows = await api.liveProcesses(
          procHost === "all" ? undefined : procHost,
          40,
        );
        if (!cancelled && rows?.length) setLiveProcs(rows);
      } catch {
        /* keep parent procs */
      }
    }
    void pull();
    const id = setInterval(() => void pull(), 8_000);
    return () => {
      cancelled = true;
      clearInterval(id);
    };
  }, [view, procHost]);

  const envChart = useMemo(
    () =>
      (envs?.envs ?? []).map((e) => ({
        env: e.env,
        hosts: e.hosts,
        containers: e.containers,
        cpu: e.avg_host_cpu_pct,
      })),
    [envs],
  );

  const procHosts = useMemo(() => {
    const s = new Set(liveProcs.map((p) => p.host));
    return Array.from(s).sort();
  }, [liveProcs]);

  const show = (section: InfraView | "overview") =>
    view === "overview" || view === section;

  return (
    <div className="obs-section">
      {view === "overview" && (
        <div className="obs-sig">
          <strong>
            {String(stats?.running ?? "—")} running containers · {hostmap?.hosts?.length ?? 0} hosts ·{" "}
            {envs?.envs?.length ?? 0} envs
          </strong>
          <span>
            Infrastructure overview — Host Map, Containers Explorer, environments, and Kubernetes
            utilization (Datadog parity surfaces).
          </span>
        </div>
      )}

      {error && (
        <div className="dd-banner">
          <strong>Infra load failed</strong>
          <span>{error}</span>
          <button type="button" onClick={() => void load()}>
            Retry
          </button>
        </div>
      )}

      {view === "overview" && (
        <div className="obs-kpi-row">
          <Kpi label="Hosts" value={String(hostmap?.hosts?.length ?? 0)} />
          <Kpi
            label="Containers"
            value={String(stats?.total ?? 0)}
            hint={`${stats?.running ?? "—"} running`}
          />
          <Kpi label="Envs" value={String(envs?.envs?.length ?? 0)} hint="unified tagging" />
          <Kpi label="K8s pods" value={String(stats?.kubernetes ?? 0)} />
          <Kpi label="Avg CPU" value={`${fmt(Number(stats?.avg_cpu_pct ?? 0), 1)}%`} />
          <Kpi label="Mem vs limit" value={`${fmt(Number(stats?.avg_mem_util_pct ?? 0), 0)}%`} />
          <Kpi label="Processes" value={String(liveProcs.length)} />
        </div>
      )}

      {show("envs") && (
        <div className="obs-card">
          <div className="infra-toolbar">
            <h3 style={{ margin: 0 }}>Environments</h3>
            <span className="muted">
              Unified tags {envs?.unified_tags?.join(" / ")} — click an env to open Containers Explorer
            </span>
          </div>
          <div className="env-pills">
            <button
              type="button"
              className={!envFilter ? "on" : ""}
              onClick={() => setEnvFilter(undefined)}
            >
              all
            </button>
            {(envs?.envs ?? []).map((e) => (
              <button
                key={e.env}
                type="button"
                className={envFilter === e.env ? "on" : ""}
                onClick={() => setEnvFilter(e.env)}
                title={`${e.hosts} hosts · ${e.containers} containers · CPU ${fmt(e.avg_host_cpu_pct, 0)}%`}
              >
                {e.env}
                <em>
                  {e.containers} ctr · {fmt(e.avg_host_cpu_pct, 0)}%
                </em>
              </button>
            ))}
          </div>
          <div className="obs-chart" style={{ height: 160, marginTop: "0.75rem" }}>
            <ResponsiveContainer width="100%" height="100%">
              <BarChart data={envChart}>
                <CartesianGrid stroke="rgba(255,255,255,0.06)" vertical={false} />
                <XAxis dataKey="env" tick={{ fill: "#8B97A8", fontSize: 11 }} />
                <YAxis tick={{ fill: "#8B97A8", fontSize: 11 }} width={36} />
                <Tooltip contentStyle={tip} />
                <Bar dataKey="containers" fill="#5B91EB" name="containers" />
                <Bar dataKey="hosts" fill="#2EC4B6" name="hosts" />
              </BarChart>
            </ResponsiveContainer>
          </div>
          {view === "envs" && envFilter && (
            <div style={{ marginTop: "1rem" }}>
              <ContainersExplorer initialEnv={envFilter} />
            </div>
          )}
        </div>
      )}

      {show("hostmap") && (
        <div className={`hostmap-card${view === "hostmap" ? " hostmap-full" : ""}`}>
          <HostMap />
        </div>
      )}

      {view === "container-map" && <ContainerMap />}

      {show("containers") && view !== "envs" && view !== "container-map" && (
        <div className="containers-card">
          <ContainersExplorer initialEnv={view === "overview" ? envFilter : undefined} />
        </div>
      )}

      {show("k8s") && (
        <div className="obs-card">
          <h3>Kubernetes resource utilization</h3>
          <p className="muted">{util?.significance}</p>
          <table className="dd-table compact">
            <thead>
              <tr>
                <th>Namespace / Deploy</th>
                <th>Pods</th>
                <th>CPU u/r</th>
                <th>CPU u/l</th>
                <th>Mem u/l</th>
                <th>Hint</th>
              </tr>
            </thead>
            <tbody>
              {(util?.rows ?? []).map((r) => (
                <tr key={`${r.kube_namespace}-${r.kube_deployment}`}>
                  <td>
                    <code>{r.kube_namespace}</code>
                    <div className="tags">{r.kube_deployment}</div>
                  </td>
                  <td>{r.pods}</td>
                  <td>{fmt(r.cpu_usage_vs_request_pct, 0)}%</td>
                  <td>{fmt(r.cpu_usage_vs_limit_pct, 0)}%</td>
                  <td>{fmt(r.memory_usage_vs_limit_pct, 0)}%</td>
                  <td className="tags">{r.waste_hint}</td>
                </tr>
              ))}
              {!util?.rows?.length && (
                <tr>
                  <td colSpan={6} className="tags">
                    No Kubernetes workloads
                  </td>
                </tr>
              )}
            </tbody>
          </table>
        </div>
      )}

      {show("processes") && (
        <div className="obs-card">
          <div className="infra-toolbar">
            <h3 style={{ margin: 0 }}>Live processes</h3>
            <label className="infra-select">
              Host
              <select value={procHost} onChange={(e) => setProcHost(e.target.value)}>
                <option value="all">all</option>
                {procHosts.map((h) => (
                  <option key={h} value={h}>
                    {h}
                  </option>
                ))}
              </select>
            </label>
          </div>
          <p className="muted">htop-style process view — CPU and RSS by host</p>
          <table className="dd-table compact">
            <thead>
              <tr>
                <th>PID</th>
                <th>Host</th>
                <th>User</th>
                <th>Command</th>
                <th>CPU%</th>
                <th>RSS</th>
              </tr>
            </thead>
            <tbody>
              {liveProcs
                .filter((p) => procHost === "all" || p.host === procHost)
                .slice(0, 40)
                .map((p) => (
                  <tr key={`${p.host}-${p.pid}`}>
                    <td>{p.pid}</td>
                    <td>
                      <code>{p.host}</code>
                    </td>
                    <td>{p.user}</td>
                    <td className="tags">{p.cmdline}</td>
                    <td>{fmt(p.cpu_pct, 1)}</td>
                    <td>{fmt(p.mem_rss_mb, 0)} MB</td>
                  </tr>
                ))}
              {!liveProcs.length && (
                <tr>
                  <td colSpan={6} className="tags">
                    No processes
                  </td>
                </tr>
              )}
            </tbody>
          </table>
        </div>
      )}
    </div>
  );
}
