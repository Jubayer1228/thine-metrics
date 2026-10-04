import { useEffect, useMemo, useState, type ReactNode } from "react";
import {
  Area,
  AreaChart,
  Bar,
  BarChart,
  CartesianGrid,
  Legend,
  Line,
  LineChart,
  ResponsiveContainer,
  Tooltip,
  XAxis,
  YAxis,
} from "recharts";
import { api } from "./api";
import type {
  AiDataset,
  AiEvalResult,
  AiExample,
  AiExperiment,
  AiFeedback,
  AiGrader,
  AiHostSample,
  AiMetricDef,
  AiProject,
  AiRun,
} from "./api";
import { fmt } from "./widgets";
import { chartTip } from "./theme";

const tip = { ...chartTip, borderRadius: 8, padding: "10px 12px" };

const COLORS = ["#5B91EB", "#2EC4B6", "#A371E3", "#F4A261", "#F25F5C", "#4CC9F0", "#E9C46A", "#90BE6D"];

function Kpi({ label, value, hint }: { label: string; value: string; hint?: string }) {
  return (
    <div className="obs-kpi">
      <span>{label}</span>
      <strong>{value}</strong>
      {hint && <small>{hint}</small>}
    </div>
  );
}

function ChartCard({
  title,
  subtitle,
  children,
  tall,
}: {
  title: string;
  subtitle?: string;
  children: ReactNode;
  tall?: boolean;
}) {
  return (
    <div className="gpu-chart-card">
      <header>
        <h3>{title}</h3>
        {subtitle && <p>{subtitle}</p>}
      </header>
      <div className={tall ? "gpu-chart tall" : "gpu-chart"}>{children}</div>
    </div>
  );
}

type Props = {
  summary: Record<string, unknown> | null;
  projects: AiProject[];
  runs: AiRun[];
  samples: AiHostSample[];
  experiments: AiExperiment[];
  evals: AiEvalResult[];
  graders: AiGrader[];
  datasets: AiDataset[];
  feedback: AiFeedback[];
  catalog: AiMetricDef[];
  health: { findings?: { project: string; severity: string; finding: string }[] } | null;
  onRefresh?: () => void;
};

export function AiPanel({
  summary,
  projects,
  runs,
  samples,
  experiments,
  evals,
  graders,
  datasets,
  feedback,
  catalog,
  health,
  onRefresh,
}: Props) {
  const [projFilter, setProjFilter] = useState<string>("all");
  const [metricGroup, setMetricGroup] = useState<string>("all");
  const [running, setRunning] = useState(false);
  const [lastEval, setLastEval] = useState<Record<string, unknown> | null>(null);
  const [examples, setExamples] = useState<AiExample[]>([]);
  const [emitting, setEmitting] = useState(false);
  const [emitInfo, setEmitInfo] = useState<string | null>(null);

  useEffect(() => {
    api.aiExamples().then(setExamples).catch(() => setExamples([]));
  }, [datasets]);

  const names = useMemo(() => projects.map((p) => p.name), [projects]);
  const active = projFilter === "all" ? projects : projects.filter((p) => p.name === projFilter);

  const timeSeries = useMemo(() => {
    const byTs = new Map<number, Record<string, number | string>>();
    for (const s of [...samples].sort((a, b) => a.timestamp_ms - b.timestamp_ms)) {
      if (projFilter !== "all" && s.project !== projFilter) continue;
      const row =
        byTs.get(s.timestamp_ms) ??
        ({
          t: new Date(s.timestamp_ms).toLocaleTimeString([], { hour: "2-digit", minute: "2-digit" }),
        } as Record<string, number | string>);
      row[`${s.project}_req`] = s.requests;
      row[`${s.project}_lat`] = s.latency_ms;
      row[`${s.project}_ttft`] = s.ttft_ms;
      row[`${s.project}_cost`] = s.cost_usd;
      row[`${s.project}_pass`] = s.pass_rate * 100;
      byTs.set(s.timestamp_ms, row);
    }
    return Array.from(byTs.entries())
      .sort((a, b) => a[0] - b[0])
      .map(([, v]) => v)
      .slice(-48);
  }, [samples, projFilter]);

  const seriesNames = projFilter === "all" ? names.slice(0, 6) : [projFilter];

  const modalityBars = useMemo(() => {
    const m = new Map<string, { modality: string; runs: number; cost: number }>();
    for (const p of active) {
      const row = m.get(p.modality) ?? { modality: p.modality, runs: 0, cost: 0 };
      row.runs += p.runs_24h;
      row.cost += p.cost_usd_24h;
      m.set(p.modality, row);
    }
    return Array.from(m.values());
  }, [active]);

  const groups = useMemo(() => {
    const g = new Map<string, AiMetricDef[]>();
    for (const m of catalog) {
      if (metricGroup !== "all" && m.group !== metricGroup) continue;
      const list = g.get(m.group) ?? [];
      list.push(m);
      g.set(m.group, list);
    }
    return Array.from(g.entries());
  }, [catalog, metricGroup]);

  const filteredRuns = useMemo(
    () => (projFilter === "all" ? runs : runs.filter((r) => r.project === projFilter)),
    [runs, projFilter],
  );

  const runEval = async () => {
    setRunning(true);
    try {
      const ds = datasets[0]?.id ?? "ds-chinook";
      const r = await api.aiRunExperiment(ds, 5);
      setLastEval(r);
      onRefresh?.();
    } finally {
      setRunning(false);
    }
  };

  return (
    <div className="obs-section gpu-studio">
      <div className="obs-kpi-row">
        <Kpi label="Projects" value={String(summary?.projects ?? projects.length)} hint={(summary?.modalities as string[] | undefined)?.join(" · ")} />
        <Kpi label="Runs / 24h" value={fmt(Number(summary?.runs_24h ?? 0), 0)} />
        <Kpi label="Cost / 24h" value={`$${fmt(Number(summary?.cost_usd_24h ?? 0), 0)}`} />
        <Kpi label="Avg pass" value={`${fmt(Number(summary?.avg_pass_rate ?? 0) * 100, 0)}%`} />
        <Kpi label="Experiments" value={String(summary?.experiments ?? experiments.length)} />
        <Kpi label="Graders" value={String(summary?.graders ?? graders.length)} hint="code · judge · human" />
        <Kpi label="Metrics" value={String(summary?.metrics_catalog_count ?? catalog.length)} hint="ai.* multimodal" />
        <Kpi
          label="Ingest edge"
          value={emitInfo ?? "Rust"}
          hint={emitInfo ? "last emit" : "ring buffer · no GC"}
        />
      </div>
      <div className="dd-toolbar" style={{ marginBottom: "0.65rem" }}>
        <button
          type="button"
          className="ghost"
          disabled={emitting}
          onClick={async () => {
            setEmitting(true);
            try {
              const r = await api.aiEmit();
              setEmitInfo(`${r.points_emitted} pts`);
              onRefresh?.();
            } finally {
              setEmitting(false);
            }
          }}
        >
          {emitting ? "Emitting…" : "Emit AI metrics → store"}
        </button>
        <span className="muted">Push multimodal ai.* samples into the metric engine (LangSmith can’t)</span>
      </div>

      <div className="gpu-device-picker">
        <span>Project</span>
        <button className={projFilter === "all" ? "on" : ""} onClick={() => setProjFilter("all")}>
          All
        </button>
        {projects.map((p) => (
          <button
            key={p.id}
            className={projFilter === p.name ? "on" : ""}
            onClick={() => setProjFilter(p.name)}
          >
            {p.name}
            <small>
              {p.modality} · {p.region}
            </small>
          </button>
        ))}
      </div>

      {(health?.findings?.length ?? 0) > 0 && (
        <div className="obs-card" style={{ marginBottom: "0.75rem" }}>
          <h3>Health findings</h3>
          <ul className="dbm-findings">
            {health!.findings!.map((f, i) => (
              <li key={i}>
                <span className={`sev ${f.severity}`}>{f.severity}</span>
                <code>{f.project}</code> {f.finding}
              </li>
            ))}
          </ul>
        </div>
      )}

      <div className="gpu-chart-grid">
        <ChartCard title="Request rate" subtitle="ai.*.requests by project" tall>
          <ResponsiveContainer width="100%" height="100%">
            <AreaChart data={timeSeries} margin={{ top: 12, right: 16, left: 0, bottom: 8 }}>
              <CartesianGrid stroke="rgba(255,255,255,0.05)" vertical={false} />
              <XAxis dataKey="t" tick={{ fill: "#9AA6B8", fontSize: 10 }} minTickGap={28} axisLine={false} tickLine={false} />
              <YAxis tick={{ fill: "#9AA6B8", fontSize: 11 }} axisLine={false} tickLine={false} width={40} />
              <Tooltip contentStyle={tip} />
              <Legend />
              {seriesNames.map((n, i) => (
                <Area
                  key={n}
                  type="monotone"
                  dataKey={`${n}_req`}
                  name={n}
                  stroke={COLORS[i % COLORS.length]}
                  fill={COLORS[i % COLORS.length]}
                  fillOpacity={0.12}
                  strokeWidth={2}
                  dot={false}
                  isAnimationActive={false}
                />
              ))}
            </AreaChart>
          </ResponsiveContainer>
        </ChartCard>

        <ChartCard title="Latency" subtitle="ms — agent turns + LLM + media" tall>
          <ResponsiveContainer width="100%" height="100%">
            <LineChart data={timeSeries} margin={{ top: 12, right: 16, left: 0, bottom: 8 }}>
              <CartesianGrid stroke="rgba(255,255,255,0.05)" vertical={false} />
              <XAxis dataKey="t" tick={{ fill: "#9AA6B8", fontSize: 10 }} minTickGap={28} axisLine={false} tickLine={false} />
              <YAxis tick={{ fill: "#9AA6B8", fontSize: 11 }} axisLine={false} tickLine={false} width={48} />
              <Tooltip contentStyle={tip} />
              <Legend />
              {seriesNames.map((n, i) => (
                <Line
                  key={n}
                  type="monotone"
                  dataKey={`${n}_lat`}
                  name={n}
                  stroke={COLORS[i % COLORS.length]}
                  strokeWidth={2}
                  dot={false}
                  isAnimationActive={false}
                />
              ))}
            </LineChart>
          </ResponsiveContainer>
        </ChartCard>

        <ChartCard title="TTFT" subtitle="Time to first token / partial (streaming)">
          <ResponsiveContainer width="100%" height="100%">
            <LineChart data={timeSeries} margin={{ top: 12, right: 16, left: 0, bottom: 8 }}>
              <CartesianGrid stroke="rgba(255,255,255,0.05)" vertical={false} />
              <XAxis dataKey="t" tick={{ fill: "#9AA6B8", fontSize: 10 }} minTickGap={28} axisLine={false} tickLine={false} />
              <YAxis tick={{ fill: "#9AA6B8", fontSize: 11 }} axisLine={false} tickLine={false} width={40} />
              <Tooltip contentStyle={tip} />
              {seriesNames.map((n, i) => (
                <Line
                  key={n}
                  type="monotone"
                  dataKey={`${n}_ttft`}
                  name={n}
                  stroke={COLORS[i % COLORS.length]}
                  strokeWidth={2}
                  dot={false}
                  isAnimationActive={false}
                />
              ))}
            </LineChart>
          </ResponsiveContainer>
        </ChartCard>

        <ChartCard title="Pass rate" subtitle="Online eval / regression bar">
          <ResponsiveContainer width="100%" height="100%">
            <LineChart data={timeSeries} margin={{ top: 12, right: 16, left: 0, bottom: 8 }}>
              <CartesianGrid stroke="rgba(255,255,255,0.05)" vertical={false} />
              <XAxis dataKey="t" tick={{ fill: "#9AA6B8", fontSize: 10 }} minTickGap={28} axisLine={false} tickLine={false} />
              <YAxis domain={[70, 100]} tick={{ fill: "#9AA6B8", fontSize: 11 }} axisLine={false} tickLine={false} width={36} unit="%" />
              <Tooltip contentStyle={tip} />
              {seriesNames.map((n, i) => (
                <Line
                  key={n}
                  type="monotone"
                  dataKey={`${n}_pass`}
                  name={n}
                  stroke={COLORS[i % COLORS.length]}
                  strokeWidth={2}
                  dot={false}
                  isAnimationActive={false}
                />
              ))}
            </LineChart>
          </ResponsiveContainer>
        </ChartCard>

        <ChartCard title="Volume by modality" subtitle="Runs / 24h — LLM · agent · STT · TTS · image · video · physical">
          <ResponsiveContainer width="100%" height="100%">
            <BarChart data={modalityBars} margin={{ top: 12, right: 8, left: 0, bottom: 8 }}>
              <CartesianGrid stroke="rgba(255,255,255,0.05)" vertical={false} />
              <XAxis dataKey="modality" tick={{ fill: "#9AA6B8", fontSize: 10 }} axisLine={false} tickLine={false} />
              <YAxis tick={{ fill: "#9AA6B8", fontSize: 11 }} axisLine={false} tickLine={false} width={48} />
              <Tooltip contentStyle={tip} />
              <Bar dataKey="runs" fill="#5B91EB" name="Runs" radius={[3, 3, 0, 0]} />
            </BarChart>
          </ResponsiveContainer>
        </ChartCard>

        <ChartCard title="Cost by modality" subtitle="$ / 24h — token + audio-min + image + GPU-hour">
          <ResponsiveContainer width="100%" height="100%">
            <BarChart data={modalityBars} margin={{ top: 12, right: 8, left: 0, bottom: 8 }}>
              <CartesianGrid stroke="rgba(255,255,255,0.05)" vertical={false} />
              <XAxis dataKey="modality" tick={{ fill: "#9AA6B8", fontSize: 10 }} axisLine={false} tickLine={false} />
              <YAxis tick={{ fill: "#9AA6B8", fontSize: 11 }} axisLine={false} tickLine={false} width={40} />
              <Tooltip contentStyle={tip} />
              <Bar dataKey="cost" fill="#F4A261" name="$" radius={[3, 3, 0, 0]} />
            </BarChart>
          </ResponsiveContainer>
        </ChartCard>
      </div>

      <div className="obs-card" style={{ marginTop: "0.75rem" }}>
        <h3>Projects</h3>
        <p className="muted">LangSmith-style apps — multimodal · region · provider · pass rate</p>
        <table className="dd-table compact">
          <thead>
            <tr>
              <th>Name</th>
              <th>Modality</th>
              <th>Provider / Model</th>
              <th>Region</th>
              <th>Runs</th>
              <th>Err%</th>
              <th>Avg / p95</th>
              <th>Tok/s</th>
              <th>Cost</th>
              <th>Pass</th>
            </tr>
          </thead>
          <tbody>
            {active.map((p) => (
              <tr key={p.id}>
                <td>
                  <code>{p.name}</code>
                  <div className="muted">{p.description}</div>
                </td>
                <td>{p.modality}</td>
                <td>
                  {p.provider}
                  <div className="muted">{p.model}</div>
                </td>
                <td>{p.region}</td>
                <td>{p.runs_24h.toLocaleString()}</td>
                <td>{fmt(p.error_rate * 100, 1)}%</td>
                <td>
                  {fmt(p.avg_latency_ms, 0)} / {fmt(p.p95_latency_ms, 0)}ms
                </td>
                <td>{p.tokens_per_sec ? fmt(p.tokens_per_sec, 0) : "—"}</td>
                <td>${fmt(p.cost_usd_24h, 0)}</td>
                <td>{fmt(p.pass_rate * 100, 0)}%</td>
              </tr>
            ))}
          </tbody>
        </table>
      </div>

      {examples.length > 0 && (
        <div className="obs-card" style={{ marginTop: "0.75rem" }}>
          <h3>Dataset examples</h3>
          <p className="muted">Ground-truth rows for offline evals — open alongside APM traces for full-stack RCA</p>
          <table className="dd-table compact">
            <thead>
              <tr>
                <th>ID</th>
                <th>Dataset</th>
                <th>Inputs</th>
                <th>Outputs</th>
              </tr>
            </thead>
            <tbody>
              {examples.slice(0, 12).map((ex) => (
                <tr key={ex.id}>
                  <td>
                    <code>{ex.id}</code>
                  </td>
                  <td>{ex.dataset_id}</td>
                  <td className="tags">{JSON.stringify(ex.inputs).slice(0, 80)}</td>
                  <td className="tags">{JSON.stringify(ex.outputs).slice(0, 80)}</td>
                </tr>
              ))}
            </tbody>
          </table>
        </div>
      )}

      <div className="obs-card" style={{ marginTop: "0.75rem" }}>
        <div className="dd-toolbar">
          <h3 style={{ margin: 0 }}>Deep-agent evaluations</h3>
          <button className="dd-btn" disabled={running} onClick={runEval}>
            {running ? "Running…" : "Run offline eval (pass@k)"}
          </button>
        </div>
        <p className="muted">
          Patterns from{" "}
          <a href="https://aws.amazon.com/blogs/machine-learning/evaluating-deep-agents-using-langsmith-on-aws/">
            LangSmith on AWS
          </a>
          : custom per-datapoint · single-step · full turn · multi-turn · code + LLM-as-judge + human
        </p>
        {lastEval && (
          <p className="muted">
            Last: pass@k={fmt(Number(lastEval.pass_at_k) * 100, 0)}% · pass^k=
            {fmt(Number(lastEval.pass_hat_k) * 100, 0)}% · {String(lastEval.elapsed_us)}µs harness
          </p>
        )}
        <table className="dd-table compact">
          <thead>
            <tr>
              <th>Experiment</th>
              <th>Dataset</th>
              <th>Trials</th>
              <th>pass@k</th>
              <th>pass^k</th>
              <th>Avg score</th>
              <th>Status</th>
            </tr>
          </thead>
          <tbody>
            {experiments.map((e) => (
              <tr key={e.id}>
                <td>
                  <code>{e.name}</code>
                </td>
                <td>{e.dataset_id}</td>
                <td>{e.trials_per_task}</td>
                <td>{fmt(e.pass_at_k * 100, 0)}%</td>
                <td>{fmt(e.pass_hat_k * 100, 0)}%</td>
                <td>{fmt(e.avg_score, 2)}</td>
                <td>{e.status}</td>
              </tr>
            ))}
          </tbody>
        </table>
        <table className="dd-table compact" style={{ marginTop: "0.75rem" }}>
          <thead>
            <tr>
              <th>Example</th>
              <th>Pattern</th>
              <th>Grader</th>
              <th>Trial</th>
              <th>Score</th>
              <th>Pass</th>
              <th>Detail</th>
            </tr>
          </thead>
          <tbody>
            {evals.slice(0, 20).map((e) => (
              <tr key={e.id}>
                <td>
                  <code>{e.example_id}</code>
                </td>
                <td>{e.pattern}</td>
                <td>{e.grader}</td>
                <td>{e.trial}</td>
                <td>{fmt(e.score, 2)}</td>
                <td>{e.passed ? "yes" : "no"}</td>
                <td className="muted">{e.detail}</td>
              </tr>
            ))}
          </tbody>
        </table>
      </div>

      <div className="obs-grid-2" style={{ marginTop: "0.75rem" }}>
        <div className="obs-card">
          <h3>Graders</h3>
          <table className="dd-table compact">
            <thead>
              <tr>
                <th>Name</th>
                <th>Kind</th>
                <th>Description</th>
              </tr>
            </thead>
            <tbody>
              {graders.map((g) => (
                <tr key={g.id}>
                  <td>
                    <code>{g.name}</code>
                  </td>
                  <td>{g.kind}</td>
                  <td className="muted">{g.description}</td>
                </tr>
              ))}
            </tbody>
          </table>
        </div>
        <div className="obs-card">
          <h3>Datasets</h3>
          <table className="dd-table compact">
            <thead>
              <tr>
                <th>Name</th>
                <th>Examples</th>
                <th>Modality</th>
              </tr>
            </thead>
            <tbody>
              {datasets.map((d) => (
                <tr key={d.id}>
                  <td>
                    <code>{d.name}</code>
                    <div className="muted">{d.description}</div>
                  </td>
                  <td>{d.example_count}</td>
                  <td>{d.modality}</td>
                </tr>
              ))}
            </tbody>
          </table>
        </div>
      </div>

      <div className="obs-card" style={{ marginTop: "0.75rem" }}>
        <h3>Runs / traces</h3>
        <p className="muted">LangSmith run types + multimodal — trajectory for deep agents</p>
        <table className="dd-table">
          <thead>
            <tr>
              <th>Name</th>
              <th>Type</th>
              <th>Project</th>
              <th>Model</th>
              <th>Latency</th>
              <th>TTFT</th>
              <th>Tokens</th>
              <th>Cost</th>
              <th>Trajectory</th>
              <th>I/O</th>
            </tr>
          </thead>
          <tbody>
            {filteredRuns.slice(0, 40).map((r) => (
              <tr key={r.id}>
                <td>
                  <code>{r.name}</code>
                  <div className="muted">{r.status}</div>
                </td>
                <td>{r.run_type}</td>
                <td>{r.project}</td>
                <td className="muted">{r.model || "—"}</td>
                <td>{fmt(r.latency_ms, 0)}ms</td>
                <td>{r.ttft_ms != null ? `${fmt(r.ttft_ms, 0)}ms` : "—"}</td>
                <td>
                  {r.input_tokens}/{r.output_tokens}
                  {r.cache_read_tokens ? (
                    <div className="muted">cache {r.cache_read_tokens}</div>
                  ) : null}
                </td>
                <td>${fmt(r.cost_usd, 4)}</td>
                <td className="tags">{r.trajectory.length ? r.trajectory.join(" → ") : "—"}</td>
                <td className="sql-cell muted">
                  {r.input_preview}
                  {r.output_preview ? ` → ${r.output_preview}` : ""}
                </td>
              </tr>
            ))}
          </tbody>
        </table>
      </div>

      <div className="obs-card" style={{ marginTop: "0.75rem" }}>
        <h3>Feedback</h3>
        <table className="dd-table compact">
          <thead>
            <tr>
              <th>Run</th>
              <th>Key</th>
              <th>Score</th>
              <th>Source</th>
              <th>Comment</th>
            </tr>
          </thead>
          <tbody>
            {feedback.map((f) => (
              <tr key={f.id}>
                <td>
                  <code>{f.run_id}</code>
                </td>
                <td>{f.key}</td>
                <td>{fmt(f.score, 2)}</td>
                <td>{f.source}</td>
                <td className="muted">{f.comment || "—"}</td>
              </tr>
            ))}
          </tbody>
        </table>
      </div>

      <div className="gpu-chart-card" style={{ marginTop: "0.75rem" }}>
        <header>
          <h3>All AI metrics ({catalog.length})</h3>
          <p>OTel gen_ai + LangSmith ml_obs parity + speech / image / video / physical / sim</p>
        </header>
        <div className="dd-toolbar" style={{ marginBottom: "0.5rem" }}>
          <select value={metricGroup} onChange={(e) => setMetricGroup(e.target.value)}>
            <option value="all">All groups</option>
            {[...new Set(catalog.map((m) => m.group))].map((g) => (
              <option key={g} value={g}>
                {g}
              </option>
            ))}
          </select>
        </div>
        {groups.map(([group, metrics]) => (
          <div key={group} className="gpu-metric-group">
            <h4>{group}</h4>
            <table className="dd-table compact">
              <thead>
                <tr>
                  <th>Metric</th>
                  <th>Unit</th>
                  <th>Modalities</th>
                  <th>Description</th>
                </tr>
              </thead>
              <tbody>
                {metrics.map((m) => (
                  <tr key={m.name}>
                    <td>
                      <code>{m.name}</code>
                    </td>
                    <td>{m.unit}</td>
                    <td>{m.modalities.join(", ")}</td>
                    <td className="muted">{m.description}</td>
                  </tr>
                ))}
              </tbody>
            </table>
          </div>
        ))}
      </div>
    </div>
  );
}
