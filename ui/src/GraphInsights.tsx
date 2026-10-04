import { useEffect, useState } from "react";
import {
  api,
  type CorrelationSearchResponse,
  type DashboardAnomalyIssue,
  type WatchdogExplainResult,
} from "./api";
import { Sparkline, fmt } from "./widgets";

type Props = {
  boardId: string;
  rangeMs: number;
  startMs: number;
  endMs: number;
  issues: DashboardAnomalyIssue[];
  autoDetect: boolean;
  onAutoDetectChange: (v: boolean) => void;
  onFocusWidget: (widgetId: string) => void;
  selectedMetric?: string | null;
};

export function GraphInsightsPanel({
  rangeMs,
  startMs,
  endMs,
  issues,
  autoDetect,
  onAutoDetectChange,
  onFocusWidget,
  selectedMetric,
}: Props) {
  const [tab, setTab] = useState<"investigate" | "correlations" | "explain">("investigate");
  const [corr, setCorr] = useState<CorrelationSearchResponse | null>(null);
  const [explain, setExplain] = useState<WatchdogExplainResult | null>(null);
  const [metric, setMetric] = useState(selectedMetric || "http.server.duration");
  const [busy, setBusy] = useState(false);
  const [expandedIssue, setExpandedIssue] = useState<string | null>(null);

  useEffect(() => {
    if (selectedMetric) {
      setMetric(selectedMetric);
      setTab("correlations");
    }
  }, [selectedMetric]);

  async function runCorrelations() {
    setBusy(true);
    try {
      const r = await api.graphCorrelations({
        metric,
        start_ms: startMs,
        end_ms: endMs,
        sources: ["apm", "integrations", "dashboards"],
        limit: 20,
      });
      setCorr(r);
      setTab("correlations");
    } finally {
      setBusy(false);
    }
  }

  async function runExplain() {
    setBusy(true);
    try {
      const r = await api.graphExplain({
        metric,
        start_ms: startMs,
        end_ms: endMs,
        group_keys: ["service", "env", "gpu_id", "host"],
      });
      setExplain(r);
      setTab("explain");
    } finally {
      setBusy(false);
    }
  }

  return (
    <aside className="gi-panel">
      <header className="gi-head">
        <div>
          <strong>Graph Insights</strong>
          <small>
            <a href="https://docs.datadoghq.com/dashboards/graph_insights/" target="_blank" rel="noreferrer">
              docs
            </a>
          </small>
        </div>
        <label className="gi-toggle">
          <input
            type="checkbox"
            checked={autoDetect}
            onChange={(e) => onAutoDetectChange(e.target.checked)}
          />
          Auto-detect issues
        </label>
      </header>

      <div className="gi-tabs">
        <button className={tab === "investigate" ? "on" : ""} onClick={() => setTab("investigate")}>
          Investigate ({issues.length})
        </button>
        <button className={tab === "correlations" ? "on" : ""} onClick={() => setTab("correlations")}>
          Correlations
        </button>
        <button className={tab === "explain" ? "on" : ""} onClick={() => setTab("explain")}>
          Watchdog Explains
        </button>
      </div>

      {tab === "investigate" && (
        <div className="gi-body">
          {!autoDetect && <p className="muted">Anomaly detection is off for this view.</p>}
          {autoDetect && issues.length === 0 && (
            <p className="muted">No anomalies detected on timeseries widgets in this window.</p>
          )}
          {issues.map((issue) => (
            <div key={issue.id} className={`gi-issue ${expandedIssue === issue.id ? "open" : ""}`}>
              <button
                className="gi-issue-title"
                onClick={() => {
                  setExpandedIssue(expandedIssue === issue.id ? null : issue.id);
                  if (issue.widget_ids[0]) onFocusWidget(issue.widget_ids[0]);
                }}
              >
                <span className={`sev ${issue.anomaly.severity}`}>{issue.anomaly.severity}</span>
                {issue.title}
              </button>
              {expandedIssue === issue.id && (
                <div className="gi-issue-body">
                  <p>{issue.anomaly.summary}</p>
                  <div className="gi-tags">
                    <strong>Influential tags</strong>
                    {issue.influential_tags.length === 0 && <span className="muted">—</span>}
                    {issue.influential_tags.map((t) => (
                      <code key={`${t.key}:${t.value}`}>
                        {t.key}:{t.value} ({fmt(t.contribution * 100, 0)}%)
                      </code>
                    ))}
                  </div>
                  {issue.co_occurring_metrics.length > 1 && (
                    <div className="gi-tags">
                      <strong>Co-occurs with</strong>
                      {issue.co_occurring_metrics.map((m) => (
                        <code key={m}>{m}</code>
                      ))}
                    </div>
                  )}
                  <div className="gi-tags">
                    <strong>Next steps</strong>
                    <ul>
                      {issue.next_steps.map((s) => (
                        <li key={s}>{s}</li>
                      ))}
                    </ul>
                  </div>
                  <div className="gi-actions">
                    <button
                      onClick={() => {
                        setMetric(issue.metric);
                        void runExplain();
                      }}
                    >
                      Watchdog Explains
                    </button>
                    <button
                      onClick={() => {
                        setMetric(issue.metric);
                        void runCorrelations();
                      }}
                    >
                      Find correlated metrics
                    </button>
                  </div>
                </div>
              )}
            </div>
          ))}
        </div>
      )}

      {tab === "correlations" && (
        <div className="gi-body">
          <div className="gi-search">
            <input value={metric} onChange={(e) => setMetric(e.target.value)} />
            <button disabled={busy} onClick={() => void runCorrelations()}>
              {busy ? "Searching…" : "Find correlated metrics"}
            </button>
          </div>
          <p className="muted">
            Window {new Date(startMs).toLocaleTimeString()} – {new Date(endMs).toLocaleTimeString()} · range{" "}
            {Math.round(rangeMs / 60_000)}m
          </p>
          {corr && (
            <>
              <p className="muted">
                Interest {new Date(corr.interest_start_ms).toLocaleTimeString()} –{" "}
                {new Date(corr.interest_end_ms).toLocaleTimeString()} · {corr.hits.length} hits
              </p>
              <table className="dd-table compact">
                <thead>
                  <tr>
                    <th>Type</th>
                    <th>Source</th>
                    <th>#</th>
                    <th>Preview</th>
                  </tr>
                </thead>
                <tbody>
                  {corr.results.map((g) => (
                    <tr key={`${g.source_type}:${g.source}`}>
                      <td>
                        <code>{g.source_type}</code>
                      </td>
                      <td>
                        {g.source}
                        <div className="muted">{g.metrics[0]?.metric}</div>
                      </td>
                      <td>{g.correlations}</td>
                      <td style={{ width: 90 }}>
                        <Sparkline points={g.preview.slice(-40)} color="#F77FBE" />
                      </td>
                    </tr>
                  ))}
                </tbody>
              </table>
            </>
          )}
        </div>
      )}

      {tab === "explain" && (
        <div className="gi-body">
          <div className="gi-search">
            <input value={metric} onChange={(e) => setMetric(e.target.value)} />
            <button disabled={busy} onClick={() => void runExplain()}>
              {busy ? "Analyzing…" : "Investigate anomaly"}
            </button>
          </div>
          {explain && (
            <>
              <p>{explain.summary}</p>
              <p className="muted">
                {explain.affects_everyone
                  ? "Likely affects everyone (broad signal)"
                  : "Isolated to specific tag groups"}
              </p>
              {explain.anomaly && (
                <div className="gi-chip">
                  <strong>Pink region</strong> z={fmt(explain.anomaly.z_score, 2)} ·{" "}
                  {fmt(explain.anomaly.deviation_pct, 0)}% vs expected
                </div>
              )}
              <h4>Findings</h4>
              {explain.findings.length === 0 && <p className="muted">No strong tag drivers.</p>}
              {explain.findings.map((f) => (
                <div key={`${f.tag_key}:${f.tag_value}`} className="gi-finding">
                  <code>
                    {f.tag_key}:{f.tag_value}
                  </code>
                  <span>{fmt(f.contribution * 100, 0)}%</span>
                  <p>{f.message}</p>
                  <small className="muted">
                    with {fmt(f.with_tag_peak)} → without {fmt(f.without_tag_peak)}
                  </small>
                </div>
              ))}
            </>
          )}
        </div>
      )}
    </aside>
  );
}
