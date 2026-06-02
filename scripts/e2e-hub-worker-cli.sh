#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
PORT="${AGENTGRID_E2E_PORT:-32181}"
HUB_URL="http://127.0.0.1:${PORT}"
TMP_DIR="$(mktemp -d "${TMPDIR:-/tmp}/agentgrid-e2e.XXXXXX")"
DB_PATH="$TMP_DIR/agentgrid-hub.db"
WEB_DIR="$TMP_DIR/web"
HUB_LOG="$TMP_DIR/hub.log"
WORKER_LOG="$TMP_DIR/worker.log"
NODE_ID="e2e-local-node"
JOIN_TOKEN="agj_e2e_local_token"
ADMIN_EMAIL="agentgrid-e2e@example.com"
ADMIN_PASSWORD="AgentGridE2E!12345"

cleanup() {
  trap - TERM
  if [[ -n "${WORKER_PID:-}" ]]; then
    kill "$WORKER_PID" >/dev/null 2>&1 || true
    wait "$WORKER_PID" >/dev/null 2>&1 || true
  fi
  if [[ -n "${HUB_PID:-}" ]]; then
    kill "$HUB_PID" >/dev/null 2>&1 || true
    wait "$HUB_PID" >/dev/null 2>&1 || true
  fi
  rm -rf "$TMP_DIR"
}
trap cleanup EXIT

mkdir -p "$WEB_DIR"
printf '<!doctype html><title>AgentGrid E2E</title>' > "$WEB_DIR/index.html"

cd "$ROOT"
cargo build -p agentgrid-hub -p agentgrid-worker-app -p agentgrid-cli

"$ROOT/target/debug/agentgrid-hub" \
  --host 127.0.0.1 \
  --port "$PORT" \
  --db "$DB_PATH" \
  --web-dir "$WEB_DIR" \
  >"$HUB_LOG" 2>&1 &
HUB_PID="$!"

for _ in {1..80}; do
  if "$ROOT/target/debug/agentgrid" --hub "$HUB_URL" health >/dev/null 2>&1; then
    break
  fi
  sleep 0.25
done
"$ROOT/target/debug/agentgrid" --hub "$HUB_URL" health >/dev/null

ADMIN_TOKEN="$(curl -fsS -X POST \
  -H 'content-type: application/json' \
  -d "{\"email\":\"$ADMIN_EMAIL\",\"name\":\"AgentGrid E2E Admin\",\"password\":\"$ADMIN_PASSWORD\"}" \
  "$HUB_URL/api/bootstrap/admin" \
  | sed -n 's/.*"token":"\([^"]*\)".*/\1/p')"
if [[ -z "$ADMIN_TOKEN" ]]; then
  echo "AgentGrid E2E failed: bootstrap admin token missing" >&2
  exit 1
fi

"$ROOT/target/debug/agentgrid-worker" \
  --hub "$HUB_URL" \
  --id "$NODE_ID" \
  --name "E2E Local Node" \
  --join-token "$JOIN_TOKEN" \
  --machine-fingerprint e2e-local-fingerprint \
  --capability command \
  --tag e2e \
  --interval-seconds 2 \
  --no-auto-update \
  >"$WORKER_LOG" 2>&1 &
WORKER_PID="$!"

for _ in {1..80}; do
  if "$ROOT/target/debug/agentgrid" --hub "$HUB_URL" nodes | grep -q "$NODE_ID"; then
    break
  fi
  sleep 0.25
done

curl -fsS -X POST \
  -H "authorization: Bearer $ADMIN_TOKEN" \
  -H 'content-type: application/json' \
  -d '{"actor":"agentgrid-e2e"}' \
  "$HUB_URL/api/nodes/$NODE_ID/approve" >/dev/null

"$ROOT/target/debug/agentgrid" --hub "$HUB_URL" submit-command \
  --node "$NODE_ID" \
  --program sh \
  --arg=-c \
  --arg 'printf agentgrid-e2e-ok' \
  --timeout-seconds 20 \
  --title "AgentGrid E2E command task" \
  --wait \
  --wait-timeout-seconds 60 \
  --expect-exit-code 0 \
  --expect-stdout-contains agentgrid-e2e-ok \
  --output text

echo "AgentGrid E2E passed: Hub + Worker + CLI command flow"
