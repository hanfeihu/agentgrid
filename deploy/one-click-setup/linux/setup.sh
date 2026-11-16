#!/bin/sh
# AgentGrid one-line setup for Linux servers:
#   curl -fsSL https://agentgrid.tminos.com/setup.sh | sudo sh
# Optional: name this machine with  ... | sudo AGENTGRID_NODE_NAME="My server" sh
#
# - The worker binary comes from the Hub (no GitHub access needed).
# - It runs as the low-privilege user "agentgrid" and owns its own directory, so it can update itself.
# - Running it again on a machine that already has a worker keeps the same node id and name.
set -eu

HUB="${AGENTGRID_HUB_URL:-https://agentgrid.tminos.com}"
DIR=/opt/agentgrid-worker
BIN="$DIR/agentgrid-worker"
UNIT=/etc/systemd/system/agentgrid-worker.service
SVC_USER=agentgrid

fail() { echo "ERROR: $*" >&2; echo "Setup did NOT finish. Please send this output to us." >&2; exit 1; }

[ "$(id -u)" = 0 ] || fail "please run as root:  curl -fsSL $HUB/setup.sh | sudo sh"
command -v systemctl >/dev/null 2>&1 || fail "this machine has no systemd"
command -v curl >/dev/null 2>&1 || fail "curl is not installed"
[ "$(uname -m)" = x86_64 ] || fail "only x86_64 Linux is supported (this is $(uname -m))"

glibc=$(ldd --version 2>/dev/null | head -1 | grep -o '[0-9]*\.[0-9]*$' || true)
if [ -n "$glibc" ]; then
  major=${glibc%%.*}; minor=${glibc#*.}
  if [ "$major" -lt 2 ] || { [ "$major" -eq 2 ] && [ "$minor" -lt 34 ]; }; then
    fail "glibc $glibc is too old; the worker needs glibc 2.34 or newer (Ubuntu 22.04+, Debian 12+, RHEL 9+)"
  fi
fi

# Keep the node id and name of an existing worker (installed by this script or by install.sh).
NODE_ID=""; OLD_NAME=""
if [ -f "$UNIT" ]; then
  exec_line=$(grep '^ExecStart=' "$UNIT" || true)
  NODE_ID=$(printf '%s\n' "$exec_line" | sed -n 's/.* --id \([^ ]*\).*/\1/p')
  OLD_NAME=$(printf '%s\n' "$exec_line" | sed -n 's/.* --name "\([^"]*\)".*/\1/p')
fi
[ -n "$NODE_ID" ] || NODE_ID="$(hostname)-linux"
NAME="${AGENTGRID_NODE_NAME:-${OLD_NAME:-$(hostname)}}"
NAME=$(printf '%s' "$NAME" | tr -d '"\\%$`')
JOBS=$(nproc 2>/dev/null || echo 4); [ "$JOBS" -gt 4 ] && JOBS=4; [ "$JOBS" -lt 1 ] && JOBS=1

echo "AgentGrid setup"
echo "  hub:  $HUB"
echo "  node: $NODE_ID ($NAME)"

id "$SVC_USER" >/dev/null 2>&1 || useradd --system --home-dir "$DIR" --shell /usr/sbin/nologin "$SVC_USER"
install -d -o "$SVC_USER" -g "$SVC_USER" -m 0750 "$DIR" "$DIR/.agentgrid"

echo "Downloading worker..."
tmp=$(mktemp)
curl -fsSL "$HUB/api/worker/download/linux-x86_64" -o "$tmp" || fail "could not download the worker from $HUB"
want=$(curl -fsSL "$HUB/api/worker/update-manifest?worker_target=linux-x86_64&os=linux&arch=x86_64&glibc_version=${glibc:-2.35}" \
  | tr ',{}' '\n\n\n' | grep '"sha256"' | head -1 | cut -d'"' -f4)
got=$(sha256sum "$tmp" | cut -d' ' -f1)
[ -n "$want" ] && [ "$want" = "$got" ] || { rm -f "$tmp"; fail "downloaded worker checksum mismatch (expected $want, got $got)"; }
install -o "$SVC_USER" -g "$SVC_USER" -m 0755 "$tmp" "$BIN"
rm -f "$tmp"

cat > "$UNIT" <<UNIT
[Unit]
Description=AgentGrid Worker
After=network-online.target
Wants=network-online.target

[Service]
User=$SVC_USER
WorkingDirectory=$DIR
Environment=HOME=$DIR
ExecStart=$BIN --hub $HUB --id $NODE_ID --name "$NAME" --tag worker --tag linux --capability http --capability command --capability file --capability git --capability docker --capability browser --capability session --capability agentmessage --capability plugin --max-concurrent-jobs $JOBS --interval-seconds 5
Restart=always
RestartSec=5

[Install]
WantedBy=multi-user.target
UNIT

START=$(date -u +%Y-%m-%dT%H:%M:%S)
systemctl daemon-reload
systemctl enable agentgrid-worker >/dev/null 2>&1
systemctl restart agentgrid-worker

echo "Waiting for the Hub to see this machine..."
i=0
while [ $i -lt 30 ]; do
  # one node per chunk: split the node list at each "kind":"Node", then read our own last heartbeat.
  # Only a heartbeat newer than the restart counts: the old process's last one keeps the node "online" for a while.
  hb=$(curl -fsS "$HUB/api/nodes" 2>/dev/null | sed 's/"kind":"Node"/\n/g' | grep "\"id\":\"$NODE_ID\"" \
    | grep -o '"last_heartbeat_at":"[^"]*"' | head -1 | cut -d'"' -f4)
  if [ -n "$hb" ] && [ "$(printf '%s\n%s\n' "$START" "$hb" | sort | tail -1)" = "$hb" ] && [ "$hb" != "$START" ]; then
    echo "Done! This machine is connected as \"$NAME\"."
    exit 0
  fi
  i=$((i + 1)); sleep 2
done
systemctl --no-pager status agentgrid-worker | head -5 >&2 || true
journalctl -u agentgrid-worker --no-pager -n 15 >&2 || true
fail "the worker is installed but has not come online yet"
