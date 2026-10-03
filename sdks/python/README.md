# Thine Metrics — Python SDK

Connect any Python service to Thine Metrics.

## Zero-dependency JSON client

```python
from thine_metrics import ThineClient

client = ThineClient("http://localhost:4318")
client.gauge("orders.open", 12, tags={"service": "checkout", "env": "prod"})
```

## OpenTelemetry (recommended)

```bash
pip install "thine-metrics[otel]"
```

```python
from thine_metrics import configure_thine
from opentelemetry import metrics

configure_thine("http://localhost:4318", service_name="checkout")
meter = metrics.get_meter("checkout")
requests = meter.create_counter("http.server.request.count")
requests.add(1, {"route": "/pay"})
```

Or point the standard OTEL env vars at Thine — no SDK import required:

```bash
export OTEL_EXPORTER_OTLP_ENDPOINT=http://localhost:4318
export OTEL_EXPORTER_OTLP_PROTOCOL=http/json
```
