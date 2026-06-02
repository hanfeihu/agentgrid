#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
WEB_DIR="${AGENTGRID_WEB_DIR:-$ROOT/apps/agentgrid-web/dist}"
TARGET="${AGENTGRID_TARGET:-$(uname -s | tr '[:upper:]' '[:lower:]')-$(uname -m)}"

case "$TARGET" in
  darwin-arm64) TARGET="darwin-aarch64" ;;
  darwin-aarch64) TARGET="darwin-aarch64" ;;
  darwin-x86_64) TARGET="darwin-x86_64" ;;
  linux-amd64) TARGET="linux-x86_64" ;;
  linux-x86_64) TARGET="linux-x86_64" ;;
  windows-amd64) TARGET="windows-x86_64" ;;
esac

BIN_NAME="agentgrid-worker"
if [[ "$TARGET" == windows-* ]]; then
  BIN_NAME="agentgrid-worker.exe"
fi

SOURCE_BIN="${1:-$ROOT/target/release/$BIN_NAME}"
DEST_DIR="$WEB_DIR/downloads/$TARGET"
DEST_BIN="$DEST_DIR/$BIN_NAME"

if [ ! -x "$SOURCE_BIN" ]; then
  echo "worker binary not found: $SOURCE_BIN" >&2
  exit 1
fi

mkdir -p "$DEST_DIR"
cp "$SOURCE_BIN" "$DEST_BIN"
chmod 0755 "$DEST_BIN" 2>/dev/null || true
if command -v shasum >/dev/null 2>&1; then
  shasum -a 256 "$DEST_BIN" > "$DEST_BIN.sha256"
elif command -v sha256sum >/dev/null 2>&1; then
  sha256sum "$DEST_BIN" > "$DEST_BIN.sha256"
fi

SIGNATURE_FILE="$DEST_BIN.ed25519.sig"
if [[ -n "${AGENTGRID_WORKER_UPDATE_PRIVATE_KEY_FILE:-}" ]]; then
  if ! command -v openssl >/dev/null 2>&1; then
    echo "openssl is required to sign worker updates" >&2
    exit 1
  fi
  openssl pkeyutl \
    -sign \
    -rawin \
    -inkey "$AGENTGRID_WORKER_UPDATE_PRIVATE_KEY_FILE" \
    -in "$DEST_BIN" \
    | base64 | tr -d '\n' > "$SIGNATURE_FILE"
  echo >> "$SIGNATURE_FILE"
  echo "signed worker update: $SIGNATURE_FILE"
else
  rm -f "$SIGNATURE_FILE"
  echo "worker update signature skipped: set AGENTGRID_WORKER_UPDATE_PRIVATE_KEY_FILE to sign"
fi

echo "published $TARGET worker update: $DEST_BIN"
