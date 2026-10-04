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

post_check() {
  local name="$1" path="$2" body="$3"
  local expect="${4:-200}"
  local code
  code=$(curl -sS -o /tmp/thine_feat.json -w '%{http_code}' --max-time 10 \
    -H 'content-type: application/json' \
    -d "$body" \
    "${BASE}${path}" || echo "000")
  if [[ "$code" == "$expect" || "$code" == "200" || "$code" == "201" || "$code" == "204" ]]; then
    echo "PASS  $name ($code) POST $path"
    pass=$((pass + 1))
  else
    echo "FAIL  $name ($code) POST $path"
    fail=$((fail + 1))
    cat /tmp/thine_feat.json 2>/dev/null | head -c 200; echo
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
check container_stats /api/v1/containers/stats
check gpu /api/v1/gpu/devices
check gpu_summary /api/v1/gpu/summary
check volumes /api/v1/storage/volumes
check serverless /api/v1/serverless/functions
check cost /api/v1/cost/summary
check cost_detail /api/v1/cost/detail
check network /api/v1/network/flows
check k8s /api/v1/k8s/autoscalers
check traces /api/v1/apm/traces
check apm_services /api/v1/apm/services
check usm /api/v1/usm/services
check usm_map /api/v1/usm/map
check logs '/api/v1/logs/search?limit=10'
check audit /api/v1/audit
check errors /api/v1/errors
check agents /api/v1/agents
check marketplace /api/v1/marketplace
check pipelines /api/v1/pipelines
check profiler /api/v1/profiler/profiles
check streams /api/v1/streams
check dbm /api/v1/dbm/instances
check dbm_queries /api/v1/dbm/queries
check data_assets /api/v1/data/assets
check sds /api/v1/sds/rules
check sds_scan /api/v1/sds/scan
check fleet /api/v1/fleet/agents
check governance /api/v1/governance/policies
check dora /api/v1/dora
check mcp /api/v1/mcp/tools
check cli /api/v1/cli/info
check probes /api/v1/probes
check byoc /api/v1/byoc/sinks
check mobile /api/v1/mobile/config
check ide /api/v1/ide/plugins
check compare /api/v1/compare/datadog
check apm_stats /api/v1/apm/stats
check service_map /api/v1/service-map
check log_facets /api/v1/logs/facets
check log_agg '/api/v1/logs/aggregate?group_by=level'
check slo_budgets /api/v1/slos/budgets
check host_live /api/v1/infra/hosts/live
check notify_channels /api/v1/notify/channels

check usm_ebpf /api/v1/usm/ebpf
check watchdog_summary /api/v1/watchdog/summary
check ha_status /api/v1/ha/status
check ha_regions /api/v1/ha/regions
check integ_search '/api/v1/integrations/search?limit=5'
check formula '/api/v1/query/formula?expr=avg:http.server.request.duration'
check cloudcraft '/api/v1/cloudcraft/diagram?overlay=observability&group_by=region,vpc'
check processes /api/v1/infra/processes
check usm_red /api/v1/usm/red
check dbm_explain /api/v1/dbm/explain
check dbm_schema /api/v1/dbm/schema
check dbm_apm /api/v1/dbm/apm
check lineage /api/v1/data/lineage
check cost_recs /api/v1/cost/recommendations
check pipe_workers /api/v1/pipelines/workers


# POST probes — deep behaviors
post_check bits_chat /api/v1/bits/chat '{"message":"how many SLOs?"}'
post_check logs_ingest /api/v1/logs/ingest '[{"timestamp_ms":1,"level":"info","service":"test","message":"feature test","attrs":{}}]'
post_check auth_token /api/v1/auth/token '{"email":"tester@thine.dev","roles":["admin"]}' 201
TOKEN=$(python3 -c "import json; print(json.load(open('/tmp/thine_feat.json'))['token'])" 2>/dev/null || true)
if [[ -n "${TOKEN:-}" ]]; then
  code=$(curl -sS -o /tmp/thine_feat.json -w '%{http_code}' --max-time 10 \
    -H "Authorization: Bearer $TOKEN" "${BASE}/api/v1/auth/me" || echo 000)
  if [[ "$code" == "200" ]]; then echo "PASS  auth_me ($code)"; pass=$((pass+1)); else echo "FAIL  auth_me ($code)"; fail=$((fail+1)); fi
fi

post_check fleet_heartbeat /api/v1/fleet/heartbeat '{"id":"agent-test","version":"0.1.1","host":"i-test"}'
post_check fleet_rollout /api/v1/fleet/rollout '{"target_version":"0.1.2"}'
post_check network_ingest /api/v1/network/flows '[{"id":"t1","src":"web","dst":"api","protocol":"tcp","bytes":1000,"packets":10,"timestamp_ms":1}]'
post_check mcp_invoke /api/v1/mcp/invoke '{"tool":"list_slos","args":{}}'
post_check upsert_probe /api/v1/probes '{"id":"probe-test","service":"api","language":"go","method":"Main","enabled":true,"capture":["err"]}' 201
post_check byoc_forward /api/v1/byoc/sinks/s3-archive/forward '{}'
post_check pipeline_run /api/v1/pipelines/default-logs/run '[{"timestamp_ms":9,"level":"info","service":"pipe","message":"k=v","attrs":{}}]'

# marketplace install if apps exist
APP=$(python3 - <<'PY'
import json,urllib.request,os
base=os.environ.get("THINE_ENDPOINT","http://127.0.0.1:4318")
apps=json.load(urllib.request.urlopen(base+"/api/v1/marketplace", timeout=10))
print(apps[0]["id"] if apps else "")
PY
)
if [[ -n "$APP" ]]; then
  post_check marketplace_install "/api/v1/marketplace/${APP}/install" '{}'
fi

# detail assertions
python3 - <<'PY'
import json,urllib.request,os,sys
base=os.environ.get("THINE_ENDPOINT","http://127.0.0.1:4318")
fail=0

def get(path):
    return json.load(urllib.request.urlopen(base+path, timeout=10))

def require(cond, msg):
    global fail
    if cond:
        print(f"PASS  assert {msg}")
    else:
        print(f"FAIL  assert {msg}")
        fail += 1

stats=get("/api/v1/features/stats")
require(stats["stub"]==0 and stats["planned"]==0, f"no stubs/planned: {stats}")
require(stats.get("done")==stats["total"] and stats.get("partial",0)==0, "all done")

bits=json.load(urllib.request.urlopen(urllib.request.Request(
    base+"/api/v1/bits/chat", data=b'{"message":"slo status"}',
    headers={"content-type":"application/json"})))
require(bits.get("intent")=="slo_status", "bits intent slo_status")
require("citations" in bits, "bits citations")

cost=get("/api/v1/cost/detail")
require(isinstance(cost,list) and len(cost)>=1, "cost detail lines")

sds=get("/api/v1/sds/scan")
require(isinstance(sds,list), "sds scan list")

usm=get("/api/v1/usm/map")
require("endpoints" in usm and "edges" in usm and "ebpf" in usm, "usm map shape")
require(get("/api/v1/integrations/search?limit=1")["catalog_size"]>=800, "800+ integrations")
require(get("/api/v1/ha/status").get("quorum") is True, "ha quorum")
require(get("/api/v1/usm/ebpf").get("enabled") is True, "ebpf usm enabled")

probes=get("/api/v1/probes")
require(isinstance(probes,list) and len(probes)>=1, "probes seeded")

byoc=get("/api/v1/byoc/sinks")
require(isinstance(byoc,list) and len(byoc)>=1, "byoc sinks")

mobile=get("/api/v1/mobile/config")
require(mobile.get("app_name"), "mobile config")

ide=get("/api/v1/ide/plugins")
require(len(ide)>=2, "ide plugins")

queries=get("/api/v1/dbm/queries")
require(len(queries)>=1, "dbm queries")

cli=get("/api/v1/cli/info")
require(isinstance(cli.get("commands"), list) and len(cli["commands"])>=5, "cli schema")

sys.exit(fail)
PY
detail_fail=$?
fail=$((fail + detail_fail))

echo
echo "pass=$pass fail=$fail"
python3 - <<'PY'
import json,urllib.request,os
base=os.environ.get("THINE_ENDPOINT","http://127.0.0.1:4318")
stats=json.load(urllib.request.urlopen(base+"/api/v1/features/stats", timeout=10))
print("catalog:", stats)
PY

[[ "$fail" -eq 0 ]]
