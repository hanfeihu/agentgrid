#!/usr/bin/env bash
set -euo pipefail

APP_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PID_FILE="$APP_DIR/agentgrid-hub.pid"
LOG_FILE="$APP_DIR/hub.log"

cd "$APP_DIR"

if [ -f "$PID_FILE" ] && kill -0 "$(cat "$PID_FILE")" 2>/dev/null; then
  echo "AgentGrid Hub is already running with pid $(cat "$PID_FILE")"
  exit 0
fi

BIN="${AGENTGRID_HUB_BIN:-$APP_DIR/bin/agentgrid-hub}"
if [ ! -x "$BIN" ]; then
  BIN="$APP_DIR/../../target/release/agentgrid-hub"
fi
if [ ! -x "$BIN" ]; then
  BIN="$APP_DIR/../../target/debug/agentgrid-hub"
fi

nohup "$BIN" \
  --host "${AGENTGRID_HUB_HOST:-0.0.0.0}" \
  --port "${AGENTGRID_HUB_PORT:-20181}" \
  --db "${AGENTGRID_HUB_DB:-$APP_DIR/data/agentgrid-hub.db}" \
  > "$LOG_FILE" 2>&1 &

echo "$!" > "$PID_FILE"
echo "AgentGrid Hub started with pid $(cat "$PID_FILE")"
