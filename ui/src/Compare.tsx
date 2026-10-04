import { useEffect, useMemo, useState } from "react";
import {
  Bar,
  BarChart,
  CartesianGrid,
  Cell,
  Legend,
  ResponsiveContainer,
  Tooltip,
  XAxis,
  YAxis,
} from "recharts";
import { api, type CompetitorsCompare } from "./api";
import { chartTip } from "./theme";
import { fmt } from "./widgets";

const tip = chartTip;

const VENDORS = [
  { key: "thine", label: "Thine", color: "#5B91EB" },
  { key: "signoz", label: "SigNoz", color: "#F4A261" },
  { key: "grafana", label: "Grafana", color: "#F77FBE" },
  { key: "datadog", label: "Datadog", color: "#A371E3" },
  { key: "new_relic", label: "New Relic", color: "#2EC4B6" },
  { key: "cloudwatch", label: "CloudWatch", color: "#E9C46A" },
  { key: "clickstack", label: "ClickStack", color: "#4CC9F0" },
  { key: "dash0", label: "Dash0", color: "#FF6B6B" },
] as const;

export function ComparePage() {
  const [data, setData] = useState<CompetitorsCompare | null>(null);
  const [focus, setFocus] = useState<"all" | "signoz">("signoz");
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    api.competitorsCompare()
      .then((d) => {
        setData(d);
        setError(null);
      })
      .catch((e) => setError(e instanceof Error ? e.message : "Compare failed"));
  }, []);

  const avgChart = useMemo(() => {
    if (!data) return [];
    return VENDORS.map((v) => ({
      name: v.label,
      score: Number(data.averages[v.key] ?? 0),
      fill: v.color,
    })).sort((a, b) => b.score - a.score);
  }, [data]);

  const capChart = useMemo(() => {
    if (!data) return [];
    return data.capabilities.map((c) => ({
      name: c.capability.length > 22 ? `${c.capability.slice(0, 20)}…` : c.capability,
      full: c.capability,
      Thine: c.thine,
      SigNoz: c.signoz,
      Grafana: c.grafana,
      Datadog: c.datadog,
    }));
  }, [data]);

  return (
    <section className="dd-panel compare-page">
      <div className="docs-hero">
        <div>
          <h2>Competitive landscape</h2>
          <p>
            Thine scored against SigNoz and the vendors SigNoz lists as peers — Grafana, Datadog, New Relic,
            CloudWatch, ClickStack, Dash0. Scores are relative capability (0–100), not marketing claims.
          </p>
        </div>
        <div className="obs-kpi-row docs-stats">
          <div className="obs-kpi">
            <span>Thine avg</span>
            <strong>{data ? fmt(data.averages.thine, 1) : "—"}</strong>
            <small>across capabilities</small>
          </div>
          <div className="obs-kpi">
            <span>SigNoz avg</span>
            <strong>{data ? fmt(data.averages.signoz, 1) : "—"}</strong>
            <small>OTel-native peer</small>
          </div>
          <div className="obs-kpi">
            <span>DD catalog parity</span>
            <strong>{data ? `${data.datadog_avg_parity_pct}%` : "—"}</strong>
            <small>{data ? `${data.catalog_features} features` : ""}</small>
          </div>
        </div>
      </div>

      {error && (
        <div className="dd-banner">
          <strong>Compare load failed</strong>
          <span>{error}</span>
        </div>
      )}

      {data && (
        <>
          <div className="obs-card">
            <h3>Average capability score by vendor</h3>
            <div className="obs-chart tall">
              <ResponsiveContainer width="100%" height="100%">
                <BarChart data={avgChart} layout="vertical" margin={{ left: 88, right: 16 }}>
                  <CartesianGrid stroke="rgba(255,255,255,0.06)" horizontal={false} />
                  <XAxis type="number" domain={[0, 100]} tick={{ fill: "#8B97A8", fontSize: 10 }} />
                  <YAxis type="category" dataKey="name" tick={{ fill: "#8B97A8", fontSize: 11 }} width={84} />
                  <Tooltip contentStyle={tip} />
                  <Bar dataKey="score" name="Avg score" radius={[0, 4, 4, 0]}>
                    {avgChart.map((e) => (
                      <Cell key={e.name} fill={e.fill} />
                    ))}
                  </Bar>
                </BarChart>
              </ResponsiveContainer>
            </div>
            <p className="muted" style={{ marginTop: "0.5rem" }}>
              Source: Thine capability matrix · live API <code>/api/v1/compare</code>
            </p>
          </div>

          <div className="obs-toolbar">
            <button className={focus === "signoz" ? "ghost on" : "ghost"} onClick={() => setFocus("signoz")}>
              Thine vs SigNoz
            </button>
            <button className={focus === "all" ? "ghost on" : "ghost"} onClick={() => setFocus("all")}>
              All vendors
            </button>
          </div>

          {focus === "signoz" && (
            <div className="obs-section">
              <p className="obs-lead">{data.vs_signoz.summary}</p>
              <div className="obs-grid-2">
                <div className="obs-card">
                  <h3>Where Thine leads SigNoz</h3>
                  <table className="dd-table compact">
                    <thead>
                      <tr>
                        <th>Capability</th>
                        <th>Thine</th>
                        <th>SigNoz</th>
                        <th>Δ</th>
                      </tr>
                    </thead>
                    <tbody>
                      {data.vs_signoz.thine_wins
                        .slice()
                        .sort((a, b) => b.delta - a.delta)
                        .map((r) => (
                          <tr key={r.capability}>
                            <td>{r.capability}</td>
                            <td>{r.thine}</td>
                            <td>{r.signoz}</td>
                            <td>+{r.delta}</td>
                          </tr>
                        ))}
                    </tbody>
                  </table>
                </div>
                <div className="obs-card">
                  <h3>Where SigNoz leads Thine</h3>
                  <table className="dd-table compact">
                    <thead>
                      <tr>
                        <th>Capability</th>
                        <th>Thine</th>
                        <th>SigNoz</th>
                        <th>Δ</th>
                      </tr>
                    </thead>
                    <tbody>
                      {data.vs_signoz.signoz_leads
                        .slice()
                        .sort((a, b) => b.delta - a.delta)
                        .map((r) => (
                          <tr key={r.capability}>
                            <td>{r.capability}</td>
                            <td>{r.thine}</td>
                            <td>{r.signoz}</td>
                            <td>+{r.delta}</td>
                          </tr>
                        ))}
                    </tbody>
                  </table>
                </div>
              </div>
            </div>
          )}

          {focus === "all" && (
            <div className="obs-card">
              <h3>Capability heatmap (selected vendors)</h3>
              <div className="obs-chart" style={{ height: 420 }}>
                <ResponsiveContainer width="100%" height="100%">
                  <BarChart data={capChart} margin={{ bottom: 8, left: 8, right: 8 }}>
                    <CartesianGrid stroke="rgba(255,255,255,0.06)" vertical={false} />
                    <XAxis dataKey="name" tick={{ fill: "#8B97A8", fontSize: 9 }} interval={0} angle={-25} textAnchor="end" height={70} />
                    <YAxis domain={[0, 100]} tick={{ fill: "#8B97A8", fontSize: 10 }} />
                    <Tooltip contentStyle={tip} />
                    <Legend />
                    <Bar dataKey="Thine" fill="#5B91EB" />
                    <Bar dataKey="SigNoz" fill="#F4A261" />
                    <Bar dataKey="Grafana" fill="#F77FBE" />
                    <Bar dataKey="Datadog" fill="#A371E3" />
                  </BarChart>
                </ResponsiveContainer>
              </div>
            </div>
          )}

          <div className="obs-card">
            <h3>When to pick whom</h3>
            <table className="dd-table">
              <thead>
                <tr>
                  <th>Choose</th>
                  <th>When</th>
                </tr>
              </thead>
              <tbody>
                {data.pick_guide.map((p) => (
                  <tr key={p.choose}>
                    <td>
                      <strong>{p.choose}</strong>
                    </td>
                    <td>{p.when}</td>
                  </tr>
                ))}
              </tbody>
            </table>
          </div>

          <div className="docs-list">
            {data.profiles.map((p) => (
              <article key={p.name} className="docs-card">
                <header>
                  <div>
                    <h3>{p.name}</h3>
                    <code>{p.deployment}</code>
                  </div>
                  <div className="docs-badges">
                    <span className="pill muted-pill">{p.license_model}</span>
                  </div>
                </header>
                <p>{p.positioning}</p>
                <div className="docs-meta">
                  <div>
                    <span>Strengths</span>
                    <ul>
                      {p.strengths.map((s) => (
                        <li key={s}>{s}</li>
                      ))}
                    </ul>
                  </div>
                  <div>
                    <span>Tradeoffs</span>
                    <ul>
                      {p.weaknesses.map((s) => (
                        <li key={s}>{s}</li>
                      ))}
                    </ul>
                    <span>Best when</span>
                    <p>{p.best_when}</p>
                  </div>
                </div>
              </article>
            ))}
          </div>

          <div className="obs-card">
            <h3>Full capability matrix</h3>
            <div style={{ overflowX: "auto" }}>
              <table className="dd-table compact">
                <thead>
                  <tr>
                    <th>Capability</th>
                    {VENDORS.map((v) => (
                      <th key={v.key}>{v.label}</th>
                    ))}
                  </tr>
                </thead>
                <tbody>
                  {data.capabilities.map((c) => (
                    <tr key={c.capability}>
                      <td>
                        <strong>{c.capability}</strong>
                        <div className="tags">{c.note}</div>
                      </td>
                      <td>{c.thine}</td>
                      <td>{c.signoz}</td>
                      <td>{c.grafana}</td>
                      <td>{c.datadog}</td>
                      <td>{c.new_relic}</td>
                      <td>{c.cloudwatch}</td>
                      <td>{c.clickstack}</td>
                      <td>{c.dash0}</td>
                    </tr>
                  ))}
                </tbody>
              </table>
            </div>
          </div>
        </>
      )}
    </section>
  );
}
