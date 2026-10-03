# Datadog feature matrix

Canonical live catalog: `GET /api/v1/features` and `GET /api/v1/features/stats`.

Smoke suite: `./scripts/test_features.sh` (48/48 green after platform landing).

| Status | Meaning | Count (current) |
|--------|---------|----------------:|
| done | Substantial working path | 6 |
| partial | MVP API + in-memory storage | 17 |
| stub | Endpoint exists, shallow depth | 23 |
| planned | Not exposed yet | 4 |
| **total** | | **50** |

**Coverage (done + partial): 46%**

See the Cursor canvas `datadog-feature-parity.canvas.tsx` for the interactive tracker.
