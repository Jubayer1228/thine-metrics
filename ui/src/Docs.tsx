import { useEffect, useMemo, useState } from "react";
import { api, type FeatureEntry } from "./api";

const HOW_TO: Record<
  string,
  { summary: string; steps: string[]; curl: string; effectiveness: string }
> = {
  dashboards: {
    summary:
      "Datadog-style dashboards plus Graph Insights (correlations, Watchdog Explains, anomaly issues) and Recently Deleted restore.",
    steps: [
      "Open Dashboards → Investigate for co-occurring anomalies; Correlations / Watchdog Explains tabs.",
      "On a timeseries widget click Correlations, or use Dashboard List → Recently Deleted → Restore to.",
      "Template variables ($env, $service, $gpu_id) filter all widgets; share / clipboard still available.",
    ],
    curl: `curl -s localhost:4318/api/v1/graph_insights/guide | jq .
ID=$(curl -s localhost:4318/api/v1/boards | jq -r '.[]|select(.name=="Service Overview")|.id')
curl -s "localhost:4318/api/v1/boards/$ID/anomalies?range_ms=3600000" | jq '{issues:(.issues|length),scanned:.scanned_widgets}'
curl -s -X POST localhost:4318/api/v1/graph_insights/correlations -H 'content-type: application/json' -d '{"metric":"http.server.duration","start_ms":0,"end_ms":0}' | jq '.results|length'
curl -s localhost:4318/api/v1/boards/deleted | jq '.[].name'`,
    effectiveness:
      "Matches https://docs.datadoghq.com/dashboards/graph_insights/ — Investigate issues, pink anomaly chips, correlations by source, 30-day soft-delete restore.",
  },
  cloudcraft: {
    summary: "Map AWS/GCP/Azure resources with Group By and overlays (cost, observability, monitors).",
    steps: [
      "Open Observability → Cloudcraft.",
      "Pick an overlay (cost / observability / monitors) and group_by (service, kind, region).",
      "Use GET /api/v1/cloudcraft/diagram to embed diagrams in automation.",
    ],
    curl: `curl -s 'localhost:4318/api/v1/cloudcraft/diagram?overlay=cost&group_by=kind' | jq '.resource_count,.groups'`,
    effectiveness: "Surfaces monthly spend per resource and alert status so you can rightsize before paying SaaS CCM.",
  },
  usm: {
    summary: "Discover services and emit RED metrics from eBPF/network flows — no instrumentation.",
    steps: [
      "Ingest network flows (seeded demo or agent).",
      "Call POST /api/v1/usm/discover or read GET /api/v1/usm/red.",
      "Compare hits/errors/duration against APM for uninstrumented services.",
    ],
    curl: `curl -s localhost:4318/api/v1/usm/red | jq '.[0:3]'`,
    effectiveness: "Measures RPS and p95 for every discovered protocol edge without deploying SDKs.",
  },
  apm: {
    summary: "Trace latency percentiles and error rates per service.",
    steps: [
      "Send OTLP spans or use seeded demos.",
      "Inspect Observability → APM for p50/p95/p99.",
      "Drill into /api/v1/apm/traces/{id} for a single request.",
    ],
    curl: `curl -s localhost:4318/api/v1/apm/stats | jq .`,
    effectiveness: "Shows which service owns latency — checkout vs postgres — with live error counts.",
  },
  opentelemetry: {
    summary:
      "Full Datadog dual-path: native (HTTP + DogStatsD UDP:8125) and OTel OTLP converge → normalize → intake bus → RTDB+tag index / TraceStore / Husky fragments under THINE_DATA_DIR.",
    steps: [
      "Native: POST /api/v1/ingest, /api/v1/ingest/statsd, UDP :8125, /api/v1/apm/traces, /api/v1/logs/ingest, /api/v1/events.",
      "OTel JSON: POST /v1/metrics|/v1/traces|/v1/logs (protobuf → terminate at Collector → JSON).",
      "Durable engines: data/rtdb (WAL+index), data/traces, data/husky/fragments + catalog.",
      "GET /api/v1/ingest/architecture for counters + durable stats.",
    ],
    curl: `curl -s localhost:4318/api/v1/ingest/architecture | jq .
echo 'orders.created:1|c|#env:prod,service:api' | nc -u -w1 127.0.0.1 8125
curl -s -X POST localhost:4318/v1/traces -H 'content-type: application/json' -d '{"resourceSpans":[{"resource":{"attributes":[{"key":"service.name","value":{"stringValue":"demo"}}]},"scopeSpans":[{"spans":[{"traceId":"t1","spanId":"s1","name":"GET /","startTimeUnixNano":"1700000000000000000","endTimeUnixNano":"1700000000050000000","status":{"code":1}}]}]}]}' | jq .
ls data/rtdb data/husky data/traces 2>/dev/null | head`,
    effectiveness:
      "Mirrors Datadog: dual on-ramps, shared intake, signal-specific stores (RTDB/Husky/traces), always-on APM stats, UDP DogStatsD.",
  },
  database_monitoring: {
    summary:
      "Datadog-style DBM: Postgres/MySQL host metrics, query metrics, samples, EXPLAIN, waits, blocking, schema, APM correlation.",
    steps: [
      "Open Observability → Database for QPS/latency/lag charts and instance KPIs.",
      "Inspect Query Metrics (avg/p95/% time/blocks), EXPLAIN plans, wait events, and live activity.",
      "Browse the full postgresql.*/mysql.* metrics catalog; emit via POST /api/v1/dbm/emit.",
    ],
    curl: `curl -s localhost:4318/api/v1/dbm/summary | jq .
curl -s localhost:4318/api/v1/dbm/query_metrics | jq '.[0:3] | .[] | {fingerprint,avg_latency_ms,pct_time}'
curl -s localhost:4318/api/v1/dbm/metrics | jq '{count,groups}'`,
    effectiveness:
      "Surfaces Hash Join cost 1840, replication lag, buffer hit %, and lock wait events in one surface — same depth as GPU panel.",
  },
  infra_monitoring: {
    summary: "Hosts, live processes, and containers for capacity and noisy-neighbor detection.",
    steps: [
      "Open Observability → Infra & Processes.",
      "Sort by CPU% to find runaway workers.",
      "Correlate host with Cloudcraft resource id.",
    ],
    curl: `curl -s 'localhost:4318/api/v1/infra/processes' | jq '.[0:5]'`,
    effectiveness: "Highlights worker at ~60% CPU vs agents at ~4% — actionable process-level insight.",
  },
  cloud_cost: {
    summary: "Cost detail by service plus rightsizing / lifecycle recommendations.",
    steps: [
      "Open Observability → Cloud Cost.",
      "Apply recommendations via change tickets (EC2 rightsize, S3 lifecycle).",
      "Track savings_month rollup as the effectiveness KPI.",
    ],
    curl: `curl -s localhost:4318/api/v1/cost/recommendations | jq '[.[] | {title,savings_month}]'`,
    effectiveness: "Aggregates $/mo salvage across underutilized EC2 and cold S3 buckets.",
  },
  data_streams: {
    summary: "Kafka/stream lag and throughput for backlog SLOs.",
    steps: ["Watch Observability → Streams & Lineage.", "Alert when lag exceeds threshold.", "Pair with lineage edges to find producers."],
    curl: `curl -s localhost:4318/api/v1/streams | jq .`,
    effectiveness: "orders lag 128 vs notifications lag 3 shows which topic needs consumers.",
  },
  data_observability: {
    summary: "Asset lineage and freshness anomalies across facts and dashboards.",
    steps: ["Inspect lineage graph edges.", "Investigate freshness_anomalies > 0.", "Wire monitors on stale assets."],
    curl: `curl -s localhost:4318/api/v1/data/lineage | jq '{nodes,freshness_anomalies,edges}'`,
    effectiveness: "Traces kafka:orders → orders_fact → revenue_dashboard for blast-radius analysis.",
  },
  log_management: {
    summary: "Search, facets, and SDS scanning over ingested logs.",
    steps: ["Open Observability → Logs & Errors.", "Filter level=error.", "Review SDS hits for leaked emails/PII."],
    curl: `curl -s 'localhost:4318/api/v1/logs/search?limit=10' | jq '.[].message'`,
    effectiveness: "Error rate in the sample plus SDS matches prove scanner + search effectiveness.",
  },
  multi_region_ha: {
    summary: "Active/active regions with quorum, RPO/RTO, and failover readiness.",
    steps: ["Open Observability → HA / Watchdog.", "Confirm quorum and regional lag.", "Use /api/v1/ha/status in runbooks."],
    curl: `curl -s localhost:4318/api/v1/ha/status | jq '{mode,primary,quorum,rpo_seconds,rto_seconds}'`,
    effectiveness: "Shows replica lag (ms) per region so you know failover safety before promoting.",
  },
  watchdog_ml: {
    summary: "Anomaly scan over metrics/services without external ML SaaS.",
    steps: ["Click Run Watchdog scan on the HA tab.", "Read /api/v1/watchdog/summary.", "Open incidents from anomalies."],
    curl: `curl -s -X POST localhost:4318/api/v1/watchdog/scan -H 'content-type: application/json' -d '{}' | jq .`,
    effectiveness: "Surfaces anomaly counts and affected services from local heuristics.",
  },
  obs_pipelines: {
    summary: "Pipeline workers and destinations for log/metric routing.",
    steps: ["Check GET /api/v1/pipelines/workers.", "Route to thine_logs / s3_archive.", "Scale workers by events_per_sec."],
    curl: `curl -s localhost:4318/api/v1/pipelines/workers | jq .`,
    effectiveness: "Reports multi-region worker EPS so you can size ingest capacity.",
  },
  gpu_monitoring: {
    summary: "Granular NVIDIA GPU telemetry (DCGM/Datadog-parity) — SM, memory, power, pipes, health, processes.",
    steps: [
      "Open Observability → GPU for fleet KPIs and the full metric catalog.",
      "Query Explorer for gpu.sm_active, gpu.power.usage, gpu.process.memory.usage.",
      "POST /api/v1/gpu/emit to refresh gauge/counter points into the metric store.",
      "Review /api/v1/gpu/health for Xid, ECC, throttle, and zombie allocations.",
    ],
    curl: `curl -s localhost:4318/api/v1/gpu/metrics | jq '.count,.metrics[:5]'
curl -s localhost:4318/api/v1/gpu/summary | jq '{devices,avg_sm_active,total_power_w,unhealthy_devices}'`,
    effectiveness: "Surfaces H100 power-cap throttle, L4 thermal/Xid degradation, and A10G zombie memory (high mem / low SM).",
  },
  ai_observability: {
    summary:
      "LangSmith / OTel GenAI parity — multimodal runs, deep-agent evals (pass@k / pass^k), code + LLM-as-judge graders.",
    steps: [
      "Open Observability → AI / Agents for projects, traces, evals, and the ai.* metric catalog.",
      "POST /api/v1/ai/experiments/run with dataset_id=ds-chinook for offline harness (single-step / full / multi-turn).",
      "Batch-ingest runs via POST /api/v1/ai/runs/batch — Rust ring buffer, µs-level elapsed reported.",
      "POST /api/v1/ai/emit to push gauges into the metric store; correlate with GPU panel for serving.",
    ],
    curl: `curl -s localhost:4318/api/v1/ai/summary | jq '{projects,runs_24h,cost_usd_24h,metrics_catalog_count,performance}'
curl -s localhost:4318/api/v1/ai/experiments/run -H 'content-type: application/json' -d '{"dataset_id":"ds-chinook","trials":5}' | jq '{pass_at_k,pass_hat_k,elapsed_us}'`,
    effectiveness:
      "Catches trajectory regressions (schema-first tools), answer correctness, and multimodal cost spikes before prod.",
  },
  continuous_profiler: {
    summary: "CPU/heap profiles with flame summary frames.",
    steps: ["Upload profile blobs via profiler APIs.", "Read /api/v1/profiler/flame/{id}.", "Focus top_frames pct."],
    curl: `curl -s localhost:4318/api/v1/profiler/profiles | jq .`,
    effectiveness: "Top frames (runtime.main, SQL, JSON) quantify where CPU is spent.",
  },
  container_monitoring: {
    summary: "Container inventory and resource stats.",
    steps: ["Observability → Infra shows containers.", "Use /containers/stats for CPU/mem.", "Link image tags to deploys."],
    curl: `curl -s localhost:4318/api/v1/containers | jq 'length'`,
    effectiveness: "Counts running containers per host for density checks.",
  },
  error_tracking: {
    summary: "Group repeated exceptions across services.",
    steps: ["Logs & Errors tab → Error groups.", "Triage by count.", "Link to APM error_rate."],
    curl: `curl -s localhost:4318/api/v1/errors | jq .`,
    effectiveness: "Rolls up duplicate messages so MTTR focuses on high-count groups.",
  },
  sensitive_data_scanner: {
    summary: "Detect PII/secrets in log lines.",
    steps: ["Ensure SDS rules loaded.", "GET /api/v1/sds/scan.", "Redact or drop in pipelines."],
    curl: `curl -s localhost:4318/api/v1/sds/scan | jq .`,
    effectiveness: "Counts rule hits (emails in warn logs) as compliance evidence.",
  },
  serverless: {
    summary: "Lambda/function inventory and invocations.",
    steps: ["List /api/v1/serverless/functions.", "Correlate with Cloudcraft lambda nodes.", "Watch cold starts via APM."],
    curl: `curl -s localhost:4318/api/v1/serverless/functions | jq .`,
    effectiveness: "Ties function cost_month on Cloudcraft to invocation health.",
  },
  network_monitoring: {
    summary: "Service-to-service flows for dependency maps.",
    steps: ["Flows feed USM + Cloudcraft edges.", "Inspect bytes between api→postgres.", "Find chatty pairs."],
    curl: `curl -s localhost:4318/api/v1/network/flows | jq '.[0:5]'`,
    effectiveness: "Byte volume per edge proves which dependencies dominate bandwidth.",
  },
};

const FALLBACK_KEYS = Object.keys(HOW_TO);

function matchHowTo(f: FeatureEntry) {
  const id = f.id.toLowerCase();
  if (HOW_TO[id]) return HOW_TO[id];
  const hit = FALLBACK_KEYS.find(
    (k) => id.includes(k) || k.includes(id) || f.name.toLowerCase().includes(k.replace(/_/g, " ")),
  );
  return hit ? HOW_TO[hit] : null;
}

export function DocsPage() {
  const [features, setFeatures] = useState<FeatureEntry[]>([]);
  const [stats, setStats] = useState<{
    total: number;
    done: number;
    avg_parity_pct: number;
  } | null>(null);
  const [q, setQ] = useState("");
  const [category, setCategory] = useState("all");
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    Promise.all([api.features(), api.featuresStats()])
      .then(([f, s]) => {
        setFeatures(f);
        setStats(s);
        setError(null);
      })
      .catch((e) => setError(e instanceof Error ? e.message : "Failed to load docs"));
  }, []);

  const categories = useMemo(
    () => Array.from(new Set(features.map((f) => f.category))).sort(),
    [features],
  );

  const filtered = useMemo(() => {
    const qq = q.toLowerCase();
    return features.filter((f) => {
      if (category !== "all" && f.category !== category) return false;
      if (!qq) return true;
      return (
        f.name.toLowerCase().includes(qq) ||
        f.id.toLowerCase().includes(qq) ||
        f.notes.toLowerCase().includes(qq) ||
        f.endpoints.some((e) => e.toLowerCase().includes(qq))
      );
    });
  }, [features, q, category]);

  return (
    <section className="dd-panel docs-page">
      <div className="docs-hero">
        <div>
          <h2>Feature docs & usage</h2>
          <p>
            How to exercise each Thine surface against the live API. Open{" "}
            <strong>Observability</strong> to see the same data visualized.
          </p>
        </div>
        <div className="obs-kpi-row docs-stats">
          <div className="obs-kpi">
            <span>Catalog</span>
            <strong>{stats ? `${stats.done}/${stats.total}` : "—"}</strong>
            <small>done</small>
          </div>
          <div className="obs-kpi">
            <span>Parity</span>
            <strong>{stats ? `${stats.avg_parity_pct.toFixed(0)}%` : "—"}</strong>
            <small>avg vs Datadog</small>
          </div>
          <div className="obs-kpi">
            <span>Guides</span>
            <strong>{Object.keys(HOW_TO).length}</strong>
            <small>how-to writeups</small>
          </div>
        </div>
      </div>

      {error && (
        <div className="dd-banner">
          <strong>Docs load failed</strong>
          <span>{error}</span>
        </div>
      )}

      <div className="docs-toolbar">
        <input placeholder="Search features, endpoints…" value={q} onChange={(e) => setQ(e.target.value)} />
        <select value={category} onChange={(e) => setCategory(e.target.value)}>
          <option value="all">All categories</option>
          {categories.map((c) => (
            <option key={c} value={c}>
              {c}
            </option>
          ))}
        </select>
        <span className="muted">{filtered.length} features</span>
      </div>

      <div className="docs-quick">
        <h3>Quick start</h3>
        <ol>
          <li>
            Start the stack: <code>./scripts/dev.sh</code> (UI + API on :4318)
          </li>
          <li>
            Confirm catalog: <code>curl -s localhost:4318/api/v1/features/stats</code>
          </li>
          <li>
            Open <strong>Observability</strong> for Cloudcraft / USM / APM / DBM / Cost / HA demos
          </li>
          <li>
            Smoke test: <code>./scripts/test_features.sh</code>
          </li>
        </ol>
      </div>

      <div className="docs-list">
        {filtered.map((f) => {
          const guide = matchHowTo(f);
          return (
            <article key={f.id} className="docs-card">
              <header>
                <div>
                  <h3>{f.name}</h3>
                  <code>{f.id}</code>
                </div>
                <div className="docs-badges">
                  <span className="pill ok">{f.status}</span>
                  <span className="pill muted-pill">{f.category}</span>
                  <span className="pill accent">{f.parity_pct}% parity</span>
                </div>
              </header>
              <p>{f.notes}</p>
              {guide && (
                <div className="docs-guide">
                  <p>
                    <strong>How to use:</strong> {guide.summary}
                  </p>
                  <ol>
                    {guide.steps.map((s) => (
                      <li key={s}>{s}</li>
                    ))}
                  </ol>
                  <p>
                    <strong>Effectiveness:</strong> {guide.effectiveness}
                  </p>
                  <pre>{guide.curl}</pre>
                </div>
              )}
              <div className="docs-meta">
                <div>
                  <span>Endpoints</span>
                  <ul>
                    {f.endpoints.map((e) => (
                      <li key={e}>
                        <code>{e}</code>
                      </li>
                    ))}
                  </ul>
                </div>
                <div>
                  <span>Thine strength</span>
                  <p>{f.thine_strength}</p>
                  <span>Datadog edge</span>
                  <p>{f.datadog_advantage}</p>
                  <span>Recommendation</span>
                  <p>{f.recommendation}</p>
                </div>
              </div>
            </article>
          );
        })}
      </div>
    </section>
  );
}
