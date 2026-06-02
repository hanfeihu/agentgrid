#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
HUB_PORT="${AGENTGRID_BRIDGE_E2E_PORT:-32182}"
CODEX_PORT="${AGENTGRID_BRIDGE_E2E_CODEX_PORT:-8390}"
HUB_URL="http://127.0.0.1:${HUB_PORT}"
TMP_DIR="$(mktemp -d "${TMPDIR:-/tmp}/agentgrid-bridge-e2e.XXXXXX")"
DB_PATH="$TMP_DIR/agentgrid-hub.db"
WEB_DIR="$TMP_DIR/web"
HUB_LOG="$TMP_DIR/hub.log"
WORKER_LOG="$TMP_DIR/worker.log"
CODEX_LOG="$TMP_DIR/codex.log"
NODE_ID="e2e-codex-bridge-node"
JOIN_TOKEN="agj_e2e_codex_bridge_token"
ADMIN_EMAIL="agentgrid-bridge-e2e@example.com"
ADMIN_PASSWORD="AgentGridBridgeE2E!12345"

cleanup() {
  trap - TERM
  for pid in "${WORKER_PID:-}" "${HUB_PID:-}" "${CODEX_PID:-}"; do
    if [[ -n "$pid" ]]; then
      kill "$pid" >/dev/null 2>&1 || true
      wait "$pid" >/dev/null 2>&1 || true
    fi
  done
  rm -rf "$TMP_DIR"
}
trap cleanup EXIT

if lsof -iTCP:"$CODEX_PORT" -sTCP:LISTEN >/dev/null 2>&1; then
  echo "AgentGrid Bridge E2E requires local port $CODEX_PORT, but it is already in use." >&2
  exit 2
fi

mkdir -p "$WEB_DIR"
printf '<!doctype html><title>AgentGrid Bridge E2E</title>' > "$WEB_DIR/index.html"

node <<'NODE' >"$CODEX_LOG" 2>&1 &
const http = require('node:http');
const port = Number(process.env.AGENTGRID_BRIDGE_E2E_CODEX_PORT || '8390');
const server = http.createServer((req, res) => {
  let body = '';
  req.setEncoding('utf8');
  req.on('data', chunk => { body += chunk; });
  req.on('end', () => {
    res.writeHead(200, { 'content-type': 'application/json' });
    res.end(JSON.stringify({
      ok: true,
      service: 'fake-codex-local',
      method: req.method,
      path: req.url,
      body: body ? JSON.parse(body) : null
    }));
  });
});
server.listen(port, '127.0.0.1');
NODE
CODEX_PID="$!"

cd "$ROOT"
cargo build -p agentgrid-hub -p agentgrid-worker-app

"$ROOT/target/debug/agentgrid-hub" \
  --host 127.0.0.1 \
  --port "$HUB_PORT" \
  --db "$DB_PATH" \
  --web-dir "$WEB_DIR" \
  >"$HUB_LOG" 2>&1 &
HUB_PID="$!"

for _ in {1..80}; do
  if curl -fsS "$HUB_URL/api/health" >/dev/null 2>&1; then
    break
  fi
  sleep 0.25
done
curl -fsS "$HUB_URL/api/health" >/dev/null

ADMIN_TOKEN="$(curl -fsS -X POST \
  -H 'content-type: application/json' \
  -d "{\"email\":\"$ADMIN_EMAIL\",\"name\":\"AgentGrid Bridge E2E Admin\",\"password\":\"$ADMIN_PASSWORD\"}" \
  "$HUB_URL/api/bootstrap/admin" \
  | sed -n 's/.*"token":"\([^"]*\)".*/\1/p')"
if [[ -z "$ADMIN_TOKEN" ]]; then
  echo "AgentGrid Bridge E2E failed: bootstrap admin token missing" >&2
  exit 1
fi

"$ROOT/target/debug/agentgrid-worker" \
  --hub "$HUB_URL" \
  --id "$NODE_ID" \
  --name "E2E Codex Bridge Node" \
  --join-token "$JOIN_TOKEN" \
  --machine-fingerprint e2e-codex-bridge-fingerprint \
  --capability command \
  --capability codex.local_bridge \
  --tag e2e \
  --interval-seconds 1 \
  --no-auto-update \
  >"$WORKER_LOG" 2>&1 &
WORKER_PID="$!"

for _ in {1..80}; do
  if curl -fsS "$HUB_URL/api/nodes" | grep -q "$NODE_ID"; then
    break
  fi
  sleep 0.25
done

curl -fsS -X POST \
  -H "authorization: Bearer $ADMIN_TOKEN" \
  -H 'content-type: application/json' \
  -d '{"actor":"agentgrid-bridge-e2e"}' \
  "$HUB_URL/api/nodes/$NODE_ID/approve" >/dev/null

for _ in {1..80}; do
  if curl -fsS "$HUB_URL/api/local-services" | grep -q '"status":"available"'; then
    break
  fi
  sleep 0.25
done
curl -fsS "$HUB_URL/api/local-services" | grep -q '"codex.local"'
curl -fsS "$HUB_URL/api/local-services" | grep -q '"status":"available"'

SESSION_JSON="$(curl -fsS -X POST \
  -H "authorization: Bearer $ADMIN_TOKEN" \
  -H 'content-type: application/json' \
  -d "{\"node_id\":\"$NODE_ID\",\"service_id\":\"codex.local\"}" \
  "$HUB_URL/api/bridge-sessions")"
SESSION_ID="$(printf '%s' "$SESSION_JSON" | node -e "let s=''; process.stdin.on('data', d=>s+=d); process.stdin.on('end', ()=>console.log(JSON.parse(s).item.metadata.id));")"
BRIDGE_TOKEN="$(printf '%s' "$SESSION_JSON" | node -e "let s=''; process.stdin.on('data', d=>s+=d); process.stdin.on('end', ()=>console.log(JSON.parse(s).item.spec.token));")"

HUB_URL="$HUB_URL" SESSION_ID="$SESSION_ID" BRIDGE_TOKEN="$BRIDGE_TOKEN" node <<'NODE'
const hubUrl = process.env.HUB_URL;
const sessionId = process.env.SESSION_ID;
const token = process.env.BRIDGE_TOKEN;
const wsBase = hubUrl.replace(/^http:/, 'ws:').replace(/^https:/, 'wss:');
const ws = new WebSocket(`${wsBase}/api/bridge-sessions/${sessionId}/ws?token=${encodeURIComponent(token)}`);
const timeout = setTimeout(() => {
  console.error('bridge websocket timed out');
  process.exit(1);
}, 10000);

ws.addEventListener('open', () => {
  ws.send(JSON.stringify({
    type: 'bridge.request',
    method: 'POST',
    path: '/v1/agentgrid-bridge-e2e',
    headers: { 'content-type': 'application/json' },
    body: { hello: 'agentgrid-bridge' }
  }));
});

ws.addEventListener('message', event => {
  const value = JSON.parse(event.data);
  if (value.type === 'bridge.ready') return;
  clearTimeout(timeout);
  if (value.type !== 'bridge.response' || value.status !== 200) {
    console.error(JSON.stringify(value, null, 2));
    process.exit(1);
  }
  const body = JSON.parse(value.body);
  if (!body.ok || body.service !== 'fake-codex-local' || body.path !== '/v1/agentgrid-bridge-e2e') {
    console.error(JSON.stringify(value, null, 2));
    process.exit(1);
  }
  ws.close();
  console.log('AgentGrid Bridge E2E passed: client -> Hub -> Worker -> 127.0.0.1:8390');
});

ws.addEventListener('error', event => {
  clearTimeout(timeout);
  console.error(event.message || 'bridge websocket error');
  process.exit(1);
});
NODE
