#!/usr/bin/env bash
set -euo pipefail

REMOTE="${1:-chenqi.tminos.com}"
APP_DIR="/opt/agentgrid-hub"

ssh "$REMOTE" "sudo mkdir -p '$APP_DIR/data' && sudo chown -R \$(id -un):\$(id -gn) '$APP_DIR'"
rsync -av --delete \
  --exclude .git \
  --exclude target \
  --exclude apps/agentgrid-web/node_modules \
  --exclude apps/agentgrid-web/dist \
  --exclude '*.db' \
  --exclude '*.log' \
  apps/agentgrid-hub/server.py \
  apps/agentgrid-hub/README.md \
  examples \
  "$REMOTE:$APP_DIR/"
scp deploy/systemd/agentgrid-hub.service "$REMOTE:/tmp/agentgrid-hub.service"
ssh "$REMOTE" "sudo mv /tmp/agentgrid-hub.service /etc/systemd/system/agentgrid-hub.service && sudo systemctl daemon-reload"

echo "Files copied to $REMOTE:$APP_DIR"
echo "Start it with: ssh $REMOTE 'sudo systemctl enable --now agentgrid-hub'"
