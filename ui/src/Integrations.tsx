import { useCallback, useEffect, useMemo, useState } from "react";
import {
  api,
  type DoraMetrics,
  type Incident,
  type IntegrationTile,
  type SloEntry,
} from "./api";
import { fmt } from "./widgets";

export function IntegrationsPage() {
  const [tiles, setTiles] = useState<IntegrationTile[]>([]);
  const [q, setQ] = useState("");
  const [category, setCategory] = useState("all");
  const [marketplace, setMarketplace] = useState<{ id: string; name: string; installed: boolean }[]>([]);
  const [slos, setSlos] = useState<SloEntry[]>([]);
  const [incidents, setIncidents] = useState<Incident[]>([]);
  const [dora, setDora] = useState<DoraMetrics | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState<string | null>(null);
  const [dd, setDd] = useState<Record<string, unknown> | null>(null);

  const load = useCallback(async () => {
    try {
      const [t, m, s, i, d, c] = await Promise.all([
        q.trim() ? api.integrationsSearch(q.trim()) : api.integrations(),
        api.marketplace(),
        api.slos(),
        api.incidents(),
        api.dora(),
        api.datadogCompare(),
      ]);
      setTiles(t);
      setMarketplace(m);
      setSlos(s);
      setIncidents(i);
      setDora(d);
      setDd(c);
      setError(null);
    } catch (e) {
      setError(e instanceof Error ? e.message : "Load failed");
    }
  }, [q]);

  useEffect(() => {
    void load();
  }, [load]);

  const categories = useMemo(() => {
    const set = new Set(tiles.map((t) => t.category));
    return ["all", ...Array.from(set).sort()];
  }, [tiles]);

  const filtered = useMemo(
    () => (category === "all" ? tiles : tiles.filter((t) => t.category === category)),
    [tiles, category],
  );

  const enabled = tiles.filter((t) => t.enabled).length;

  async function install(id: string) {
    setBusy(id);
    try {
      await api.installApp(id);
      await load();
    } catch (e) {
      setError(e instanceof Error ? e.message : "Install failed");
    } finally {
      setBusy(null);
    }
  }

  return (
    <section className="dd-panel integ-page">
      {error && <div className="dd-banner">{error}</div>}
      <div className="obs-kpi-row">
        <div className="obs-kpi">
          <span>Integration tiles</span>
          <strong>{tiles.length}</strong>
          <small>{enabled} enabled</small>
        </div>
        <div className="obs-kpi">
          <span>Open incidents</span>
          <strong>{incidents.filter((i) => i.status === "open").length}</strong>
        </div>
        <div className="obs-kpi">
          <span>SLOs</span>
          <strong>{slos.length}</strong>
          <small>{slos.filter((s) => s.status === "breaching").length} breaching</small>
        </div>
        <div className="obs-kpi">
          <span>DORA deploy/day</span>
          <strong>{dora ? fmt(dora.deploy_frequency_per_day, 1) : "—"}</strong>
          <small>AI impact {dora ? fmt(dora.ai_impact_score * 100, 0) : "—"}%</small>
        </div>
        <div className="obs-kpi">
          <span>vs Datadog parity</span>
          <strong>{dd ? `${fmt(Number(dd.avg_parity_pct ?? 0), 0)}%` : "—"}</strong>
          <small>live catalog score</small>
        </div>
      </div>

      <div className="integ-toolbar">
        <input
          placeholder="Search integrations"
          value={q}
          onChange={(e) => setQ(e.target.value)}
        />
        <div className="integ-cats">
          {categories.slice(0, 12).map((c) => (
            <button key={c} type="button" className={category === c ? "on" : ""} onClick={() => setCategory(c)}>
              {c}
            </button>
          ))}
        </div>
      </div>

      <div className="obs-grid-2">
        <div className="obs-card">
          <h3>Marketplace apps</h3>
          <table className="dd-table compact">
            <thead>
              <tr>
                <th>App</th>
                <th>Status</th>
                <th />
              </tr>
            </thead>
            <tbody>
              {marketplace.map((m) => (
                <tr key={m.id}>
                  <td>{m.name}</td>
                  <td>{m.installed ? "Installed" : "Available"}</td>
                  <td>
                    {!m.installed && (
                      <button type="button" className="ghost" disabled={busy === m.id} onClick={() => void install(m.id)}>
                        {busy === m.id ? "…" : "Install"}
                      </button>
                    )}
                  </td>
                </tr>
              ))}
              {!marketplace.length && (
                <tr>
                  <td colSpan={3} className="tags">
                    No marketplace apps
                  </td>
                </tr>
              )}
            </tbody>
          </table>
          {incidents.length > 0 && (
            <>
              <h3 style={{ marginTop: "1rem" }}>Incidents</h3>
              <table className="dd-table compact">
                <thead>
                  <tr>
                    <th>Title</th>
                    <th>Sev</th>
                    <th>Service</th>
                    <th>Status</th>
                  </tr>
                </thead>
                <tbody>
                  {incidents.map((i) => (
                    <tr key={i.id}>
                      <td>{i.title}</td>
                      <td>
                        <span className={`sev ${i.severity}`}>{i.severity}</span>
                      </td>
                      <td>
                        <code>{i.service}</code>
                      </td>
                      <td>{i.status}</td>
                    </tr>
                  ))}
                </tbody>
              </table>
            </>
          )}
        </div>

        <div className="obs-card">
          <h3>SLO burn</h3>
          <table className="dd-table compact">
            <thead>
              <tr>
                <th>SLO</th>
                <th>Current</th>
                <th>Target</th>
                <th>Budget</th>
                <th>Status</th>
              </tr>
            </thead>
            <tbody>
              {slos.map((s) => (
                <tr key={s.id}>
                  <td>
                    <strong>{s.name}</strong>
                    <div className="tags">
                      <code>{s.metric}</code>
                    </div>
                  </td>
                  <td>{fmt(s.current_pct, 1)}%</td>
                  <td>{fmt(s.target, 0)}%</td>
                  <td>{fmt(s.budget_left, 1)}</td>
                  <td>
                    <span className={`sev ${s.status === "breaching" ? "alert" : "ok"}`}>{s.status}</span>
                  </td>
                </tr>
              ))}
            </tbody>
          </table>
          {dora && (
            <div className="dora-strip">
              <div>
                <span>Lead time</span>
                <strong>{fmt(dora.lead_time_hours, 1)}h</strong>
              </div>
              <div>
                <span>CFR</span>
                <strong>{fmt(dora.change_failure_rate * 100, 1)}%</strong>
              </div>
              <div>
                <span>MTTR</span>
                <strong>{fmt(dora.mttr_hours, 1)}h</strong>
              </div>
            </div>
          )}
        </div>
      </div>

      <div className="obs-card" style={{ marginTop: "0.75rem" }}>
        <h3>
          Integration catalog <small className="muted">({filtered.length} shown)</small>
        </h3>
        <div className="integ-grid">
          {filtered.slice(0, 48).map((t) => (
            <div key={t.id} className={`integ-tile${t.enabled ? " on" : ""}`}>
              <strong>{t.title}</strong>
              <span>{t.category}</span>
              <em>{t.enabled ? "Enabled" : "Available"}</em>
            </div>
          ))}
        </div>
      </div>
    </section>
  );
}
