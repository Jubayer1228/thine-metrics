import { useEffect, useMemo, useState } from "react";
import { api, type CatalogService, type ServiceScorecard } from "./api";

type Props = {
  onOpenService?: (name: string) => void;
};

/** Datadog Software Catalog — ownership, tier, scorecards, service-definition YAML. */
export function SoftwareCatalogPage({ onOpenService }: Props) {
  const [rows, setRows] = useState<CatalogService[]>([]);
  const [scorecards, setScorecards] = useState<ServiceScorecard[]>([]);
  const [q, setQ] = useState("");
  const [error, setError] = useState<string | null>(null);
  const [selected, setSelected] = useState<string | null>(null);
  const [yaml, setYaml] = useState("");
  const [saving, setSaving] = useState(false);

  async function refresh() {
    try {
      const [c, sc] = await Promise.all([api.catalogServices(), api.catalogScorecards()]);
      setRows(c);
      setScorecards(sc);
      setError(null);
    } catch (e) {
      setError(e instanceof Error ? e.message : "Catalog load failed");
    }
  }

  useEffect(() => {
    void refresh();
  }, []);

  useEffect(() => {
    if (!selected) return;
    api
      .catalogDefinition(selected)
      .then((d) => setYaml(d.yaml))
      .catch(() => setYaml(""));
  }, [selected]);

  const filtered = useMemo(() => {
    const s = q.trim().toLowerCase();
    if (!s) return rows;
    return rows.filter(
      (r) =>
        r.name.toLowerCase().includes(s) ||
        (r.team ?? "").toLowerCase().includes(s) ||
        (r.tier ?? "").toLowerCase().includes(s),
    );
  }, [rows, q]);

  const scoreBy = useMemo(() => {
    const m = new Map<string, ServiceScorecard>();
    scorecards.forEach((s) => m.set(s.service, s));
    return m;
  }, [scorecards]);

  async function saveYaml() {
    if (!selected) return;
    setSaving(true);
    try {
      await api.putCatalogDefinition(selected, yaml);
      await refresh();
    } catch (e) {
      setError(e instanceof Error ? e.message : "Save definition failed");
    } finally {
      setSaving(false);
    }
  }

  return (
    <section className="dd-panel corr-page">
      <header className="corr-hero">
        <div>
          <h2>Software Catalog</h2>
          <p className="muted">
            Inventory + Scorecards + service-definition YAML (Datadog Software Catalog workflow).
          </p>
        </div>
        <input placeholder="Filter name / team / tier" value={q} onChange={(e) => setQ(e.target.value)} />
      </header>
      {error && <p className="error">{error}</p>}
      <table className="dd-table">
        <thead>
          <tr>
            <th>Service</th>
            <th>Team</th>
            <th>Tier</th>
            <th>Scorecard</th>
            <th>Languages</th>
            <th>Links</th>
          </tr>
        </thead>
        <tbody>
          {filtered.map((r) => {
            const sc = scoreBy.get(r.name);
            return (
              <tr
                key={r.name}
                style={{ cursor: "pointer" }}
                className={selected === r.name ? "on" : ""}
                onClick={() => {
                  setSelected(r.name);
                  onOpenService?.(r.name);
                }}
              >
                <td>
                  <code>{r.name}</code>
                </td>
                <td>{r.team || "—"}</td>
                <td>{r.tier || "—"}</td>
                <td>
                  {sc ? (
                    <span className={`sev ${sc.score_pct >= 80 ? "ok" : sc.score_pct >= 50 ? "warn" : "alert"}`}>
                      {sc.score_pct.toFixed(0)}%
                    </span>
                  ) : (
                    "—"
                  )}
                </td>
                <td>{r.languages?.length ? r.languages.join(", ") : "—"}</td>
                <td className="tags">
                  {r.links
                    ? Object.entries(r.links)
                        .slice(0, 3)
                        .map(([k, v]) => (
                          <a key={k} href={String(v)} target="_blank" rel="noreferrer" onClick={(e) => e.stopPropagation()}>
                            {k}
                          </a>
                        ))
                    : "—"}
                </td>
              </tr>
            );
          })}
          {!filtered.length && (
            <tr>
              <td colSpan={6} className="muted">
                No catalog entities
              </td>
            </tr>
          )}
        </tbody>
      </table>

      {selected && (
        <div className="obs-grid-2" style={{ marginTop: "1rem" }}>
          <div className="obs-card">
            <h3>Scorecard · {selected}</h3>
            <ul className="corr-metric-list">
              {(scoreBy.get(selected)?.checks ?? []).map((c) => (
                <li key={c.id}>
                  <span>
                    {c.passed ? "✓" : "✗"} {c.name}
                  </span>
                  <span className="muted">{c.detail}</span>
                </li>
              ))}
            </ul>
          </div>
          <div className="obs-card">
            <h3>Service definition YAML</h3>
            <textarea
              value={yaml}
              onChange={(e) => setYaml(e.target.value)}
              rows={14}
              style={{ width: "100%", fontFamily: "ui-monospace, monospace", fontSize: 12 }}
            />
            <button type="button" disabled={saving} onClick={() => void saveYaml()} style={{ marginTop: 8 }}>
              {saving ? "Saving…" : "Save definition"}
            </button>
          </div>
        </div>
      )}
    </section>
  );
}
