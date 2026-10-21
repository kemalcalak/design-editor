#!/usr/bin/env bash
# Build this checkout and install it as /Applications/Design Editor.app.
#
# Personal-build installer: stages the pinned cargo-bundle once, builds the
# desktop binary and the `de` CLI in release mode, wraps them with
# scripts/bundle-macos.sh (ad-hoc signed), and replaces the installed app.
# A Homebrew-managed copy would be overwritten by `brew upgrade`, so the
# script refuses to run while the design-editor cask is installed.

set -euo pipefail

WS_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
export PATH="/opt/homebrew/opt/rustup/bin:$HOME/.cargo/bin:$PATH"
INSTALL_DIR="${DESIGN_EDITOR_INSTALL_DIR:-/Applications}"

if command -v brew >/dev/null 2>&1 && brew list --cask design-editor >/dev/null 2>&1; then
  echo "error: the Homebrew design-editor cask is installed; run 'brew uninstall --cask design-editor' first" >&2
  exit 1
fi

CARGO_BUNDLE_HOME="$WS_ROOT/target/cargo-bundle-host"
if [ ! -x "$CARGO_BUNDLE_HOME/bin/cargo-bundle" ]; then
  echo "==> staging pinned cargo-bundle"
  "$WS_ROOT/tools/pinned-release-tools.sh" cargo-cli cargo-bundle "$CARGO_BUNDLE_HOME"
fi

echo "==> building release binaries"
( cd "$WS_ROOT" && cargo build --release --locked -p de-host-desktop -p de-cli )

echo "==> bundling"
DESIGN_EDITOR_BINARY="$WS_ROOT/target/release/design-editor" \
DESIGN_EDITOR_CLI_BINARY="$WS_ROOT/target/release/de" \
CARGO_BUNDLE_HOME="$CARGO_BUNDLE_HOME" \
  "$WS_ROOT/scripts/bundle-macos.sh"

APP="$(find "$WS_ROOT/target/release/bundle/osx" -maxdepth 1 -name 'Design Editor.app' -print -quit)"
if [ -z "$APP" ]; then
  echo "error: bundle-macos.sh did not produce Design Editor.app" >&2
  exit 1
fi

echo "==> installing to $INSTALL_DIR"
osascript -e 'quit app "Design Editor"' >/dev/null 2>&1 || true
rm -rf "$INSTALL_DIR/Design Editor.app"
cp -R "$APP" "$INSTALL_DIR/Design Editor.app"
echo "Installed: $INSTALL_DIR/Design Editor.app"
