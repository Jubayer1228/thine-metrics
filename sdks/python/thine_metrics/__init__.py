"""Thine Metrics Python helper — configures OpenTelemetry to export to Thine.

Works with any Python codebase. Install OpenTelemetry packages, then::

    from thine_metrics import configure_thine
    configure_thine(endpoint="http://localhost:4318")

Or push a simple JSON batch without OTEL::

    from thine_metrics import ThineClient
    ThineClient("http://localhost:4318").gauge("orders.open", 12, tags={"service": "api"})
"""

from __future__ import annotations

import json
import time
import urllib.request
from typing import Any, Mapping, Optional


def configure_thine(
    endpoint: str = "http://localhost:4318",
    service_name: str = "python-service",
    export_interval_millis: int = 5000,
) -> Any:
    """Configure OTEL metrics export to a Thine Metrics OTLP/HTTP JSON endpoint.

    Requires:
      pip install opentelemetry-api opentelemetry-sdk opentelemetry-exporter-otlp-proto-http
    """
    from opentelemetry import metrics
    from opentelemetry.exporter.otlp.proto.http.metric_exporter import OTLPMetricExporter
    from opentelemetry.sdk.metrics import MeterProvider
    from opentelemetry.sdk.metrics.export import PeriodicExportingMetricReader
    from opentelemetry.sdk.resources import Resource

    resource = Resource.create({"service.name": service_name})
    exporter = OTLPMetricExporter(
        endpoint=f"{endpoint.rstrip('/')}/v1/metrics",
    )
    reader = PeriodicExportingMetricReader(
        exporter, export_interval_millis=export_interval_millis
    )
    provider = MeterProvider(resource=resource, metric_readers=[reader])
    metrics.set_meter_provider(provider)
    return metrics.get_meter(service_name)


class ThineClient:
    """Minimal JSON client — no OTEL dependency required."""

    def __init__(self, base_url: str = "http://localhost:4318") -> None:
        self.base_url = base_url.rstrip("/")

    def _post(self, path: str, payload: Any) -> dict:
        data = json.dumps(payload).encode("utf-8")
        req = urllib.request.Request(
            f"{self.base_url}{path}",
            data=data,
            headers={"Content-Type": "application/json"},
            method="POST",
        )
        with urllib.request.urlopen(req, timeout=10) as resp:
            return json.loads(resp.read().decode("utf-8"))

    def gauge(
        self,
        name: str,
        value: float,
        tags: Optional[Mapping[str, str]] = None,
        timestamp_ms: Optional[int] = None,
    ) -> dict:
        return self._post(
            "/api/v1/ingest",
            [
                {
                    "name": name,
                    "type": "gauge",
                    "value": value,
                    "tags": dict(tags or {}),
                    "timestamp_ms": timestamp_ms or int(time.time() * 1000),
                }
            ],
        )

    def counter(
        self,
        name: str,
        value: float,
        tags: Optional[Mapping[str, str]] = None,
    ) -> dict:
        return self._post(
            "/api/v1/ingest",
            [
                {
                    "name": name,
                    "type": "counter",
                    "value": value,
                    "tags": dict(tags or {}),
                    "timestamp_ms": int(time.time() * 1000),
                }
            ],
        )

    def series(self, metric: str, points: list[tuple[float, float]], tags: Optional[Mapping[str, str]] = None) -> dict:
        return self._post(
            "/api/v1/series",
            {
                "series": [
                    {
                        "metric": metric,
                        "type": "gauge",
                        "tags": dict(tags or {}),
                        "points": [[ts, val] for ts, val in points],
                    }
                ]
            },
        )


__all__ = ["configure_thine", "ThineClient"]
