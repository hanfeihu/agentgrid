#!/usr/bin/env bash
set -euo pipefail

REPO="${AGENTGRID_REPO:-hanfeihu/agentgrid}"
VERSION="${AGENTGRID_VERSION:-latest}"
BIN_DIR="${AGENTGRID_BIN_DIR:-/usr/local/bin}"
AGENTGRID_HOME="${AGENTGRID_HOME:-/opt/agentgrid}"
INSTALL_WORKER_SERVICE="${AGENTGRID_INSTALL_WORKER_SERVICE:-0}"
HUB_URL="${AGENTGRID_HUB_URL:-http://127.0.0.1:20181}"
NODE_ID="${AGENTGRID_NODE_ID:-$(hostname)-$(uname -s | tr '[:upper:]' '[:lower:]')}"
NODE_NAME="${AGENTGRID_NODE_NAME:-$(hostname)}"
JOIN_TOKEN="${AGENTGRID_JOIN_TOKEN:-${AG_JOIN_TOKEN:-}}"
MAX_JOBS="${AGENTGRID_MAX_CONCURRENT_JOBS:-4}"
INTERVAL="${AGENTGRID_WORKER_INTERVAL:-5}"

need() {
  command -v "$1" >/dev/null 2>&1 || {
    echo "agentgrid installer requires '$1'" >&2
    exit 1
  }
}

need curl

OS="$(uname -s)"
ARCH="$(uname -m)"
case "$OS" in
  Linux) PLATFORM_OS="linux" ;;
  Darwin) PLATFORM_OS="macos" ;;
  *) echo "Unsupported OS: $OS" >&2; exit 1 ;;
esac

case "$ARCH" in
  x86_64|amd64) PLATFORM_ARCH="x86_64" ;;
  arm64|aarch64) PLATFORM_ARCH="arm64" ;;
  *) echo "Unsupported architecture: $ARCH" >&2; exit 1 ;;
esac

if [ "$PLATFORM_OS" = "linux" ] && [ "$PLATFORM_ARCH" != "x86_64" ]; then
  echo "No official Linux ${PLATFORM_ARCH} release is published yet. Build from source for this platform." >&2
  exit 1
fi

if [ "$PLATFORM_OS" = "macos" ] && [ "$PLATFORM_ARCH" != "arm64" ]; then
  echo "No official macOS ${PLATFORM_ARCH} release is published yet. Build from source on Intel Macs." >&2
  exit 1
fi

if [ "$VERSION" = "latest" ]; then
  VERSION="$(curl -fsSL "https://api.github.com/repos/${REPO}/releases?per_page=1" | sed -n 's/.*"tag_name": *"\([^"]*\)".*/\1/p' | head -n 1)"
  if [ -z "$VERSION" ]; then
    echo "Could not resolve latest AgentGrid release for ${REPO}" >&2
    exit 1
  fi
fi

PACKAGE="agentgrid-${VERSION}-${PLATFORM_OS}-${PLATFORM_ARCH}"
ARCHIVE="${PACKAGE}.tar.gz"
URL="https://github.com/${REPO}/releases/download/${VERSION}/${ARCHIVE}"
TMP_DIR="$(mktemp -d)"
cleanup() {
  rm -rf "$TMP_DIR"
}
trap cleanup EXIT

echo "Downloading AgentGrid ${VERSION} for ${PLATFORM_OS}-${PLATFORM_ARCH}"
curl -fL "$URL" -o "$TMP_DIR/$ARCHIVE"
tar -xzf "$TMP_DIR/$ARCHIVE" -C "$TMP_DIR"

PKG_DIR="$TMP_DIR/$PACKAGE"
if [ ! -d "$PKG_DIR" ]; then
  echo "Invalid release archive: package directory not found" >&2
  exit 1
fi

as_root() {
  if [ "$(id -u)" -eq 0 ]; then
    "$@"
  else
    sudo "$@"
  fi
}

echo "Installing AgentGrid binaries to ${BIN_DIR}"
as_root mkdir -p "$BIN_DIR" "$AGENTGRID_HOME"
for bin in agentgrid agentgrid-hub agentgrid-worker agentgrid-mcp; do
  as_root install -m 0755 "$PKG_DIR/bin/$bin" "$BIN_DIR/$bin"
done

echo "Installing AgentGrid web assets and docs to ${AGENTGRID_HOME}"
as_root rm -rf "$AGENTGRID_HOME/web" "$AGENTGRID_HOME/docs" "$AGENTGRID_HOME/examples" "$AGENTGRID_HOME/scripts"
as_root cp -R "$PKG_DIR/web" "$AGENTGRID_HOME/web"
as_root cp -R "$PKG_DIR/docs" "$AGENTGRID_HOME/docs"
as_root cp -R "$PKG_DIR/examples" "$AGENTGRID_HOME/examples"
as_root cp -R "$PKG_DIR/scripts" "$AGENTGRID_HOME/scripts"

install_linux_worker_service() {
  SERVICE_USER="${AGENTGRID_WORKER_USER:-agentgrid}"
  if ! command -v systemctl >/dev/null 2>&1; then
    echo "systemd was not detected; skipping worker service install" >&2
    return
  fi
  if ! id "$SERVICE_USER" >/dev/null 2>&1; then
    as_root useradd --system --home "$AGENTGRID_HOME" --shell /usr/sbin/nologin "$SERVICE_USER"
  fi
  JOIN_ARG=""
  if [ -n "$JOIN_TOKEN" ]; then
    JOIN_ARG=" --join-token ${JOIN_TOKEN}"
  fi
  as_root tee /etc/systemd/system/agentgrid-worker.service >/dev/null <<SERVICE
[Unit]
Description=AgentGrid Worker
After=network-online.target
Wants=network-online.target

[Service]
User=${SERVICE_USER}
WorkingDirectory=${AGENTGRID_HOME}
ExecStart=${BIN_DIR}/agentgrid-worker --hub ${HUB_URL} --id ${NODE_ID} --name "${NODE_NAME}" --tag worker --tag ${PLATFORM_OS} --capability http --capability command --capability file --capability git --capability docker --capability browser --capability session --capability agentmessage --capability plugin --max-concurrent-jobs ${MAX_JOBS} --interval-seconds ${INTERVAL}${JOIN_ARG}
Restart=always
RestartSec=5

[Install]
WantedBy=multi-user.target
SERVICE
  as_root systemctl daemon-reload
  as_root systemctl enable --now agentgrid-worker.service
  systemctl status agentgrid-worker.service --no-pager || true
}

install_macos_worker_service() {
  PLIST="$HOME/Library/LaunchAgents/io.agentgrid.worker.plist"
  mkdir -p "$(dirname "$PLIST")"
  JOIN_XML=""
  if [ -n "$JOIN_TOKEN" ]; then
    JOIN_XML="    <string>--join-token</string><string>${JOIN_TOKEN}</string>"
  fi
  cat > "$PLIST" <<PLIST
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
  <key>Label</key><string>io.agentgrid.worker</string>
  <key>ProgramArguments</key>
  <array>
    <string>${BIN_DIR}/agentgrid-worker</string>
    <string>--hub</string><string>${HUB_URL}</string>
    <string>--id</string><string>${NODE_ID}</string>
    <string>--name</string><string>${NODE_NAME}</string>
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
    <string>--max-concurrent-jobs</string><string>${MAX_JOBS}</string>
    <string>--interval-seconds</string><string>${INTERVAL}</string>
${JOIN_XML}
  </array>
  <key>RunAtLoad</key><true/>
  <key>KeepAlive</key><true/>
  <key>StandardOutPath</key><string>${AGENTGRID_HOME}/worker.log</string>
  <key>StandardErrorPath</key><string>${AGENTGRID_HOME}/worker.err.log</string>
</dict>
</plist>
PLIST
  launchctl unload "$PLIST" >/dev/null 2>&1 || true
  launchctl load "$PLIST"
  launchctl list | grep io.agentgrid.worker || true
}

if [ "$INSTALL_WORKER_SERVICE" = "1" ]; then
  if [ "$PLATFORM_OS" = "linux" ]; then
    install_linux_worker_service
  elif [ "$PLATFORM_OS" = "macos" ]; then
    install_macos_worker_service
  fi
fi

cat <<DONE

AgentGrid ${VERSION} installed.

CLI:
  agentgrid --help

Run a local Hub:
  agentgrid-hub --host 127.0.0.1 --port 20181 --db ${AGENTGRID_HOME}/agentgrid-hub.db --web-dir ${AGENTGRID_HOME}/web

Run a local Worker:
  agentgrid-worker --hub http://127.0.0.1:20181 --id local-worker --name "Local Worker" --capability command --capability file --capability http

Open console:
  http://127.0.0.1:20181
DONE
