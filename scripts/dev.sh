#!/usr/bin/env bash
# Start Thine Metrics with UI (single process, production-built UI).
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "$ROOT"

echo "==> building UI"
(cd ui && npm run build)

echo "==> starting thine-server on http://localhost:4318"
export THINE_UI_DIR="$ROOT/ui/dist"
export THINE_SEED_DEMO="${THINE_SEED_DEMO:-true}"
export THINE_PORT="${THINE_PORT:-4318}"
export THINE_HOST="${THINE_HOST:-127.0.0.1}"
exec cargo run -p thine-server
