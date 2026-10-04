import { useEffect, useState } from "react";
import { api, type FleetAgent } from "./api";

type Props = {
  onInstallAgent: () => void;
  onOpenDashboards: () => void;
};

export function GetStartedPage({ onInstallAgent, onOpenDashboards }: Props) {
  const [agents, setAgents] = useState<FleetAgent[]>([]);
  const [loading, setLoading] = useState(true);

  useEffect(() => {
    api
      .fleetAgents()
      .then(setAgents)
      .catch(() => setAgents([]))
      .finally(() => setLoading(false));
  }, []);

  const hasInfra = agents.length > 0;

  return (
    <section className="dd-panel get-started">
      <div className="gs-hero">
        <div>
          <h2>Welcome — let&apos;s start monitoring your stack</h2>
          <p className="muted">
            Install the Thine Agent to collect real-time CPU, memory, disk, network, logs, and
            traces — then build dashboards and monitors like the Datadog tutorial flow.
          </p>
        </div>
        <div className="gs-trial">
          <span>Local demo workspace</span>
          <button type="button" className="ghost" onClick={onOpenDashboards}>
            Open Dashboards
          </button>
        </div>
      </div>

      <div className={`gs-cta${hasInfra ? " live" : ""}`}>
        <div>
          <h3>{loading ? "Checking infrastructure…" : hasInfra ? "Infrastructure detected" : "No Infrastructure Detected"}</h3>
          <p>
            {hasInfra
              ? `${agents.length} agent${agents.length === 1 ? "" : "s"} reporting — you can explore Fleet Automation or jump into dashboards.`
              : "Install your first agent to monitor hosts with live CPU, memory, disk, and network metrics."}
          </p>
        </div>
        <button type="button" className="gs-primary" onClick={onInstallAgent}>
          {hasInfra ? "Manage Agents →" : "Install Your First Agent →"}
        </button>
      </div>

      <div className="gs-preview">
        <header>
          <strong>{hasInfra ? "Your hosts are live" : "Your first host is live!"}</strong>
          <span className="muted">Preview of essential host widgets after agent install</span>
        </header>
        <div className="gs-preview-grid">
          <PreviewCard title="CPU usage (%)" kind="area" />
          <PreviewCard title="Processes memory usage" kind="treemap" />
          <PreviewCard title="Disk usage by device (%)" kind="line" />
        </div>
      </div>

      <div className="gs-steps">
        <h3>Tutorial path</h3>
        <ol>
          <li>
            <strong>Install Agent</strong> — Fleet Automation → pick Linux / Docker / Kubernetes
          </li>
          <li>
            <strong>Create a dashboard</strong> — Timeboard for troubleshooting, Screenboard for status
          </li>
          <li>
            <strong>Add Essential Widgets</strong> — Timeseries, Query Value, Top List, Golden Signals
          </li>
          <li>
            <strong>Template variables</strong> — one board for every env / service / region
          </li>
          <li>
            <strong>Create monitors</strong> — threshold alerts with recovery + severity routing
          </li>
        </ol>
      </div>
    </section>
  );
}

function PreviewCard({ title, kind }: { title: string; kind: "area" | "treemap" | "line" }) {
  return (
    <article className="gs-preview-card">
      <h4>{title}</h4>
      <div className={`gs-viz ${kind}`} aria-hidden>
        {kind === "treemap" ? (
          <>
            <div className="tm a">
              chrome
              <em>23.07</em>
            </div>
            <div className="tm b">
              kernel
              <em>22.45</em>
            </div>
            <div className="tm c">
              other
              <em>18.2</em>
            </div>
          </>
        ) : (
          <svg viewBox="0 0 200 80" preserveAspectRatio="none">
            {kind === "area" ? (
              <>
                <path d="M0 60 C30 50,50 20,80 35 S140 10,200 28 L200 80 L0 80 Z" fill="rgba(99,44,166,0.25)" />
                <path d="M0 65 C40 55,70 40,100 48 S160 30,200 42" fill="none" stroke="#e07a3d" strokeWidth="2" />
                <path d="M0 70 C35 60,75 55,110 50 S155 45,200 38" fill="none" stroke="#632ca6" strokeWidth="2" />
              </>
            ) : (
              <>
                <path d="M0 55 C40 50,80 30,120 40 S170 20,200 35" fill="none" stroke="#3d8bfd" strokeWidth="2" />
                <path d="M0 65 C50 60,90 55,130 50 S170 48,200 45" fill="none" stroke="#2ec4b6" strokeWidth="2" />
              </>
            )}
          </svg>
        )}
      </div>
    </article>
  );
}
