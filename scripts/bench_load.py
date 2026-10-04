#!/usr/bin/env python3
"""Load + latency benchmark for Thine Metrics vs published Datadog economics."""
from __future__ import annotations

import concurrent.futures
import json
import os
import statistics
import time
import urllib.error
import urllib.request
from pathlib import Path

BASE = os.environ.get("THINE_ENDPOINT", "http://127.0.0.1:4318")
OUT = Path(os.environ.get("THINE_BENCH_OUT", "docs/bench_results.json"))


def req(method: str, path: str, body: bytes | None = None, timeout: float = 30.0):
    r = urllib.request.Request(
        BASE + path,
        data=body,
        method=method,
        headers={"content-type": "application/json"} if body is not None else {},
    )
    t0 = time.perf_counter()
    try:
        with urllib.request.urlopen(r, timeout=timeout) as resp:
            data = resp.read()
            code = resp.status
    except urllib.error.HTTPError as e:
        data = e.read()
        code = e.code
    except Exception as e:
        return {"ok": False, "error": str(e), "ms": (time.perf_counter() - t0) * 1000}
    ms = (time.perf_counter() - t0) * 1000
    return {"ok": 200 <= code < 300, "code": code, "ms": ms, "bytes": len(data)}


def timed_batch(name: str, fn, n: int, workers: int):
    latencies = []
    errors = 0
    t0 = time.perf_counter()
    with concurrent.futures.ThreadPoolExecutor(max_workers=workers) as ex:
        futs = [ex.submit(fn, i) for i in range(n)]
        for f in concurrent.futures.as_completed(futs):
            r = f.result()
            latencies.append(r["ms"])
            if not r.get("ok"):
                errors += 1
    wall = time.perf_counter() - t0
    latencies.sort()

    def pct(p):
        if not latencies:
            return 0.0
        return latencies[min(len(latencies) - 1, int(p * (len(latencies) - 1)))]

    return {
        "name": name,
        "requests": n,
        "workers": workers,
        "errors": errors,
        "wall_sec": round(wall, 3),
        "rps": round(n / wall, 1) if wall > 0 else 0,
        "latency_ms": {
            "p50": round(pct(0.50), 2),
            "p95": round(pct(0.95), 2),
            "p99": round(pct(0.99), 2),
            "avg": round(statistics.fmean(latencies), 2) if latencies else 0,
            "max": round(latencies[-1], 2) if latencies else 0,
        },
    }


def ingest_one(i: int):
    body = json.dumps(
        [
            {
                "name": "bench.requests",
                "metric_type": "gauge",
                "tags": {"service": "bench", "worker": str(i % 16)},
                "sample": {
                    "timestamp_ms": int(time.time() * 1000),
                    "value": float(i % 100),
                },
            }
        ]
    ).encode()
    return req("POST", "/api/v1/ingest", body)


def query_one(_i: int):
    return req(
        "GET",
        "/api/v1/query?metric=bench.requests&aggregation=avg&step_ms=15000",
    )


def health_one(_i: int):
    return req("GET", "/health")


def features_one(_i: int):
    return req("GET", "/api/v1/features/stats")


def dashboard_one(_i: int):
    return req("GET", "/api/v1/dashboard")


def bits_one(_i: int):
    return req("POST", "/api/v1/bits/chat", b'{"message":"slo status"}')


def main():
    # warm
    for _ in range(5):
        health_one(0)

    scenarios = [
        timed_batch("health_get", health_one, 500, 32),
        timed_batch("ingest_points", ingest_one, 2000, 64),
        timed_batch("query_metric", query_one, 500, 32),
        timed_batch("dashboard", dashboard_one, 300, 16),
        timed_batch("features_stats", features_one, 200, 16),
        timed_batch("bits_chat", bits_one, 100, 8),
    ]

    # sustained ingest for 5s
    stop = time.perf_counter() + 5.0
    sustained = 0
    errors = 0
    t0 = time.perf_counter()
    with concurrent.futures.ThreadPoolExecutor(max_workers=64) as ex:
        futs = []
        i = 0
        while time.perf_counter() < stop:
            futs.append(ex.submit(ingest_one, i))
            i += 1
            if len(futs) > 500:
                for f in futs:
                    r = f.result()
                    sustained += 1
                    if not r.get("ok"):
                        errors += 1
                futs = []
        for f in futs:
            r = f.result()
            sustained += 1
            if not r.get("ok"):
                errors += 1
    wall = time.perf_counter() - t0
    sustained_rps = round(sustained / wall, 1)

    compare = json.load(urllib.request.urlopen(BASE + "/api/v1/compare/datadog", timeout=30))
    stats = json.load(urllib.request.urlopen(BASE + "/api/v1/features/stats", timeout=10))
    dash = json.load(urllib.request.urlopen(BASE + "/api/v1/dashboard", timeout=10))

    # TCO model: 50 hosts, infra+APM+logs+custom metrics
    hosts = 50
    dd_infra = hosts * 15  # Pro annual
    dd_apm = hosts * 31
    # assume 5k custom metrics → overage after 100/host = 5000-5000=0 included; use 20k CM
    custom_metrics = 20_000
    included = hosts * 100
    overage_packs = max(0, custom_metrics - included) / 100
    dd_cm = overage_packs * 5
    # logs: 500GB ingest + 50M indexed @15d ~ $1.70/M
    dd_logs = 500 * 0.10 + 50 * 1.70
    dd_monthly = dd_infra + dd_apm + dd_cm + dd_logs
    # Thine: single c6i.xlarge-ish or similar ~$120 + 200GB disk ~$20
    thine_monthly = 140

    report = {
        "endpoint": BASE,
        "timestamp": time.strftime("%Y-%m-%dT%H:%M:%SZ", time.gmtime()),
        "feature_stats": stats,
        "compare_summary": {
            "avg_parity_pct": compare.get("avg_parity_pct"),
            "thine_competitive_or_better": compare.get("thine_competitive_or_better"),
            "datadog_clear_lead": compare.get("datadog_clear_lead"),
            "thine_differentiators": compare.get("thine_differentiators"),
            "datadog_differentiators": compare.get("datadog_differentiators"),
        },
        "live": {
            "series_count": dash.get("series_count"),
            "sample_count": dash.get("sample_count"),
            "ingest_rate_per_sec": dash.get("ingest_rate_per_sec"),
        },
        "load_tests": scenarios,
        "sustained_ingest": {
            "seconds": round(wall, 2),
            "requests": sustained,
            "errors": errors,
            "rps": sustained_rps,
        },
        "tco_50_hosts_usd_month": {
            "assumptions": {
                "hosts": hosts,
                "datadog_infra_pro_per_host": 15,
                "datadog_apm_per_host": 31,
                "custom_metrics": custom_metrics,
                "log_ingest_gb": 500,
                "indexed_log_events_millions": 50,
                "thine_vm_disk": thine_monthly,
            },
            "datadog_estimated": round(dd_monthly, 2),
            "thine_estimated": thine_monthly,
            "savings_pct": round((1 - thine_monthly / dd_monthly) * 100, 1),
            "breakdown_datadog": {
                "infra": dd_infra,
                "apm": dd_apm,
                "custom_metrics_overage": round(dd_cm, 2),
                "logs": round(dd_logs, 2),
            },
        },
        "why_thine": [
            "Self-hosted Apache-2.0 — telemetry stays in your VPC",
            f"Est. {round((1 - thine_monthly / dd_monthly) * 100)}% lower monthly cost at 50-host mid-size footprint",
            f"Ingest sustained ~{sustained_rps} req/s on a single local process in this bench",
            "No per-host or per-custom-metric tax; predictable infra bill",
            f"Feature catalog: {stats.get('done')}/{stats.get('total')} Done, avg parity {stats.get('avg_parity_pct')}% vs Datadog surfaces",
            "MCP + Bits over local data — agentic ops without SaaS round-trips",
        ],
        "why_datadog_still": compare.get("datadog_differentiators"),
    }

    OUT.parent.mkdir(parents=True, exist_ok=True)
    OUT.write_text(json.dumps(report, indent=2))
    print(json.dumps(report, indent=2))
    print(f"\nWrote {OUT}")


if __name__ == "__main__":
    main()
