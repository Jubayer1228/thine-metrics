# Observability (18) — docs-driven implementation

Source: Playwright crawl of https://docs.datadoghq.com/ (+ `/llms.txt` indexes).
Artifacts: `docs/datadog_obs18_crawl.json`, `docs/datadog_obs18_extras.json`, `docs/OBS18_DOCS_ANALYSIS.md`.
Re-run: `node scripts/crawl_datadog_obs.js`

## Mapping (screenshot → Thine)

| Datadog | Thine APIs (docs-aligned) |
|---------|---------------------------|
| Infrastructure | `/infra/hosts`, `/infra/hosts/live`, `/infra/processes` |
| Metrics | `/metrics`, `/query`, `/query/formula` |
| Container Monitoring | `/containers`, `/containers/stats` |
| Serverless | `/serverless/functions` |
| Network Monitoring | `/network/flows` |
| Cloud Cost Management | `/cost/summary`, `/cost/detail`, `/cost/recommendations` |
| **Cloudcraft** | `/cloudcraft/diagram`, `/resources`, `/views` — Group By + overlays |
| Storage Management | `/storage/volumes` |
| APM | `/apm/traces`, `/stats`, `/traces/{id}` |
| Universal Service Monitoring | `/usm/ebpf`, `/discover`, `/red` (hits/errors/duration) |
| Continuous Profiler | `/profiler/profiles`, `/flame` |
| Database Monitoring | `/dbm/queries`, `/explain`, `/schema`, `/apm` |
| Data Streams Monitoring | `/streams` |
| Data Observability | `/data/assets`, `/data/lineage` |
| Log Management | `/logs/ingest`, `/search`, `/facets` |
| Sensitive Data Scanner | `/sds/rules`, `/scan` |
| Observability Pipelines | `/pipelines`, `/run`, `/workers` |
| Error Tracking | `/errors` |

## Dashboard

Open **http://localhost:4318** → **Observability** (live Cloudcraft / USM / APM / DBM / Cost / HA)
and **Docs** (per-feature how-to + curl). Seeded demo data makes effectiveness KPIs visible
(15 cloud resources, USM RED, explain plans, lineage, cost salvage, multi-region HA).

## Catalog

`GET /api/v1/features/stats` → **53 Done / 0 partial · ~75% avg parity**
Smoke: `./scripts/test_features.sh` → **91/91**
