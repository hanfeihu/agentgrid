#!/usr/bin/env bash
set -euo pipefail

HUB_URL="${AGENTGRID_HUB_URL:-http://chenqi.tminos.com:20080/agentgrid}"
NODE_ID="${AGENTGRID_NODE_ID:-$(hostname)-macos}"
NODE_NAME="${AGENTGRID_NODE_NAME:-$(hostname)}"
INSTALL_DIR="${AGENTGRID_WORKER_DIR:-$HOME/Library/Application Support/AgentGridWorker}"
BIN_PATH="$INSTALL_DIR/agentgrid-worker"
MAX_JOBS="${AGENTGRID_MAX_CONCURRENT_JOBS:-4}"
INTERVAL="${AGENTGRID_WORKER_INTERVAL:-5}"
PLIST="$HOME/Library/LaunchAgents/io.agentgrid.worker.plist"
JOIN_TOKEN="${AGENTGRID_JOIN_TOKEN:-${AG_JOIN_TOKEN:-}}"

mkdir -p "$INSTALL_DIR"
cp "${1:-target/release/agentgrid-worker}" "$BIN_PATH"
chmod 0755 "$BIN_PATH"

cat > "$PLIST" <<PLIST
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
  <key>Label</key><string>io.agentgrid.worker</string>
  <key>ProgramArguments</key>
  <array>
    <string>$BIN_PATH</string>
    <string>--hub</string><string>$HUB_URL</string>
    <string>--id</string><string>$NODE_ID</string>
    <string>--name</string><string>$NODE_NAME</string>
    <string>--tag</string><string>worker</string>
    <string>--tag</string><string>macos</string>
    <string>--capability</string><string>http</string>
    <string>--capability</string><string>command</string>
    <string>--capability</string><string>file</string>
    <string>--capability</string><string>git</string>
    <string>--capability</string><string>docker</string>
    <string>--capability</string><string>browser</string>
    <string>--capability</string><string>session</string>
    <string>--capability</string><string>agentmessage</string>
    <string>--capability</string><string>plugin</string>
    <string>--max-concurrent-jobs</string><string>$MAX_JOBS</string>
    <string>--interval-seconds</string><string>$INTERVAL</string>
$(if [ -n "$JOIN_TOKEN" ]; then printf '    <string>--join-token</string><string>%s</string>\n' "$JOIN_TOKEN"; fi)
  </array>
  <key>RunAtLoad</key><true/>
  <key>KeepAlive</key><true/>
  <key>StandardOutPath</key><string>$INSTALL_DIR/worker.log</string>
  <key>StandardErrorPath</key><string>$INSTALL_DIR/worker.err.log</string>
</dict>
</plist>
PLIST

launchctl unload "$PLIST" >/dev/null 2>&1 || true
launchctl load "$PLIST"
launchctl list | grep io.agentgrid.worker || true
