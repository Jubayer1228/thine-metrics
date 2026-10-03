# Thine Metrics

**Self-hosted metrics observability — Datadog-class charts, OpenTelemetry intake, Rust core.**

Thine is an open-source metrics platform you run yourself. Point OpenTelemetry (or the Go/Python helpers) at it, explore series in a modern UI, and alert on thresholds — without a per-host SaaS bill.

![Thine](https://img.shields.io/badge/otlp-ready-0F766E) ![Rust](https://img.shields.io/badge/rust-axum-142033) ![License](https://img.shields.io/badge/license-Apache%202.0-informational)

## Why Thine

| | Datadog | Thine |
|---|---|---|
| Metrics explorer | SaaS | Self-hosted, modern UI |
| OpenTelemetry | Supported | Native OTLP/HTTP JSON (`/v1/metrics`) |
| Go / Python | DD libraries | OTEL + thin SDKs in-repo |
| Cost model | Usage-based | Your infrastructure |
| Stack | Proprietary | Rust + React, Apache-2.0 |

Feature-by-feature scorecard: [`docs/COMPARISON.md`](docs/COMPARISON.md) (currently **26/40**).

## Quick start

```bash
# prerequisites: Rust 1.89+, Node 20+, curl
./scripts/dev.sh
# open http://localhost:4318  ← Metrics Explorer (Datadog-style UI)
```

If the UI shows **Connection failed**, the API isn’t running — use the command above (not a static `file://` open).

Or with Docker:

```bash
docker compose up --build
```

### UI (Datadog dashboard + explorer parity)

- **Dashboards** — 12-column screenboard: query value KPIs (with % change + sparkline), timeseries, toplists, section groups
- **Metrics Explorer** — `avg:metric{tags} by {tag}`, line/area/bars, split graphs
- **Metrics Summary** — granular min/avg/max/last + sparkline per metric
- **Monitors** — threshold rules with live preview graph
- Template filter: `env:prod|staging` · time ranges · Live/Paused

### Dashboard APIs

| Method | Path | Purpose |
|--------|------|---------|
| GET | `/api/v1/boards` | List board definitions |
| GET | `/api/v1/boards/{id}/render?range_ms=&tags=` | Resolve widgets (values, series, toplists) |
| GET | `/api/v1/metrics/summary?range_ms=` | Granular metric stats + sparklines |

## Plug in OpenTelemetry

Any codebase that can export OTLP metrics can use Thine:

```bash
export OTEL_EXPORTER_OTLP_ENDPOINT=http://localhost:4318
export OTEL_EXPORTER_OTLP_PROTOCOL=http/json
export OTEL_EXPORTER_OTLP_METRICS_ENDPOINT=http://localhost:4318/v1/metrics
```

### Python

```bash
python3 examples/python/emit_metrics.py
# or: pip install -e "sdks/python[otel]" && use configure_thine()
```

### Go

```bash
cd examples/go && go run .
```

## Architecture

```
crates/
  thine-common/    shared metric types
  thine-storage/   concurrent TS store, query, alerts
  thine-ingest/    OTLP JSON · StatsD · simple JSON
  thine-server/    Axum API + static UI
ui/                React metric explorer
sdks/python|go/    language helpers
examples/          emitters for any service
```

### HTTP surface

| Method | Path | Purpose |
|--------|------|---------|
| POST | `/v1/metrics` | OTLP/HTTP JSON export |
| POST | `/api/v1/ingest` | Simple JSON points |
| POST | `/api/v1/series` | Batched series |
| POST | `/api/v1/ingest/statsd` | StatsD lines |
| GET | `/api/v1/query` | Aggregate query |
| GET | `/api/v1/metrics` | Series catalog |
| GET | `/api/v1/dashboard` | Live summary |
| GET/POST | `/api/v1/alerts` | Threshold monitors |
| GET/POST | `/api/v1/boards` | Saved dashboard boards |
| GET | `/api/v1/boards/{id}/render` | Rendered widget payloads |
| GET | `/api/v1/metrics/summary` | Granular metrics summary |

## Iterate loop

```bash
./scripts/score_loop.sh
```

Runs **rebuild → test → boot → capture → score → report**. Gate defaults to score ≥ 24 with all capture checks green. Report lands in `docs/SCORE_REPORT.md`.

## Config

| Env | Default | Meaning |
|-----|---------|---------|
| `THINE_HOST` | `0.0.0.0` | Bind address |
| `THINE_PORT` | `4318` | Port (OTLP-friendly) |
| `THINE_UI_DIR` | `ui/dist` | Built UI path |
| `THINE_SEED_DEMO` | `true` | Demo series + live generator |
| `THINE_RETENTION_MS` | `86400000` | In-memory retention |

## Roadmap (from score gaps)

1. OTLP protobuf + gRPC
2. Persistent TSDB
3. Alert notification channels
4. Custom dashboard boards
5. Richer query language

## License

Apache-2.0
