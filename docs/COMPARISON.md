# Thine Metrics vs Datadog — Feature Comparison

Scoring rubric (per feature):

| Score | Meaning |
|------:|---------|
| 2 | Parity or better for OSS/self-hosted use |
| 1 | Partial / MVP present |
| 0 | Missing |

Max score for the tracked surface: **40**.

| # | Feature | Datadog | Thine | Score | Notes |
|---|---------|---------|-------|------:|-------|
| 1 | Metric ingest API | Agent + API | JSON `/api/v1/ingest` + `/api/v1/series` | 2 | Drop-in for custom agents/SDKs |
| 2 | OpenTelemetry / OTLP | OTLP intake | OTLP/HTTP JSON `/v1/metrics` | 1 | JSON today; protobuf planned |
| 3 | Language SDKs (Go/Python) | Official DD libs | Thin SDKs + OTEL env wiring | 2 | Any OTEL app can plug in |
| 4 | StatsD ingest | DogStatsD | `/api/v1/ingest/statsd` | 1 | Line protocol subset |
| 5 | Metric explorer UI | Rich | Datadog-style dark explorer + summary/monitors | 2 | Query editor, split graphs, viz toggles |
| 6 | Query + aggregation | Query language | REST query + tag filters + group_by multi-series | 1 | No full QL / formulas yet |
| 7 | Tag filtering | Full | Tag match on query/list | 2 | service/env + arbitrary tags |
| 8 | Dashboards | Drag/drop boards | Screenboard grid: query_value, timeseries, toplist, groups | 2 | `/api/v1/boards/{id}/render` resolves widget data |
| 9 | Alerting | Monitors + notify | Threshold rules + events | 1 | No email/Slack yet |
| 10 | Host / service map | Yes | Service tags only | 0 | Not in v0.1 |
| 11 | APM / traces | Yes | Metrics-only | 0 | Out of scope for v0.1 |
| 12 | Log management | Yes | No | 0 | Metrics focus |
| 13 | Retention controls | Tiered | Env `THINE_RETENTION_MS` | 1 | In-memory window |
| 14 | Multi-tenant / RBAC | Yes | Single-tenant local | 0 | Planned |
| 15 | Self-hosted / OSS | SaaS-first | Apache-2.0 self-host | 2 | Core differentiator |
| 16 | Cost predictability | Usage-based | Your infra | 2 | No per-host tax |
| 17 | Live demo seed data | — | Built-in generator | 2 | Instant local UX |
| 18 | Docker deploy | Agent + SaaS | Single container compose | 2 | `docker compose up` |
| 19 | Health / ops endpoints | Yes | `/health`, `/api/v1/stats` | 2 | Ready for k8s probes |
| 20 | Histogram support | Distributions | OTLP hist → sum/count | 1 | Full buckets TBD |

**Current total: 26 / 40 (65%)**

## Iteration backlog (ordered by score lift)

1. OTLP protobuf + gRPC intake (+1 → feature 2)
2. Alert notification channels (+1 → feature 9)
3. Persist TSDB (Parquet/SQLite) (+1 retention durability)
4. Drag/drop board editor in UI (UX polish; score already 2)
5. PromQL-ish query language (+1 → feature 6)

## Capture → score → fix loop

```bash
./scripts/score_loop.sh
```

This rebuilds, runs unit tests, boots the server, captures API evidence, recomputes the scorefile, and fails the loop if regressions drop below the gate.
