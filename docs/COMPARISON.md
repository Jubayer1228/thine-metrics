# Thine Metrics vs the competitive field

Live API: `GET /api/v1/compare` · Datadog feature matrix: `GET /api/v1/compare/datadog`  
Dashboard: **Compare** tab at http://localhost:4318

Peers mirror the SigNoz product menu: **SigNoz, Grafana, Datadog, New Relic, CloudWatch, ClickStack, Dash0**.

## Snapshot (capability averages 0–100)

| Vendor | Avg | Positioning |
|--------|-----|-------------|
| Datadog | highest SaaS breadth | Enterprise SKUs, eBPF USM, Watchdog ML |
| Grafana LGTM | dashboard king | Compose Loki/Tempo/Mimir yourself |
| Thine | self-host Datadog-surface | Apache-2.0 binary, Cloudcraft/USM/CCM/HA |
| SigNoz | OTel-native L/M/T | ClickHouse APM/logs/traces + Cloud |
| ClickStack / Dash0 | CH analytics | HyperDX / OTel SaaS on ClickHouse |
| New Relic | SaaS NRQL | Strong APM, no self-host |
| CloudWatch | AWS-native | Best if you stay in AWS |

Exact numbers: `curl -s localhost:4318/api/v1/compare | jq .averages`

## Thine vs SigNoz (headline)

**Thine wins:** self-host cost predictability, Cloudcraft architecture maps, cloud-cost recommendations, USM/Watchdog product packs, multi-region HA APIs, Datadog-parity catalog depth.

**SigNoz wins:** OTel-native APM UX, exceptions auto-derived from traces, messaging-queue observability, ClickHouse high-cardinality log search, optional managed Cloud.

**Honest Thine gap vs SigNoz:** trace funnels (multi-step drop-off) — productize next.

## When to pick whom

| Choose | When |
|--------|------|
| **Thine** | Datadog-like product breadth, data residency, predictable infra bill |
| **SigNoz** | OTel-first APM/logs/traces with ClickHouse speed |
| **Grafana** | Multi-datasource dashboards + PromQL/LogQL/TraceQL |
| **Datadog** | Enterprise SaaS + eBPF/Watchdog/compliance budget |
| **New Relic** | SaaS APM + free ingest; accept NRQL |
| **CloudWatch** | AWS-only ops |
| **ClickStack** | ClickHouse-first HyperDX stack |
| **Dash0** | Modern OTel SaaS without operating CH |

## Reproduce

```bash
curl -s localhost:4318/api/v1/compare | jq '{averages, vs_signoz:.vs_signoz.summary}'
curl -s localhost:4318/api/v1/compare/datadog | jq '{avg_parity_pct, features}'
./scripts/test_features.sh
```
