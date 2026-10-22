#!/usr/bin/env bash
# Regenerate every app icon from the SVG sources in assets/brand/.
#
# macOS only: renders with headless Google Chrome (transparent background)
# and packs the .icns with iconutil. Outputs:
#   crates/de-host-desktop/assets/icon.png   1024 px, macOS grid with margin
#   crates/de-host-desktop/assets/icon.icns  16-1024 px (incl. @2x)
#   crates/de-host-desktop/assets/icon.ico   16-256 px, PNG entries
#   packages/de-chrome-extension/icons/icon-{16,48,128}.png

set -euo pipefail

WS_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
BRAND="$WS_ROOT/assets/brand"
CHROME="${CHROME_BIN:-/Applications/Google Chrome.app/Contents/MacOS/Google Chrome}"
TMP="$(mktemp -d)"
trap 'rm -rf "$TMP"' EXIT

[ -x "$CHROME" ] || { echo "error: Google Chrome not found (set CHROME_BIN)" >&2; exit 1; }
command -v iconutil >/dev/null || { echo "error: iconutil (macOS) is required" >&2; exit 1; }

render() { # <svg> <size> <out.png>
  printf '<html><body style="margin:0;background:transparent"><img src="file://%s" width="%s" height="%s" style="display:block"></body></html>' \
    "$1" "$2" "$2" > "$TMP/page.html"
  "$CHROME" --headless=new --disable-gpu --hide-scrollbars \
    --default-background-color=00000000 --force-device-scale-factor=1 \
    --window-size="$2,$2" --screenshot="$3" "file://$TMP/page.html" >/dev/null 2>&1
}

MAC="$BRAND/design-editor-app-icon-macos.svg"
FULL="$BRAND/design-editor-icon-dark.svg"
DESKTOP="$WS_ROOT/crates/de-host-desktop/assets"

mkdir -p "$TMP/icon.iconset"
for s in 16 32 128 256 512; do
  render "$MAC" "$s" "$TMP/icon.iconset/icon_${s}x${s}.png"
  render "$MAC" "$((s * 2))" "$TMP/icon.iconset/icon_${s}x${s}@2x.png"
done
iconutil -c icns "$TMP/icon.iconset" -o "$DESKTOP/icon.icns"
render "$MAC" 1024 "$DESKTOP/icon.png"

for s in 16 24 32 48 64 128 256; do render "$FULL" "$s" "$TMP/win-$s.png"; done
python3 - "$TMP" "$DESKTOP/icon.ico" <<'EOF'
import struct, sys
tmp, out = sys.argv[1], sys.argv[2]
sizes = [16, 24, 32, 48, 64, 128, 256]
data = [open(f"{tmp}/win-{s}.png", "rb").read() for s in sizes]
offset = 6 + 16 * len(sizes)
entries = b""
for s, d in zip(sizes, data):
    dim = 0 if s == 256 else s  # 0 means 256 in the ICO directory
    entries += struct.pack("<BBBBHHII", dim, dim, 0, 0, 1, 32, len(d), offset)
    offset += len(d)
open(out, "wb").write(struct.pack("<HHH", 0, 1, len(sizes)) + entries + b"".join(data))
EOF

for s in 16 48 128; do
  render "$FULL" "$s" "$WS_ROOT/packages/de-chrome-extension/icons/icon-$s.png"
done
echo "icons regenerated from $BRAND"
