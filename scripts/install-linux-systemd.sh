#!/usr/bin/env bash
set -euo pipefail

HUB_URL="${AGENTGRID_HUB_URL:-http://chenqi.tminos.com:20080/agentgrid}"
NODE_ID="${AGENTGRID_NODE_ID:-$(hostname)-linux}"
NODE_NAME="${AGENTGRID_NODE_NAME:-$(hostname)}"
INSTALL_DIR="${AGENTGRID_WORKER_DIR:-/opt/agentgrid-worker}"
BIN_PATH="${AGENTGRID_WORKER_BIN:-$INSTALL_DIR/agentgrid-worker}"
USER_NAME="${AGENTGRID_WORKER_USER:-agentgrid}"
MAX_JOBS="${AGENTGRID_MAX_CONCURRENT_JOBS:-4}"
INTERVAL="${AGENTGRID_WORKER_INTERVAL:-5}"
JOIN_TOKEN="${AGENTGRID_JOIN_TOKEN:-${AG_JOIN_TOKEN:-}}"
JOIN_TOKEN_EXEC=""
if [ -n "$JOIN_TOKEN" ]; then
  JOIN_TOKEN_EXEC=" --join-token $JOIN_TOKEN"
fi

sudo mkdir -p "$INSTALL_DIR"
sudo cp "${1:-target/release/agentgrid-worker}" "$BIN_PATH"
sudo chmod 0755 "$BIN_PATH"
if ! id "$USER_NAME" >/dev/null 2>&1; then
  sudo useradd --system --home "$INSTALL_DIR" --shell /usr/sbin/nologin "$USER_NAME"
fi
sudo chown -R "$USER_NAME":"$USER_NAME" "$INSTALL_DIR"

sudo tee /etc/systemd/system/agentgrid-worker.service >/dev/null <<SERVICE
[Unit]
Description=AgentGrid Worker
After=network-online.target
Wants=network-online.target

[Service]
User=$USER_NAME
WorkingDirectory=$INSTALL_DIR
ExecStart=$BIN_PATH --hub $HUB_URL --id $NODE_ID --name "$NODE_NAME" --tag worker --tag linux --capability http --capability command --capability file --capability git --capability docker --capability browser --capability session --capability agentmessage --capability plugin --max-concurrent-jobs $MAX_JOBS --interval-seconds $INTERVAL$JOIN_TOKEN_EXEC
Restart=always
RestartSec=5

[Install]
WantedBy=multi-user.target
SERVICE

sudo systemctl daemon-reload
sudo systemctl enable --now agentgrid-worker.service
systemctl status agentgrid-worker.service --no-pager
