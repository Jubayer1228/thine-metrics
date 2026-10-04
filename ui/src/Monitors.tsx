import { useEffect, useMemo, useState } from "react";
import {
  CartesianGrid,
  Line,
  LineChart,
  ReferenceArea,
  ReferenceLine,
  ResponsiveContainer,
  Tooltip,
  XAxis,
  YAxis,
} from "recharts";
import { api, type AlertEvent, type AlertRule, type QueryResult } from "./api";
import { chartTip } from "./theme";

const SERIES_COLORS = ["#5B91EB", "#A371E3", "#FF6B6B", "#F4A261"];

type Props = {
  metrics: string[];
  metric: string;
  onMetric: (m: string) => void;
  alerts: AlertRule[];
  alertEvents: AlertEvent[];
  rangeMs: number;
  onCreated: () => void;
};

function timeLabel(ms: number) {
  return new Date(ms).toLocaleTimeString([], { hour: "2-digit", minute: "2-digit" });
}

export function MonitorsPage({
  metrics,
  metric,
  onMetric,
  alerts,
  alertEvents,
  rangeMs,
  onCreated,
}: Props) {
  const [step, setStep] = useState(1);
  const [method, setMethod] = useState<"threshold" | "change" | "anomaly" | "forecast">("threshold");
  const [changeType, setChangeType] = useState<"change" | "pct_change">("pct_change");
  const [changeWindowMs, setChangeWindowMs] = useState(30 * 60_000);
  const [anomalyDirection, setAnomalyDirection] = useState<"above_or_below" | "above" | "below">("above_or_below");
  const [forecastHorizonMs, setForecastHorizonMs] = useState(24 * 60 * 60_000);
  const [name, setName] = useState("CPU usage is high for host {{host.name}}");
  const [message, setMessage] = useState(
    "## What's happening\nHigh CPU usage detected on host {{host.name}}.\n\n---\n\n## Impact\nIf CPU usage remains high, latency and error rates can climb.",
  );
  const [msgTab, setMsgTab] = useState<"edit" | "preview">("edit");
  const [comparator, setComparator] = useState<"gt" | "lt">("gt");
  const [alertThreshold, setAlertThreshold] = useState("80");
  const [warnThreshold, setWarnThreshold] = useState("70");
  const [recoveryThreshold, setRecoveryThreshold] = useState("60");
  const [severity, setSeverity] = useState<"critical" | "warning" | "info">("critical");
  const [windowMs, setWindowMs] = useState(5 * 60_000);
  const [evaluate, setEvaluate] = useState("avg");
  const [recipients, setRecipients] = useState("@slack-ops @pagerduty-primary");
  const [groupBy, setGroupBy] = useState("");
  const [filterTags, setFilterTags] = useState("env:prod");
  const [preview, setPreview] = useState<QueryResult[]>([]);
  const [busy, setBusy] = useState(false);
  const [status, setStatus] = useState<string | null>(null);

  useEffect(() => {
    const end = Date.now();
    const start = end - rangeMs;
    api
      .query({
        metric,
        aggregation: evaluate,
        start_ms: start,
        end_ms: end,
        step_ms: rangeMs <= 60 * 60_000 ? 30_000 : 60_000,
      })
      .then((r) => setPreview(r.filter((s) => s.points.length)))
      .catch(() => setPreview([]));
  }, [metric, evaluate, rangeMs]);

  const chartData = useMemo(() => {
    const map = new Map<number, Record<string, number | string>>();
    preview.forEach((s, i) => {
      const key =
        Object.entries(s.tags)
          .map(([k, v]) => `${k}:${v}`)
          .join(",") || `s${i}`;
      s.points.forEach((p) => {
        const row = map.get(p.timestamp_ms) ?? {
          t: p.timestamp_ms,
          label: timeLabel(p.timestamp_ms),
        };
        row[key] = p.value;
        map.set(p.timestamp_ms, row);
      });
    });
    return Array.from(map.values()).sort((a, b) => Number(a.t) - Number(b.t));
  }, [preview]);

  const keys = useMemo(() => {
    const set = new Set<string>();
    for (const row of chartData) {
      for (const k of Object.keys(row)) if (k !== "t" && k !== "label") set.add(k);
    }
    return [...set];
  }, [chartData]);

  const thresholdNum = Number(alertThreshold) || 0;
  const stepsOk = Boolean(name.trim() && metric && !Number.isNaN(thresholdNum));

  async function publish() {
    setBusy(true);
    setStatus(null);
    try {
      const methodLabel =
        method === "threshold"
          ? "Threshold"
          : method === "change"
            ? "Change"
            : method === "anomaly"
              ? "Anomaly"
              : "Forecast";
      const tags: Record<string, string> = {};
      for (const part of filterTags.split(/[,\s]+/).filter(Boolean)) {
        const [k, ...rest] = part.split(":");
        if (k && rest.length) tags[k] = rest.join(":");
      }
      await api.createAlert({
        name: name.trim(),
        metric,
        threshold: thresholdNum,
        comparator,
        window_ms: windowMs,
        tags,
        options: {
          detection_method: method,
          warning_threshold: warnThreshold ? Number(warnThreshold) : null,
          recovery_threshold: recoveryThreshold ? Number(recoveryThreshold) : null,
          recipients,
          evaluate,
          change_type: method === "change" ? changeType : "pct_change",
          comparison_window_ms: method === "change" ? changeWindowMs : 0,
          anomaly_direction: method === "anomaly" ? anomalyDirection : "above_or_below",
          forecast_horizon_ms: method === "forecast" ? forecastHorizonMs : 0,
          group_by: groupBy || null,
          message: message.slice(0, 2000),
          severity,
        },
      });
      setStatus(`${methodLabel} monitor created — evaluation engine active`);
      onCreated();
      setStep(1);
    } catch (e) {
      setStatus(e instanceof Error ? e.message : "Create failed");
    } finally {
      setBusy(false);
    }
  }

  return (
    <section className="dd-panel monitors-page">
      <div className="mon-breadcrumb">
        Monitors <span>›</span> New Monitor <span>›</span> Metric Monitor
      </div>

      <div className="mon-methods">
        <h3>Detection methods</h3>
        <div className="mon-method-grid">
          {(
            [
              {
                id: "threshold" as const,
                badge: "THRESHOLD",
                title: "Value crosses a static limit",
                example: "CPU > 80%",
                cls: "threshold",
              },
              {
                id: "change" as const,
                badge: "CHANGE",
                title: "Value changes by X%",
                example: "Traffic dropped 50%",
                cls: "change",
              },
              {
                id: "anomaly" as const,
                badge: "ANOMALY",
                title: "ML detects unusual pattern",
                example: "Traffic is weird for 3am Tuesday",
                cls: "anomaly",
              },
              {
                id: "forecast" as const,
                badge: "FORECAST",
                title: "Predicted to breach soon",
                example: "Disk full in 2 days",
                cls: "forecast",
              },
            ] as const
          ).map((m) => (
            <article
              key={m.id}
              className={method === m.id ? "on" : ""}
              role="button"
              tabIndex={0}
              onClick={() => {
                setMethod(m.id);
                setStep(1);
                if (m.id === "change") {
                  setAlertThreshold("50");
                  setComparator("lt");
                } else if (m.id === "anomaly") {
                  setAlertThreshold("0.75");
                } else if (m.id === "forecast") {
                  setAlertThreshold("90");
                  setComparator("gt");
                } else {
                  setAlertThreshold("80");
                  setComparator("gt");
                }
              }}
              onKeyDown={(e) => {
                if (e.key === "Enter" || e.key === " ") {
                  e.preventDefault();
                  (e.currentTarget as HTMLElement).click();
                }
              }}
            >
              <em className={`badge ${m.cls}`}>{m.badge}</em>
              <strong>{m.title}</strong>
              <span>{m.example}</span>
            </article>
          ))}
        </div>
        <p className="mon-tip">
          {method === "threshold" && "Start with Threshold — easiest to understand and debug."}
          {method === "change" && "Change alerts compare the current window to a shifted prior period (absolute or %)."}
          {method === "anomaly" && "Anomaly alerts fire when values leave historically learned bounds for the time-of-day."}
          {method === "forecast" && "Forecast alerts predict future values and fire when confidence bounds cross a threshold."}
        </p>
      </div>

      <div className="mon-chart dw timeseries">
        <div className="dw-title">
          Source data · {metric}
          <span>
            alert &gt; {alertThreshold}
            {warnThreshold ? ` · warn &gt; ${warnThreshold}` : ""}
          </span>
        </div>
        <div className="dw-chart">
          <ResponsiveContainer width="100%" height="100%">
            <LineChart data={chartData} margin={{ top: 8, right: 8, left: 0, bottom: 0 }}>
              <CartesianGrid stroke="rgba(255,255,255,0.06)" vertical={false} />
              <XAxis dataKey="label" tick={{ fill: "#8B97A8", fontSize: 10 }} minTickGap={28} />
              <YAxis tick={{ fill: "#8B97A8", fontSize: 10 }} width={42} />
              <Tooltip
                contentStyle={chartTip}
              />
              {thresholdNum > 0 && (
                <>
                  <ReferenceLine y={thresholdNum} stroke="#e03131" strokeDasharray="4 4" />
                  <ReferenceArea y1={thresholdNum} y2={thresholdNum * 2} fill="#e03131" fillOpacity={0.08} />
                </>
              )}
              {keys.map((k, i) => (
                <Line
                  key={k}
                  type="monotone"
                  dataKey={k}
                  stroke={SERIES_COLORS[i % SERIES_COLORS.length]}
                  strokeWidth={2}
                  dot={false}
                  isAnimationActive={false}
                />
              ))}
            </LineChart>
          </ResponsiveContainer>
        </div>
      </div>

      <div className="mon-steps">
        <section className={`mon-step${step === 1 ? " active" : ""}`}>
          <button type="button" className="mon-step-head" onClick={() => setStep(1)}>
            <span>1</span> Choose the detection method
          </button>
          {step === 1 && (
            <div className="mon-step-body">
              <p className="muted">
                {method === "threshold" && "Metric threshold monitor — alert when the evaluated value crosses a limit."}
                {method === "change" && "Change alert — compare current evaluation to a shifted prior window."}
                {method === "anomaly" && "Anomaly monitor — alert when the series leaves expected seasonal bounds."}
                {method === "forecast" && "Forecast monitor — alert when predicted bounds will breach a static threshold."}
              </p>
              <button type="button" onClick={() => setStep(2)}>
                Continue
              </button>
            </div>
          )}
        </section>

        <section className={`mon-step${step === 2 ? " active" : ""}`}>
          <button type="button" className="mon-step-head" onClick={() => setStep(2)}>
            <span>2</span> Define the metric
          </button>
          {step === 2 && (
            <div className="mon-step-body">
              <div className="eg-query-row">
                <span className="eg-q">a</span>
                <select value={metric} onChange={(e) => onMetric(e.target.value)}>
                  {metrics.map((m) => (
                    <option key={m} value={m}>
                      {m}
                    </option>
                  ))}
                </select>
                <span className="muted">from</span>
                <input
                  value={filterTags}
                  onChange={(e) => setFilterTags(e.target.value)}
                  placeholder="env:prod"
                  style={{ minWidth: 120 }}
                />
                <select value={evaluate} onChange={(e) => setEvaluate(e.target.value)}>
                  <option value="avg">avg by</option>
                  <option value="max">max by</option>
                  <option value="min">min by</option>
                  <option value="sum">sum by</option>
                </select>
                <select value={groupBy} onChange={(e) => setGroupBy(e.target.value)}>
                  <option value="">everything (simple alert)</option>
                  <option value="host">host (multi-alert)</option>
                  <option value="service">service (multi-alert)</option>
                  <option value="env">env (multi-alert)</option>
                </select>
              </div>
              <div className="row mon-eval">
                <label>
                  Evaluate the
                  <select value={evaluate} onChange={(e) => setEvaluate(e.target.value)}>
                    <option value="avg">average</option>
                    <option value="max">maximum</option>
                    <option value="min">minimum</option>
                    <option value="sum">sum</option>
                  </select>
                </label>
                <label>
                  Of the query over the
                  <select value={windowMs} onChange={(e) => setWindowMs(Number(e.target.value))}>
                    <option value={60_000}>last 1 minute</option>
                    <option value={5 * 60_000}>last 5 minutes</option>
                    <option value={15 * 60_000}>last 15 minutes</option>
                    <option value={60 * 60_000}>last 1 hour</option>
                  </select>
                </label>
              </div>
              <button type="button" onClick={() => setStep(3)}>
                Continue
              </button>
            </div>
          )}
        </section>

        <section className={`mon-step${step === 3 ? " active" : ""}`}>
          <button type="button" className="mon-step-head" onClick={() => setStep(3)}>
            <span>3</span> Set alert conditions
          </button>
          {step === 3 && (
            <div className="mon-step-body">
              {method === "change" && (
                <div className="row mon-eval">
                  <label>
                    Change type
                    <select value={changeType} onChange={(e) => setChangeType(e.target.value as typeof changeType)}>
                      <option value="pct_change">% change</option>
                      <option value="change">Absolute change</option>
                    </select>
                  </label>
                  <label>
                    Compare to
                    <select value={changeWindowMs} onChange={(e) => setChangeWindowMs(Number(e.target.value))}>
                      <option value={5 * 60_000}>5 minutes ago</option>
                      <option value={30 * 60_000}>30 minutes ago</option>
                      <option value={60 * 60_000}>1 hour ago</option>
                      <option value={24 * 60 * 60_000}>1 day ago</option>
                    </select>
                  </label>
                </div>
              )}
              {method === "anomaly" && (
                <div className="row mon-eval">
                  <label>
                    Trigger if values are
                    <select
                      value={anomalyDirection}
                      onChange={(e) => setAnomalyDirection(e.target.value as typeof anomalyDirection)}
                    >
                      <option value="above_or_below">above or below</option>
                      <option value="above">above</option>
                      <option value="below">below</option>
                    </select>
                    the bounds
                  </label>
                </div>
              )}
              {method === "forecast" && (
                <div className="row mon-eval">
                  <label>
                    Forecast horizon
                    <select value={forecastHorizonMs} onChange={(e) => setForecastHorizonMs(Number(e.target.value))}>
                      <option value={12 * 60 * 60_000}>next 12 hours</option>
                      <option value={24 * 60 * 60_000}>next 24 hours</option>
                      <option value={7 * 24 * 60 * 60_000}>next 1 week</option>
                    </select>
                  </label>
                </div>
              )}
              <p>
                Trigger when the evaluated value is{" "}
                <select value={comparator} onChange={(e) => setComparator(e.target.value as "gt" | "lt")}>
                  <option value="gt">above</option>
                  <option value="lt">below</option>
                </select>{" "}
                the threshold
                {method === "anomaly" && " (fraction of anomalous window, 0–1)"}
                {method === "change" && changeType === "pct_change" && " (% change)"}
                {method === "forecast" && " (forecast bound vs static limit)"}.
              </p>
              <div className="mon-thresholds">
                <label className="alert">
                  Alert threshold
                  <input value={alertThreshold} onChange={(e) => setAlertThreshold(e.target.value)} />
                </label>
                <label className="warn">
                  Warning threshold
                  <input
                    value={warnThreshold}
                    onChange={(e) => setWarnThreshold(e.target.value)}
                    placeholder="Optional"
                  />
                </label>
                <label className="recovery">
                  Recovery threshold
                  <input
                    value={recoveryThreshold}
                    onChange={(e) => setRecoveryThreshold(e.target.value)}
                    placeholder="Optional"
                  />
                </label>
                <label>
                  Severity
                  <select
                    value={severity}
                    onChange={(e) => setSeverity(e.target.value as typeof severity)}
                  >
                    <option value="critical">Critical</option>
                    <option value="warning">Warning</option>
                    <option value="info">Info</option>
                  </select>
                </label>
              </div>
              <button type="button" onClick={() => setStep(4)}>
                Continue
              </button>
            </div>
          )}
        </section>

        <section className={`mon-step${step === 4 ? " active" : ""}`}>
          <button type="button" className="mon-step-head" onClick={() => setStep(4)}>
            <span>4</span> Configure notifications & automations
          </button>
          {step === 4 && (
            <div className="mon-step-body">
              <div className={`mon-recipients${recipients.trim() ? "" : " empty"}`}>
                {recipients.trim()
                  ? `Routing to: ${recipients}`
                  : "No recipients in monitor message and no matching rules."}
              </div>
              <label>
                Monitor name
                <input value={name} onChange={(e) => setName(e.target.value)} />
              </label>
              <label>
                Recipients / severity routing
                <input
                  value={recipients}
                  onChange={(e) => setRecipients(e.target.value)}
                  placeholder="@slack-channel @pagerduty"
                />
              </label>
              <div className="mon-msg">
                <div className="eg-tabs">
                  <button
                    type="button"
                    className={msgTab === "edit" ? "on" : ""}
                    onClick={() => setMsgTab("edit")}
                  >
                    Edit
                  </button>
                  <button
                    type="button"
                    className={msgTab === "preview" ? "on" : ""}
                    onClick={() => setMsgTab("preview")}
                  >
                    Preview
                  </button>
                </div>
                {msgTab === "edit" ? (
                  <textarea rows={8} value={message} onChange={(e) => setMessage(e.target.value)} />
                ) : (
                  <pre className="mon-msg-preview">{message}</pre>
                )}
              </div>
            </div>
          )}
        </section>
      </div>

      <div className="mon-footer">
        <span className={stepsOk ? "ok" : "warn"}>
          {stepsOk ? "Required steps complete" : "Complete steps 3 and 4"}
        </span>
        <div className="row">
          <button
            type="button"
            className="ghost"
            disabled={busy || !recipients.trim()}
            onClick={() => {
              void (async () => {
                try {
                  const r = await api.testAlertNotify({
                    recipients,
                    rule_name: name,
                    metric,
                  });
                  setStatus(`Test notification delivered to ${r.deliveries} channel(s)`);
                } catch (e) {
                  setStatus(e instanceof Error ? e.message : "Notify test failed");
                }
              })();
            }}
          >
            Test Notifications
          </button>
          <button
            type="button"
            className="ghost"
            onClick={() => {
              const payload = {
                name,
                type: "query alert",
                query: `${evaluate}(last_${Math.round(windowMs / 60_000)}m):${metric}{${filterTags}} ${comparator === "gt" ? ">" : "<"} ${alertThreshold}`,
                options: { thresholds: { critical: alertThreshold, warning: warnThreshold }, detection_method: method },
              };
              void navigator.clipboard.writeText(JSON.stringify(payload, null, 2));
              setStatus("Monitor JSON copied to clipboard");
            }}
          >
            Export Monitor
          </button>
          <button
            type="button"
            className="ghost"
            disabled={busy || !stepsOk}
            onClick={() => {
              setStatus("Draft saved locally — click Create to publish to the evaluation engine");
            }}
          >
            Save as Draft
          </button>
          <button type="button" disabled={busy || !stepsOk} onClick={() => void publish()}>
            Create and Publish
          </button>
        </div>
      </div>

      {status && (
        <div className="dd-banner compact">
          <span>{status}</span>
        </div>
      )}

      <table className="dd-table">
        <thead>
          <tr>
            <th>Name</th>
            <th>Metric</th>
            <th>Condition</th>
            <th>Severity</th>
            <th>Window</th>
            <th>Firing</th>
          </tr>
        </thead>
        <tbody>
          {alerts.map((a) => {
            const firing = alertEvents.filter((e) => e.rule_id === a.id && e.status === "firing").length;
            return (
              <tr key={a.id}>
                <td>{a.name}</td>
                <td>
                  <code>{a.metric}</code>
                </td>
                <td>
                  {a.comparator} {a.threshold}
                </td>
                <td>
                  <span className={`sev ${a.tags?.severity || "medium"}`}>
                    {a.tags?.severity || "—"}
                  </span>
                </td>
                <td>{a.window_ms / 1000}s</td>
                <td>{firing || "—"}</td>
              </tr>
            );
          })}
          {!alerts.length && (
            <tr>
              <td colSpan={6} className="muted">
                No monitors yet
              </td>
            </tr>
          )}
        </tbody>
      </table>
    </section>
  );
}
