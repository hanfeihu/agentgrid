#!/bin/bash
# AgentGrid worker setup for macOS (Apple silicon). Called by "AgentGrid Setup.app": install.sh <hub url> <name>
# - Installs into the user's own Library: no admin password needed.
# - Keeps the node id of an existing worker, so re-running reconnects the same machine.
# - launchd starts /bin/sh, which keeps re-running the worker. launchd pins the code identity of the program
#   it spawns, so pinning /bin/sh (a system binary) lets the worker replace itself on auto-update without
#   macOS refusing the new binary (OS_REASON_CODESIGNING); and the shell loop restarts the worker after it
#   exits, which launchd itself does not do reliably for a clean exit.
set -euo pipefail

HUB="$1"
NAME="$2"
LABEL=io.agentgrid.worker
DIR="$HOME/Library/Application Support/AgentGridWorker"
BIN="$DIR/agentgrid-worker"
PLIST="$HOME/Library/LaunchAgents/$LABEL.plist"
DOMAIN="gui/$(id -u)"

mkdir -p "$DIR" "$HOME/Library/LaunchAgents"

NODE_ID=""
if [ -f "$PLIST" ]; then
  NODE_ID=$(/usr/libexec/PlistBuddy -c "Print :ProgramArguments" "$PLIST" 2>/dev/null \
    | awk 'found { gsub(/^ +| +$/, ""); print; exit } /^ *--id *$/ { found = 1 }')
fi
if [ -z "$NODE_ID" ]; then
  NODE_ID="$(scutil --get LocalHostName 2>/dev/null || hostname -s)-macos"
fi

tmp=$(mktemp)
curl -fsSL "$HUB/api/worker/download/darwin-aarch64" -o "$tmp"
want=$(curl -fsSL "$HUB/api/worker/update-manifest?worker_target=darwin-aarch64&os=macos&arch=aarch64" \
  | tr ',{}' '\n\n\n' | grep '"sha256"' | head -1 | cut -d'"' -f4)
got=$(shasum -a 256 "$tmp" | cut -d' ' -f1)
if [ -z "$want" ] || [ "$want" != "$got" ]; then
  rm -f "$tmp"
  echo "worker checksum mismatch (expected $want, got $got)" >&2
  exit 1
fi
chmod 755 "$tmp"

launchctl bootout "$DOMAIN/$LABEL" 2>/dev/null || true
mv -f "$tmp" "$BIN"

xml() { printf '%s' "$1" | sed -e 's/&/\&amp;/g' -e 's/</\&lt;/g' -e 's/>/\&gt;/g'; }
args=("$BIN" --hub "$HUB" --id "$NODE_ID" --name "$NAME" --tag worker --tag macos
  --capability http --capability command --capability file --capability git --capability docker
  --capability browser --capability session --capability agentmessage
  --max-concurrent-jobs 4 --interval-seconds 5)
{
  echo '<?xml version="1.0" encoding="UTF-8"?>'
  echo '<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">'
  echo '<plist version="1.0"><dict>'
  echo "  <key>Label</key><string>$LABEL</string>"
  echo '  <key>ProgramArguments</key><array>'
  # /bin/sh 自己当看护:worker 退出(比如自动升级完)2 秒后重新运行。launchd 不会可靠地重启正常退出的任务
  # (实测会标成 pended nondemand spawn = inefficient 一直搁着),也会按代码身份拒绝启动换过的程序。
  echo '    <string>/bin/sh</string><string>-c</string><string>while :; do "$0" "$@"; sleep 2; done</string>'
  for a in "${args[@]}"; do echo "    <string>$(xml "$a")</string>"; done
  echo '  </array>'
  echo '  <key>RunAtLoad</key><true/>'
  echo '  <key>KeepAlive</key><true/>'
  echo '  <key>ProcessType</key><string>Interactive</string>'
  echo "  <key>WorkingDirectory</key><string>$(xml "$DIR")</string>"
  echo "  <key>StandardOutPath</key><string>$(xml "$DIR/worker.log")</string>"
  echo "  <key>StandardErrorPath</key><string>$(xml "$DIR/worker.err.log")</string>"
  echo '</dict></plist>'
} > "$PLIST.new"
plutil -lint "$PLIST.new" >/dev/null
mv -f "$PLIST.new" "$PLIST"

START=$(date -u +%Y-%m-%dT%H:%M:%S)
launchctl bootstrap "$DOMAIN" "$PLIST"
launchctl kickstart "$DOMAIN/$LABEL" 2>/dev/null || true

for _ in $(seq 1 30); do
  # 只认安装之后的新心跳:旧进程停掉前刚发的心跳会让节点在几十秒内仍显示在线
  hb=$(curl -fsS "$HUB/api/nodes" 2>/dev/null | sed 's/"kind":"Node"/\
/g' | grep "\"id\":\"$NODE_ID\"" | grep -o '"last_heartbeat_at":"[^"]*"' | head -1 | cut -d'"' -f4)
  if [ -n "$hb" ] && [[ "$hb" > "$START" ]]; then
    echo "$NODE_ID"
    exit 0
  fi
  sleep 2
done
echo "installed, but the Hub has not seen this Mac yet (node $NODE_ID)" >&2
tail -n 5 "$DIR/worker.err.log" >&2 2>/dev/null || true
exit 1
