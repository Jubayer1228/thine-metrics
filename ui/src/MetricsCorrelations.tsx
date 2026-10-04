import { useEffect, useMemo, useState } from "react";
import {
  Area,
  AreaChart,
  CartesianGrid,
  ResponsiveContainer,
  Tooltip,
  XAxis,
  YAxis,
} from "recharts";
import { api, type CorrelationSearchResponse } from "./api";
import { chartGrid, chartTip } from "./theme";
import { fmt } from "./widgets";

type Props = {
  metrics: string[];
  rangeMs: number;
  initialMetric?: string;
};

/**
 * Datadog-parity Metric Correlations explorer (Metrics → Correlations).
 * Finds metrics that behaved irregularly in the same window as the selected series.
 */
export function MetricsCorrelationsPage({ metrics, rangeMs, initialMetric }: Props) {
  const [metric, setMetric] = useState(initialMetric || metrics[0] || "http.server.duration");
  const [sources, setSources] = useState({ apm: true, integrations: true, dashboards: true, custom: false });
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [result, setResult] = useState<CorrelationSearchResponse | null>(null);
  const [selected, setSelected] = useState<string | null>(null);

  useEffect(() => {
    if (initialMetric) setMetric(initialMetric);
  }, [initialMetric]);

  async function run() {
    setBusy(true);
    setError(null);
    const end = Date.now();
    const start = end - rangeMs;
    try {
      const src = Object.entries(sources)
        .filter(([, on]) => on)
        .map(([k]) => k);
      const r = await api.graphCorrelations({
        metric,
        start_ms: start,
        end_ms: end,
        sources: src.length ? src : ["apm", "integrations", "dashboards"],
        limit: 24,
      });
      setResult(r);
      setSelected(r.results[0]?.source ?? null);
    } catch (e) {
      setError(e instanceof Error ? e.message : "Correlation search failed");
    } finally {
      setBusy(false);
    }
  }

  useEffect(() => {
    void run();
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [metric, rangeMs]);

  const selectedGroup = useMemo(
    () => result?.results.find((g) => g.source === selected) ?? result?.results[0] ?? null,
    [result, selected],
  );

  const preview = useMemo(() => {
    if (!selectedGroup?.preview?.length) return [];
    return selectedGroup.preview.map((p) => ({
      t: p.timestamp_ms,
      label: new Date(p.timestamp_ms).toLocaleTimeString([], { hour: "2-digit", minute: "2-digit" }),
      v: p.value,
    }));
  }, [selectedGroup]);

  const totalHits = result?.hits?.length ?? result?.results.reduce((a, g) => a + g.correlations, 0) ?? 0;

  return (
    <section className="dd-panel corr-page">
      <header className="corr-hero">
        <div>
          <h2>Metric Correlations</h2>
          <p className="muted">
            Find metrics that exhibited irregular behavior around the same time — Datadog Graph Insights parity.
          </p>
        </div>
        <button type="button" onClick={() => void run()} disabled={busy}>
          {busy ? "Searching…" : "Run search"}
        </button>
      </header>

      <div className="corr-toolbar">
        <label>
          Focus metric
          <select value={metric} onChange={(e) => setMetric(e.target.value)}>
            {(metrics.includes(metric) ? metrics : [metric, ...metrics]).map((m) => (
              <option key={m} value={m}>
                {m}
              </option>
            ))}
          </select>
        </label>
        <fieldset className="corr-sources">
          <legend>Search sources</legend>
          {(
            [
              ["apm", "APM"],
              ["integrations", "Integrations"],
              ["dashboards", "Dashboards"],
              ["custom", "Custom metrics"],
            ] as const
          ).map(([key, label]) => (
            <label key={key}>
              <input
                type="checkbox"
                checked={sources[key]}
                onChange={(e) => setSources((s) => ({ ...s, [key]: e.target.checked }))}
              />
              {label}
            </label>
          ))}
        </fieldset>
      </div>

      {error && <p className="error">{error}</p>}

      <div className="corr-layout">
        <div className="corr-results obs-card">
          <h3>
            Results{" "}
            <span className="muted">
              {result ? `${result.results.length} sources · ${totalHits} hits` : "—"}
            </span>
          </h3>
          <table className="dd-table compact">
            <thead>
              <tr>
                <th>Type</th>
                <th>Source</th>
                <th>Correlations</th>
              </tr>
            </thead>
            <tbody>
              {(result?.results ?? []).map((g) => (
                <tr
                  key={g.source}
                  className={selected === g.source ? "on" : ""}
                  onClick={() => setSelected(g.source)}
                  style={{ cursor: "pointer" }}
                >
                  <td>
                    <em className="corr-type">{g.source_type || "metric"}</em>
                  </td>
                  <td>
                    <code>{g.source}</code>
                  </td>
                  <td>{g.correlations}</td>
                </tr>
              ))}
              {!result?.results?.length && (
                <tr>
                  <td colSpan={3} className="muted">
                    {busy ? "Scanning…" : "No correlations yet — pick a metric and run search"}
                  </td>
                </tr>
              )}
            </tbody>
          </table>
        </div>

        <div className="corr-detail obs-card">
          <h3>{selectedGroup ? selectedGroup.source : "Select a source"}</h3>
          {selectedGroup?.metrics?.length ? (
            <ul className="corr-metric-list">
              {selectedGroup.metrics.map((m) => (
                <li key={m.metric}>
                  <code>{m.metric}</code>
                  {m.score != null && <span>{fmt(m.score, 2)}</span>}
                </li>
              ))}
            </ul>
          ) : (
            <p className="muted">Correlated metric names appear here when available.</p>
          )}
          <div className="corr-preview">
            {preview.length ? (
              <ResponsiveContainer width="100%" height={220}>
                <AreaChart data={preview} margin={{ top: 8, right: 8, left: 0, bottom: 0 }}>
                  <defs>
                    <linearGradient id="corrGrad" x1="0" y1="0" x2="0" y2="1">
                      <stop offset="0%" stopColor="#4285F4" stopOpacity={0.35} />
                      <stop offset="100%" stopColor="#4285F4" stopOpacity={0.02} />
                    </linearGradient>
                  </defs>
                  <CartesianGrid stroke={chartGrid} vertical={false} strokeDasharray="3 6" />
                  <XAxis dataKey="label" tick={{ fill: "var(--surf-muted)", fontSize: 10 }} minTickGap={28} />
                  <YAxis tick={{ fill: "var(--surf-muted)", fontSize: 10 }} width={42} />
                  <Tooltip contentStyle={chartTip} />
                  <Area type="monotone" dataKey="v" stroke="#4285F4" fill="url(#corrGrad)" strokeWidth={2} dot={false} />
                </AreaChart>
              </ResponsiveContainer>
            ) : (
              <div className="dd-empty">Preview chart for selected correlation</div>
            )}
          </div>
        </div>
      </div>
    </section>
  );
}
