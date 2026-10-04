import { useCallback, useEffect, useMemo, useState } from "react";
import { api, type FleetAgent, type FleetSummary } from "./api";

type Tab = "view" | "install" | "configure" | "upgrade";
type PlatformId =
  | "kubernetes"
  | "docker"
  | "ecs"
  | "cloudrun"
  | "linux"
  | "windows"
  | "macos"
  | "aix";

const PLATFORMS: { id: PlatformId; label: string; group: string; blurb: string }[] = [
  { id: "kubernetes", label: "Kubernetes", group: "Container platforms", blurb: "Helm / Operator" },
  { id: "docker", label: "Docker", group: "Container platforms", blurb: "One-line container" },
  { id: "ecs", label: "Amazon ECS", group: "Container platforms", blurb: "Daemon / sidecar" },
  { id: "cloudrun", label: "Google Cloud Run", group: "Container platforms", blurb: "Sidecar agent" },
  { id: "linux", label: "Linux", group: "Host based", blurb: "apt / yum one-liner" },
  { id: "windows", label: "Windows", group: "Host based", blurb: "MSI / PowerShell" },
  { id: "macos", label: "macOS", group: "Host based", blurb: "pkg install" },
  { id: "aix", label: "AIX", group: "Host based", blurb: "rpm package" },
];

function siteBase() {
  if (typeof window !== "undefined" && window.location?.origin) return window.location.origin;
  return "http://127.0.0.1:4318";
}

function installScript(platform: PlatformId, apiKey: string, site: string): string {
  const base = site.replace(/\/$/, "");
  const key = apiKey || "thine_demo_key";
  switch (platform) {
    case "linux":
    case "macos":
    case "aix":
      return `DD_API_KEY=${key} DD_SITE="${base}" THINE_PLATFORM=${platform} \\
  bash -c "$(curl -fsSL ${base}/install/install_agent.sh)"`;
    case "docker":
      return `# Bootstrap via control plane (recommended for local demo)
curl -fsSL -X POST ${base}/api/v1/fleet/bootstrap \\
  -H "content-type: application/json" \\
  -H "DD-API-KEY: ${key}" \\
  -d '{"platform":"docker","host":"docker-host-1"}'`;
    case "kubernetes":
      return `# Bootstrap a k8s agent against this control plane
curl -fsSL -X POST ${base}/api/v1/fleet/bootstrap \\
  -H "content-type: application/json" \\
  -H "DD-API-KEY: ${key}" \\
  -d '{"platform":"kubernetes","host":"k8s-node-1"}'`;
    case "windows":
      return `powershell -c "iwr ${base}/install/Install-Agent.ps1 -OutFile install.ps1; .\\install.ps1 -ApiKey ${key} -Site ${base}"`;
    case "ecs":
    case "cloudrun":
      return `curl -fsSL -X POST ${base}/api/v1/fleet/bootstrap \\
  -H "content-type: application/json" \\
  -H "DD-API-KEY: ${key}" \\
  -d '{"platform":"${platform}","host":"${platform}-task-1"}'`;
  }
}

type Props = {
  initialTab?: Tab;
};

export function FleetPage({ initialTab = "install" }: Props) {
  const [tab, setTab] = useState<Tab>(initialTab);
  const [agents, setAgents] = useState<FleetAgent[]>([]);
  const [summary, setSummary] = useState<FleetSummary | null>(null);
  const [platform, setPlatform] = useState<PlatformId | null>(null);
  const [apiKey, setApiKey] = useState("thine_demo_key");
  const [site, setSite] = useState(siteBase());
  const [rolloutVer, setRolloutVer] = useState("0.2.0");
  const [profile, setProfile] = useState("standard");
  const [msg, setMsg] = useState<string | null>(null);
  const [err, setErr] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  const [lastBootstrapId, setLastBootstrapId] = useState<string | null>(null);
  const [metricCheck, setMetricCheck] = useState<string | null>(null);

  const refresh = useCallback(async () => {
    try {
      const [a, s] = await Promise.all([api.fleetAgents(), api.fleetSummary()]);
      setAgents(a);
      setSummary(s);
    } catch {
      setAgents([]);
      setSummary(null);
    }
  }, []);

  useEffect(() => {
    void refresh();
    const id = setInterval(() => void refresh(), 8_000);
    return () => clearInterval(id);
  }, [refresh]);

  useEffect(() => {
    setTab(initialTab);
  }, [initialTab]);

  const groups = useMemo(() => {
    const map = new Map<string, typeof PLATFORMS>();
    for (const p of PLATFORMS) {
      const arr = map.get(p.group) ?? [];
      arr.push(p);
      map.set(p.group, arr);
    }
    return [...map.entries()];
  }, []);

  const pct = summary?.install_pct ?? (agents.length ? 100 : 0);

  async function installOnThisHost() {
    if (!platform) return;
    setBusy(true);
    setMsg(null);
    setErr(null);
    setMetricCheck(null);
    try {
      const r = await api.fleetBootstrap({
        platform,
        host: `${platform}-local`,
      });
      setLastBootstrapId(r.agent.id);
      setMsg(r.message);
      await refresh();
      // verify metrics shortly after first emit
      await new Promise((res) => setTimeout(res, 1200));
      const end = Date.now();
      const series = await api.query({
        metric: "system.cpu.user",
        start_ms: end - 5 * 60_000,
        end_ms: end,
        step_ms: 10_000,
        aggregation: "avg",
        tags: `agent:${r.agent.id}`,
      });
      const pts = series.reduce((n, s) => n + s.points.length, 0);
      setMetricCheck(
        pts > 0
          ? `Verified: system.cpu.user has ${pts} points from agent ${r.agent.id}`
          : `Agent registered — metrics will appear within ~10s (DogStatsD :8125)`,
      );
      setTab("view");
    } catch (e) {
      setErr(e instanceof Error ? e.message : "Bootstrap failed");
    } finally {
      setBusy(false);
    }
  }

  async function copyScript() {
    if (!platform) return;
    const text = installScript(platform, apiKey, site);
    try {
      await navigator.clipboard.writeText(text);
      setMsg("Install command copied");
    } catch {
      setErr("Clipboard unavailable — select the command and copy manually");
    }
  }

  async function applyConfig() {
    setBusy(true);
    setMsg(null);
    setErr(null);
    try {
      const r = await api.fleetConfigure(profile);
      setMsg(
        `Applied “${r.profile}” to ${r.agents_updated} agents — checks: ${r.checks.join(", ")}`,
      );
      await refresh();
    } catch (e) {
      setErr(e instanceof Error ? e.message : "Configure failed");
    } finally {
      setBusy(false);
    }
  }

  async function runRollout() {
    setBusy(true);
    setMsg(null);
    setErr(null);
    try {
      const r = await api.fleetRollout(rolloutVer);
      setMsg(`Rollout complete — ${r.agents_updated} agents → ${r.target_version}`);
      await refresh();
    } catch (e) {
      setErr(e instanceof Error ? e.message : "Rollout failed");
    } finally {
      setBusy(false);
    }
  }

  async function stopBootstrap() {
    if (!lastBootstrapId) return;
    setBusy(true);
    try {
      await api.fleetBootstrapStop(lastBootstrapId);
      setMsg(`Stopped bootstrap ${lastBootstrapId}`);
      setLastBootstrapId(null);
    } catch (e) {
      setErr(e instanceof Error ? e.message : "Stop failed");
    } finally {
      setBusy(false);
    }
  }

  return (
    <section className="dd-panel fleet-panel">
      <div className="fleet-tabs-top">
        <div className="fleet-brand">
          <strong>Fleet Automation</strong>
          <span className="muted">
            {summary ? `${summary.healthy}/${summary.total} healthy · ${summary.configured} configured` : "…"}
          </span>
        </div>
        <nav className="fleet-subtabs">
          {(
            [
              ["view", "View Agents"],
              ["install", "Install Agents"],
              ["configure", "Configure Agents"],
              ["upgrade", "Upgrade Agents"],
            ] as [Tab, string][]
          ).map(([id, label]) => (
            <button
              key={id}
              type="button"
              className={tab === id ? "on" : ""}
              onClick={() => setTab(id)}
            >
              {label}
              {(id === "configure" || id === "upgrade") && <em className="new-badge">NEW</em>}
            </button>
          ))}
        </nav>
      </div>

      {msg && (
        <div className="dd-banner compact">
          <span>{msg}</span>
        </div>
      )}
      {err && (
        <div className="dd-banner compact">
          <strong>Error</strong>
          <span>{err}</span>
        </div>
      )}
      {metricCheck && (
        <div className="dd-banner compact">
          <span>{metricCheck}</span>
        </div>
      )}

      {tab === "install" && (
        <div className="fleet-install">
          <div className="fleet-banner">
            <div>
              <h2>Collect real-time metrics, logs, and traces by installing the Thine Agent</h2>
              <p className="muted">
                Install commands hit this control plane (<code>/install/*</code> +{" "}
                <code>/api/v1/fleet/bootstrap</code>). “Install on this host” starts a live agent that
                heartbeats and emits DogStatsD metrics — not a stub URL.
              </p>
            </div>
            <div className="fleet-progress">
              <strong>{pct}% of your hosts have Agents installed</strong>
              <div className="fleet-bar">
                <i style={{ width: `${pct}%` }} />
              </div>
              <span className="muted">{agents.length} reporting</span>
            </div>
          </div>

          {!platform ? (
            <>
              <h3>Select platform to install Agents</h3>
              {groups.map(([group, items]) => (
                <div key={group} className="fleet-platform-group">
                  <h4>{group}</h4>
                  <div className="fleet-platform-grid">
                    {items.map((p) => (
                      <button
                        key={p.id}
                        type="button"
                        className="fleet-platform-card"
                        onClick={() => setPlatform(p.id)}
                      >
                        <span className={`fleet-plat-ico ${p.id}`} aria-hidden />
                        <strong>{p.label}</strong>
                        <span className="muted">{p.blurb}</span>
                      </button>
                    ))}
                  </div>
                </div>
              ))}
            </>
          ) : (
            <div className="fleet-install-detail">
              <button type="button" className="ghost" onClick={() => setPlatform(null)}>
                ← All platforms
              </button>
              <h3>Install on {PLATFORMS.find((p) => p.id === platform)?.label}</h3>
              <div className="fleet-keys row">
                <label>
                  API key
                  <input value={apiKey} onChange={(e) => setApiKey(e.target.value)} />
                </label>
                <label>
                  Site
                  <input value={site} onChange={(e) => setSite(e.target.value)} />
                </label>
              </div>
              <pre className="fleet-script">{installScript(platform, apiKey, site)}</pre>
              <p className="muted">
                Scripts are served from <code>{site.replace(/\/$/, "")}/install/install_agent.sh</code>{" "}
                and register via <code>/api/v1/fleet/bootstrap</code> (heartbeat + host metrics).
              </p>
              <div className="row">
                <button type="button" disabled={busy} onClick={() => void installOnThisHost()}>
                  Install on this host
                </button>
                <button type="button" className="ghost" onClick={() => void copyScript()}>
                  Copy command
                </button>
                <button type="button" className="ghost" onClick={() => setTab("view")}>
                  View Agents
                </button>
                {lastBootstrapId && (
                  <button type="button" className="ghost" disabled={busy} onClick={() => void stopBootstrap()}>
                    Stop {lastBootstrapId}
                  </button>
                )}
              </div>
            </div>
          )}
        </div>
      )}

      {tab === "view" && (
        <div className="fleet-view">
          <div className="dd-toolbar">
            <h3 style={{ margin: 0 }}>Agents</h3>
            <button type="button" className="ghost" onClick={() => void refresh()}>
              Refresh
            </button>
            <button type="button" onClick={() => setTab("install")}>
              Install Agents
            </button>
          </div>
          <table className="dd-table">
            <thead>
              <tr>
                <th>ID</th>
                <th>Host</th>
                <th>Platform</th>
                <th>Version</th>
                <th>Profile</th>
                <th>Checks</th>
                <th>Last seen</th>
                <th>Status</th>
              </tr>
            </thead>
            <tbody>
              {agents.map((a) => (
                <tr key={a.id}>
                  <td>
                    <code>{a.id}</code>
                  </td>
                  <td>{a.host}</td>
                  <td>{a.platform || "—"}</td>
                  <td>{a.version}</td>
                  <td>{a.config_profile || "none"}</td>
                  <td className="muted">{(a.checks || []).join(", ") || "—"}</td>
                  <td>
                    {a.last_seen_ms
                      ? new Date(a.last_seen_ms).toLocaleTimeString()
                      : "—"}
                  </td>
                  <td>
                    <span className={`fleet-status ${a.status}`}>{a.status}</span>
                  </td>
                </tr>
              ))}
              {!agents.length && (
                <tr>
                  <td colSpan={8} className="muted">
                    No agents yet — pick a platform and click Install on this host.
                  </td>
                </tr>
              )}
            </tbody>
          </table>
        </div>
      )}

      {tab === "configure" && (
        <div className="fleet-configure">
          <h3>Configure Agents</h3>
          <p className="muted">
            Push a config profile to the fleet. This writes checks + signal toggles on each agent via{" "}
            <code>POST /api/v1/fleet/configure</code>.
          </p>
          <label>
            Profile
            <select value={profile} onChange={(e) => setProfile(e.target.value)}>
              <option value="standard">standard — host metrics + logs + APM</option>
              <option value="apm">apm — host + APM</option>
              <option value="logs">logs — host + logs</option>
              <option value="full">full — all checks</option>
            </select>
          </label>
          <ul className="fleet-config-list">
            <li>
              <strong>Metrics</strong> — DogStatsD UDP <code>:8125</code>
            </li>
            <li>
              <strong>Logs</strong> — enabled for standard / logs / full
            </li>
            <li>
              <strong>APM</strong> — enabled for standard / apm / full
            </li>
          </ul>
          <button type="button" disabled={busy || !agents.length} onClick={() => void applyConfig()}>
            Apply config to {agents.length} agents
          </button>
        </div>
      )}

      {tab === "upgrade" && (
        <div className="fleet-upgrade">
          <h3>Upgrade Agents</h3>
          <p className="muted">
            Roll a target version across the fleet (<code>POST /api/v1/fleet/rollout</code>).
          </p>
          <label>
            Target version
            <input value={rolloutVer} onChange={(e) => setRolloutVer(e.target.value)} />
          </label>
          <button type="button" disabled={busy || !agents.length} onClick={() => void runRollout()}>
            Start rollout
          </button>
        </div>
      )}
    </section>
  );
}
