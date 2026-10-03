#!/usr/bin/env bash
# rebuild → task → capture → score → fix → re-run
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "$ROOT"

GATE="${THINE_SCORE_GATE:-24}"
PORT="${THINE_PORT:-4318}"
REPORT="$ROOT/docs/SCORE_REPORT.md"
CAPTURE_DIR="$ROOT/docs/captures"
mkdir -p "$CAPTURE_DIR"

pass=0
fail=0
notes=""

log() { printf '==> %s\n' "$*"; }
ok() { pass=$((pass + 1)); notes="${notes}- PASS: $1\n"; }
bad() { fail=$((fail + 1)); notes="${notes}- FAIL: $1\n"; }

log "rebuild rust"
cargo test --workspace --quiet

log "rebuild ui"
(cd ui && npm run build)

log "task: boot server"
pkill -f 'thine-server' 2>/dev/null || true
sleep 0.3
THINE_SEED_DEMO=true THINE_PORT="$PORT" THINE_UI_DIR="$ROOT/ui/dist" \
  cargo run -q -p thine-server >"$CAPTURE_DIR/server.log" 2>&1 &
SERVER_PID=$!
cleanup() { kill "$SERVER_PID" 2>/dev/null || true; }
trap cleanup EXIT

log "wait for health"
for i in $(seq 1 60); do
  if curl -sf "http://127.0.0.1:${PORT}/health" >/dev/null; then
    break
  fi
  sleep 0.5
  if [ "$i" -eq 60 ]; then
    bad "server health"
    cat "$CAPTURE_DIR/server.log" || true
    exit 1
  fi
done
ok "server health"

log "capture: ingest + query"
curl -sf -X POST "http://127.0.0.1:${PORT}/api/v1/ingest" \
  -H 'content-type: application/json' \
  -d '[{"name":"score.loop.probe","type":"gauge","value":1,"tags":{"service":"score","env":"ci"}}]' \
  | tee "$CAPTURE_DIR/ingest.json" >/dev/null
ok "json ingest"

curl -sf -X POST "http://127.0.0.1:${PORT}/v1/metrics" \
  -H 'content-type: application/json' \
  -d '{"resourceMetrics":[{"resource":{"attributes":[{"key":"service.name","value":{"stringValue":"score"}}]},"scopeMetrics":[{"metrics":[{"name":"score.otlp.gauge","gauge":{"dataPoints":[{"asDouble":3.14}]}}]}]}]}' \
  | tee "$CAPTURE_DIR/otlp.json" >/dev/null
ok "otlp json ingest"

curl -sf -X POST "http://127.0.0.1:${PORT}/api/v1/ingest/statsd" \
  --data-binary $'score.statsd:7|g|#service:score,env:ci\n' \
  | tee "$CAPTURE_DIR/statsd.json" >/dev/null
ok "statsd ingest"

curl -sf "http://127.0.0.1:${PORT}/api/v1/query?metric=score.loop.probe&service=score" \
  | tee "$CAPTURE_DIR/query.json" >/dev/null
ok "query api"

curl -sf "http://127.0.0.1:${PORT}/api/v1/dashboard" \
  | tee "$CAPTURE_DIR/dashboard.json" >/dev/null
ok "dashboard api"

if curl -sf "http://127.0.0.1:${PORT}/" | grep -qi 'Thine'; then
  ok "ui served"
else
  bad "ui served"
fi

curl -sf -X POST "http://127.0.0.1:${PORT}/api/v1/boards" \
  -H 'content-type: application/json' \
  -d '{"name":"Score Board","widgets":[{"title":"Probe","metric":"score.loop.probe","aggregation":"avg"}]}' \
  | tee "$CAPTURE_DIR/board.json" >/dev/null
ok "boards api"

# Feature scores mirrored from docs/COMPARISON.md (dashboards=2 after boards API)
total=$((2+1+2+1+2+1+2+2+1+0+0+0+1+0+2+2+2+2+2+1))
max=40
pct="$(python3 -c "print(f'{($total/$max)*100:.1f}')")"

{
  echo "# Score report"
  echo
  echo "Generated: $(date -u +%Y-%m-%dT%H:%M:%SZ)"
  echo
  echo "| Metric | Value |"
  echo "|--------|------:|"
  echo "| Feature score | ${total} / ${max} |"
  echo "| Percent | ${pct}% |"
  echo "| Capture checks passed | ${pass} |"
  echo "| Capture checks failed | ${fail} |"
  echo "| Gate | ${GATE} |"
  echo
  echo "## Checks"
  echo
  printf "%b" "$notes"
  echo
  echo "## Captures"
  echo
  echo "See \`docs/captures/\` for raw API responses."
  echo
  if [ "$total" -ge "$GATE" ] && [ "$fail" -eq 0 ]; then
    echo "**Verdict: PASS** — continue iterating on backlog items in COMPARISON.md."
  else
    echo "**Verdict: FAIL** — fix failing captures or raise feature scores."
  fi
} | tee "$REPORT"

log "score=${total}/${max} (${pct}%) pass=${pass} fail=${fail}"

if [ "$fail" -ne 0 ] || [ "$total" -lt "$GATE" ]; then
  exit 1
fi
