#!/usr/bin/env bash
# Exercise every Datadog-parity feature endpoint against a running Thine server.
set -euo pipefail
BASE="${THINE_ENDPOINT:-http://127.0.0.1:4318}"
pass=0
fail=0

check() {
  local name="$1" path="$2"
  local code
  code=$(curl -sS -o /tmp/thine_feat.json -w '%{http_code}' --max-time 10 "${BASE}${path}" || echo "000")
  if [[ "$code" == "200" || "$code" == "201" ]]; then
    echo "PASS  $name ($code) $path"
    pass=$((pass + 1))
  else
    echo "FAIL  $name ($code) $path"
    fail=$((fail + 1))
  fi
}

echo "==> feature endpoint smoke vs $BASE"
check health /health
check features /api/v1/features
check features_stats /api/v1/features/stats
check metrics /api/v1/metrics
check metrics_summary '/api/v1/metrics/summary?range_ms=3600000'
check dashboard /api/v1/dashboard
check boards /api/v1/boards
check alerts /api/v1/alerts
check notebooks /api/v1/notebooks
check teams /api/v1/teams
check rbac_users /api/v1/rbac/users
check rbac_roles /api/v1/rbac/roles
check incidents /api/v1/incidents
check work /api/v1/work
check workflows /api/v1/workflows
check workflow_runs /api/v1/workflows/runs
check slos /api/v1/slos
check catalog /api/v1/catalog/services
check integrations /api/v1/integrations
check hosts /api/v1/infra/hosts
check containers /api/v1/containers
check gpu /api/v1/gpu/devices
check volumes /api/v1/storage/volumes
check serverless /api/v1/serverless/functions
check cost /api/v1/cost/summary
check network /api/v1/network/flows
check k8s /api/v1/k8s/autoscalers
check traces /api/v1/apm/traces
check apm_services /api/v1/apm/services
check usm /api/v1/usm/services
check logs '/api/v1/logs/search?limit=10'
check audit /api/v1/audit
check errors /api/v1/errors
check agents /api/v1/agents
check marketplace /api/v1/marketplace
check pipelines /api/v1/pipelines
check profiler /api/v1/profiler/profiles
check streams /api/v1/streams
check dbm /api/v1/dbm/instances
check data_assets /api/v1/data/assets
check sds /api/v1/sds/rules
check fleet /api/v1/fleet/agents
check governance /api/v1/governance/policies
check dora /api/v1/dora
check mcp /api/v1/mcp/tools
check cli /api/v1/cli/info

# POST probes
code=$(curl -sS -o /tmp/thine_feat.json -w '%{http_code}' --max-time 10 \
  -H 'content-type: application/json' \
  -d '{"message":"how many SLOs?"}' \
  "${BASE}/api/v1/bits/chat" || echo 000)
if [[ "$code" == "200" ]]; then echo "PASS  bits_chat ($code)"; pass=$((pass+1)); else echo "FAIL  bits_chat ($code)"; fail=$((fail+1)); fi

code=$(curl -sS -o /tmp/thine_feat.json -w '%{http_code}' --max-time 10 \
  -H 'content-type: application/json' \
  -d '[{"timestamp_ms":1,"level":"info","service":"test","message":"feature test","attrs":{}}]' \
  "${BASE}/api/v1/logs/ingest" || echo 000)
if [[ "$code" == "200" ]]; then echo "PASS  logs_ingest ($code)"; pass=$((pass+1)); else echo "FAIL  logs_ingest ($code)"; fail=$((fail+1)); fi

echo
echo "pass=$pass fail=$fail"
python3 - <<'PY'
import json,urllib.request,os
base=os.environ.get("THINE_ENDPOINT","http://127.0.0.1:4318")
stats=json.load(urllib.request.urlopen(base+"/api/v1/features/stats", timeout=10))
print("catalog:", stats)
PY

[[ "$fail" -eq 0 ]]
