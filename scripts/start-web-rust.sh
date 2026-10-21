#!/usr/bin/env bash
# Build and serve the Rust CanvasKit web editor without Docker.
#
# This is the script-publish counterpart to Dockerfile.web-rust. It builds the
# deployable wasm bundle, builds the GUI-free de-host-web-server daemon, then
# serves the browser editor through `--serve-web`.
set -euo pipefail

cd "$(dirname "$0")/.."

PORT="${DESIGN_EDITOR_SERVE_PORT:-3100}"
HOST="${DESIGN_EDITOR_SERVE_HOST:-127.0.0.1}"
DOC_PATH="${1:-}"

if [ "${DESIGN_EDITOR_SKIP_WASM_BUILD:-0}" != "1" ]; then
  bash tools/check-wasm-bundle.sh
fi

if [ "${DESIGN_EDITOR_SKIP_SERVER_BUILD:-0}" != "1" ]; then
  cargo build -p de-host-web-server --release
fi

SERVER_BIN="${DESIGN_EDITOR_WEB_SERVER_BIN:-target/release/de-host-web-server}"
if [ ! -x "$SERVER_BIN" ]; then
  echo "error: server binary is not executable: $SERVER_BIN" >&2
  exit 1
fi

export DESIGN_EDITOR_WEB_BUNDLE_DIR="${DESIGN_EDITOR_WEB_BUNDLE_DIR:-$PWD/crates/de-host-web/pkg}"
export DESIGN_EDITOR_CANVASKIT_DIR="${DESIGN_EDITOR_CANVASKIT_DIR:-$PWD/crates/de-host-web/assets/canvaskit}"

echo "Design Editor Rust web editor: http://${HOST}:${PORT}/"
echo "bundle: ${DESIGN_EDITOR_WEB_BUNDLE_DIR}"
echo "canvaskit: ${DESIGN_EDITOR_CANVASKIT_DIR}"

if [ -n "$DOC_PATH" ]; then
  exec "$SERVER_BIN" --serve-web "$PORT" "$DOC_PATH" --host "$HOST"
else
  exec "$SERVER_BIN" --serve-web "$PORT" --host "$HOST"
fi
