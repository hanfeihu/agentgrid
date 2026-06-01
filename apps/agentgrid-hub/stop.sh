#!/usr/bin/env bash
set -euo pipefail

APP_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PID_FILE="$APP_DIR/agentgrid-hub.pid"

if [ ! -f "$PID_FILE" ]; then
  echo "AgentGrid Hub is not running"
  exit 0
fi

PID="$(cat "$PID_FILE")"
if kill "$PID" 2>/dev/null; then
  rm -f "$PID_FILE"
  echo "AgentGrid Hub stopped"
else
  rm -f "$PID_FILE"
  echo "AgentGrid Hub pid file removed"
fi

