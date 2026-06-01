#!/usr/bin/env bash
set -euo pipefail

HUB_URL="${AGENTGRID_HUB_URL:-http://chenqi.tminos.com:20080/agentgrid}"
NODE_ID="${AGENTGRID_NODE_ID:-$(hostname)-$(uname -s)}"
NODE_NAME="${AGENTGRID_NODE_NAME:-$(hostname)}"
INTERVAL="${AGENTGRID_WORKER_INTERVAL:-10}"
INSTALL_DIR="${AGENTGRID_WORKER_DIR:-$HOME/agentgrid-worker}"
JOIN_TOKEN="${AGENTGRID_JOIN_TOKEN:-${AG_JOIN_TOKEN:-}}"
JOIN_TOKEN_ARG=""
if [ -n "$JOIN_TOKEN" ]; then
  JOIN_TOKEN_ARG=" --join-token '$JOIN_TOKEN'"
fi

mkdir -p "$INSTALL_DIR/bin"

if ! command -v cargo >/dev/null 2>&1; then
  curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs -o /tmp/rustup-init.sh
  RUSTUP_DIST_SERVER="${RUSTUP_DIST_SERVER:-https://rsproxy.cn}" \
  RUSTUP_UPDATE_ROOT="${RUSTUP_UPDATE_ROOT:-https://rsproxy.cn/rustup}" \
    sh /tmp/rustup-init.sh -y --profile minimal
fi

. "$HOME/.cargo/env"

if [ ! -d "$INSTALL_DIR/src/.git" ] && [ ! -f "$INSTALL_DIR/src/Cargo.toml" ]; then
  mkdir -p "$INSTALL_DIR/src"
fi

echo "Please copy or rsync AgentGrid source into: $INSTALL_DIR/src"
echo "Then run:"
echo "  cd $INSTALL_DIR/src && cargo build --release -p agentgrid-worker-app"
echo "  cp target/release/agentgrid-worker $INSTALL_DIR/bin/agentgrid-worker"
echo
echo "Start command:"
echo "  nohup $INSTALL_DIR/bin/agentgrid-worker --hub '$HUB_URL' --id '$NODE_ID' --name '$NODE_NAME' --tag worker --tag linux --capability http --capability command --capability file --capability git --capability docker --capability browser --capability session --capability agentmessage --capability plugin --interval-seconds '$INTERVAL'$JOIN_TOKEN_ARG > $INSTALL_DIR/worker.log 2>&1 &"
