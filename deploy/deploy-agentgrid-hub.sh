#!/usr/bin/env bash
set -euo pipefail

REMOTE="${1:-agentgrid-host.example.com}"
APP_DIR="/opt/agentgrid-hub"

cargo build --release -p agentgrid-hub
npm --prefix apps/agentgrid-web install
npm --prefix apps/agentgrid-web run build

ssh "$REMOTE" "sudo mkdir -p '$APP_DIR/bin' '$APP_DIR/data' '$APP_DIR/web' && sudo chown -R \$(id -un):\$(id -gn) '$APP_DIR'"
rsync -av \
  target/release/agentgrid-hub \
  "$REMOTE:$APP_DIR/bin/"
rsync -av --delete \
  apps/agentgrid-web/dist/ \
  "$REMOTE:$APP_DIR/web/"
rsync -av \
  apps/agentgrid-hub/README.md \
  docs \
  "$REMOTE:$APP_DIR/"
scp deploy/systemd/agentgrid-hub.service "$REMOTE:/tmp/agentgrid-hub.service"
ssh "$REMOTE" "sudo mv /tmp/agentgrid-hub.service /etc/systemd/system/agentgrid-hub.service && sudo systemctl daemon-reload"

echo "Files copied to $REMOTE:$APP_DIR"
echo "Start it with: ssh $REMOTE 'sudo systemctl enable --now agentgrid-hub'"
