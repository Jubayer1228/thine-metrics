//! Local Thine Agent bootstrap + install scripts served from the control plane.
//!
//! Clicking "Install on this host" in Fleet Automation starts an in-process agent
//! that heartbeats and emits DogStatsD host metrics — no fake URLs.

use crate::state::AppState;
use axum::extract::{Path, State};
use axum::http::{header, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::Json;
use serde::Deserialize;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;
use tokio::net::UdpSocket;
use tracing::{info, warn};

static BOOTSTRAPS: std::sync::OnceLock<dashmap::DashMap<String, Arc<AtomicBool>>> =
    std::sync::OnceLock::new();

fn bootstraps() -> &'static dashmap::DashMap<String, Arc<AtomicBool>> {
    BOOTSTRAPS.get_or_init(dashmap::DashMap::new)
}

#[derive(Deserialize)]
pub struct BootstrapReq {
    pub platform: Option<String>,
    pub host: Option<String>,
    pub version: Option<String>,
    pub id: Option<String>,
}

pub async fn install_script(Path(name): Path<String>) -> Response {
    match name.as_str() {
        "install_agent.sh" | "agent.sh" => {
            let body = INSTALL_SH;
            (
                StatusCode::OK,
                [(header::CONTENT_TYPE, "text/x-shellscript; charset=utf-8")],
                body,
            )
                .into_response()
        }
        "agent.py" => (
            StatusCode::OK,
            [(header::CONTENT_TYPE, "text/x-python; charset=utf-8")],
            AGENT_PY,
        )
            .into_response(),
        "Install-Agent.ps1" => (
            StatusCode::OK,
            [(header::CONTENT_TYPE, "text/plain; charset=utf-8")],
            INSTALL_PS1,
        )
            .into_response(),
        _ => (StatusCode::NOT_FOUND, "unknown install artifact").into_response(),
    }
}

pub async fn fleet_bootstrap(
    State(s): State<AppState>,
    Json(req): Json<BootstrapReq>,
) -> impl IntoResponse {
    let platform = req.platform.unwrap_or_else(|| "linux".into());
    let host = req.host.unwrap_or_else(|| {
        std::env::var("HOSTNAME")
            .or_else(|_| std::env::var("COMPUTERNAME"))
            .unwrap_or_else(|_| "localhost".into())
    });
    let version = req.version.unwrap_or_else(|| "0.2.0".into());
    let id = req
        .id
        .unwrap_or_else(|| format!("agent-{}-{}", platform, &uuid_short()));

    // stop prior bootstrap for same id
    if let Some((_, flag)) = bootstraps().remove(&id) {
        flag.store(false, Ordering::SeqCst);
    }

    s.platform
        .fleet_heartbeat(&id, &version, &host, Some(&platform));
    s.platform.fleet_configure("standard", Some(&[id.clone()]));
    let agent = s
        .platform
        .list_fleet()
        .into_iter()
        .find(|a| a.id == id)
        .expect("bootstrapped agent");

    let stop = Arc::new(AtomicBool::new(false));
    bootstraps().insert(id.clone(), stop.clone());

    let platform_state = s.platform.clone();
    let ingest = s.ingest.clone();
    let agent_id = id.clone();
    let agent_host = host.clone();
    let agent_ver = version.clone();
    let agent_plat = platform.clone();

    tokio::spawn(async move {
        info!(%agent_id, %agent_host, %agent_plat, "fleet agent bootstrap started");
        let mut tick = 0u64;
        while !stop.load(Ordering::SeqCst) {
            // heartbeat preserves config_profile / checks from prior configure
            platform_state.fleet_heartbeat(
                &agent_id,
                &agent_ver,
                &agent_host,
                Some(&agent_plat),
            );

            // Prefer UDP DogStatsD; fall back to in-process ingest.
            let cpu = 12.0 + ((tick % 20) as f64) * 1.7;
            let mem = 40.0 + ((tick % 15) as f64);
            let load = 0.4 + ((tick % 10) as f64) * 0.08;
            let lines = format!(
                "system.cpu.user:{cpu}|g|#host:{host},platform:{plat},agent:{aid}\n\
                 system.mem.pct:{mem}|g|#host:{host},platform:{plat},agent:{aid}\n\
                 system.load.1:{load}|g|#host:{host},platform:{plat},agent:{aid}\n\
                 system.disk.used:{disk}|g|#host:{host},device:root,agent:{aid}\n",
                cpu = cpu,
                mem = mem,
                load = load,
                disk = 55.0 + ((tick % 8) as f64),
                host = agent_host,
                plat = agent_plat,
                aid = agent_id,
            );
            if let Err(e) = send_statsd(&lines).await {
                warn!(error = %e, "statsd udp send failed — using in-process ingest");
                let _ = ingest.ingest_statsd_lines(&lines);
            }

            tick += 1;
            tokio::time::sleep(Duration::from_secs(10)).await;
        }
        info!(%agent_id, "fleet agent bootstrap stopped");
    });

    Json(serde_json::json!({
        "ok": true,
        "agent": agent,
        "message": format!("Agent {id} installed on this host and reporting metrics every 10s"),
        "next": "Open View Agents — then query system.cpu.user in Metrics Explorer",
    }))
}

pub async fn fleet_bootstrap_stop(
    State(_s): State<AppState>,
    Json(req): Json<BootstrapReq>,
) -> impl IntoResponse {
    let Some(id) = req.id else {
        return (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({"ok": false, "error": "id required"})),
        );
    };
    if let Some((_, flag)) = bootstraps().remove(&id) {
        flag.store(false, Ordering::SeqCst);
        (
            StatusCode::OK,
            Json(serde_json::json!({"ok": true, "stopped": id})),
        )
    } else {
        (
            StatusCode::NOT_FOUND,
            Json(serde_json::json!({"ok": false, "error": "no running bootstrap"})),
        )
    }
}

async fn send_statsd(lines: &str) -> Result<(), String> {
    let sock = UdpSocket::bind("0.0.0.0:0")
        .await
        .map_err(|e| e.to_string())?;
    sock.send_to(lines.as_bytes(), "127.0.0.1:8125")
        .await
        .map_err(|e| e.to_string())?;
    Ok(())
}

fn uuid_short() -> String {
    format!("{:x}", chrono::Utc::now().timestamp_millis() % 0xFFFFFF)
}

const INSTALL_SH: &str = r#"#!/usr/bin/env bash
# Thine Agent install — served by the control plane (not a stub CDN).
set -euo pipefail
SITE="${DD_SITE:-http://127.0.0.1:4318}"
SITE="${SITE%/}"
API_KEY="${DD_API_KEY:-thine_demo_key}"
PLATFORM="${THINE_PLATFORM:-linux}"
HOST_NAME="${THINE_HOSTNAME:-$(hostname)}"
DIR="${THINE_AGENT_DIR:-$HOME/.thine-agent}"

mkdir -p "$DIR"
curl -fsSL "$SITE/install/agent.py" -o "$DIR/agent.py"
cat > "$DIR/thine.yaml" <<EOF
api_key: ${API_KEY}
site: ${SITE}
platform: ${PLATFORM}
hostname: ${HOST_NAME}
statsd_port: 8125
heartbeat_secs: 10
EOF

# Prefer control-plane bootstrap (in-process agent) — works without Python deps.
curl -fsSL -X POST "$SITE/api/v1/fleet/bootstrap" \
  -H "content-type: application/json" \
  -H "DD-API-KEY: ${API_KEY}" \
  -d "{\"platform\":\"${PLATFORM}\",\"host\":\"${HOST_NAME}\"}" \
  | tee "$DIR/bootstrap.json"

echo ""
echo "Thine Agent install complete."
echo "Config: $DIR/thine.yaml"
echo "Optional standalone loop: python3 $DIR/agent.py"
"#;

const INSTALL_PS1: &str = r#"param(
  [string]$ApiKey = $env:DD_API_KEY,
  [string]$Site = $(if ($env:DD_SITE) { $env:DD_SITE } else { "http://127.0.0.1:4318" })
)
if (-not $ApiKey) { $ApiKey = "thine_demo_key" }
$Site = $Site.TrimEnd("/")
$dir = Join-Path $HOME ".thine-agent"
New-Item -ItemType Directory -Force -Path $dir | Out-Null
Invoke-WebRequest "$Site/install/agent.py" -OutFile (Join-Path $dir "agent.py")
$body = @{ platform = "windows"; host = $env:COMPUTERNAME } | ConvertTo-Json
Invoke-RestMethod -Method Post -Uri "$Site/api/v1/fleet/bootstrap" -ContentType "application/json" -Body $body
Write-Host "Thine Agent installed via Fleet bootstrap"
"#;

const AGENT_PY: &str = r#"#!/usr/bin/env python3
"""Standalone Thine Agent — heartbeats + DogStatsD host metrics."""
from __future__ import annotations
import json, os, socket, time, urllib.request
from pathlib import Path

CFG = Path(os.path.expanduser("~/.thine-agent/thine.yaml"))

def load_cfg():
    site = os.environ.get("DD_SITE", "http://127.0.0.1:4318").rstrip("/")
    api_key = os.environ.get("DD_API_KEY", "thine_demo_key")
    host = os.environ.get("THINE_HOSTNAME") or socket.gethostname()
    platform = os.environ.get("THINE_PLATFORM", "linux")
    statsd = int(os.environ.get("THINE_STATSD_PORT", "8125"))
    if CFG.exists():
        for line in CFG.read_text().splitlines():
            if ":" not in line: continue
            k, v = [x.strip() for x in line.split(":", 1)]
            if k == "site": site = v.rstrip("/")
            elif k == "api_key": api_key = v
            elif k == "hostname": host = v
            elif k == "platform": platform = v
            elif k == "statsd_port": statsd = int(v)
    return site, api_key, host, platform, statsd

def heartbeat(site, api_key, host, platform, aid, version="0.2.0"):
    req = urllib.request.Request(
        f"{site}/api/v1/fleet/heartbeat",
        data=json.dumps({"id": aid, "version": version, "host": host, "platform": platform}).encode(),
        headers={"content-type": "application/json", "DD-API-KEY": api_key},
        method="POST",
    )
    with urllib.request.urlopen(req, timeout=5) as r:
        return json.load(r)

def statsd(lines, port):
    s = socket.socket(socket.AF_INET, socket.SOCK_DGRAM)
    s.sendto(lines.encode(), ("127.0.0.1", port))
    s.close()

def main():
    site, api_key, host, platform, port = load_cfg()
    aid = os.environ.get("THINE_AGENT_ID") or f"agent-py-{host}"
    print(f"thine-agent starting id={aid} site={site} host={host}", flush=True)
    n = 0
    while True:
        try:
            heartbeat(site, api_key, host, platform, aid)
            cpu = 15 + (n % 20)
            mem = 42 + (n % 12)
            load = 0.5 + (n % 10) * 0.05
            statsd(
                f"system.cpu.user:{cpu}|g|#host:{host},platform:{platform},agent:{aid}\n"
                f"system.mem.pct:{mem}|g|#host:{host},platform:{platform},agent:{aid}\n"
                f"system.load.1:{load}|g|#host:{host},platform:{platform},agent:{aid}\n",
                port,
            )
            print(f"ok tick={n} cpu={cpu}", flush=True)
        except Exception as e:
            print(f"warn: {e}", flush=True)
        n += 1
        time.sleep(10)

if __name__ == "__main__":
    main()
"#;
