#!/usr/bin/env python3
"""Emit sample metrics to a local Thine Metrics server."""

from __future__ import annotations

import os
import random
import sys
import time

ROOT = os.path.abspath(os.path.join(os.path.dirname(__file__), "..", "..", "sdks", "python"))
sys.path.insert(0, ROOT)

from thine_metrics import ThineClient  # noqa: E402


def main() -> None:
    endpoint = os.environ.get("THINE_ENDPOINT", "http://localhost:4318")
    client = ThineClient(endpoint)
    print(f"emitting metrics to {endpoint}")
    while True:
        latency = 20 + random.random() * 100
        cpu = random.random()
        stats = client.gauge(
            "http.server.duration",
            latency,
            tags={"service": "python-demo", "env": "dev"},
        )
        client.gauge(
            "process.runtime.cpu.utilization",
            cpu,
            tags={"service": "python-demo", "env": "dev"},
        )
        client.counter(
            "http.server.request.count",
            random.randint(1, 8),
            tags={"service": "python-demo", "env": "dev"},
        )
        print(f"accepted={stats['accepted']} series={stats['series_count']} latency={latency:.1f}")
        time.sleep(2)


if __name__ == "__main__":
    main()
