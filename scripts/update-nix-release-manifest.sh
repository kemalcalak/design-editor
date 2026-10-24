#!/usr/bin/env bash
# Point nix/release-manifest.json at a published release.
#
#   scripts/update-nix-release-manifest.sh X.Y.Z   use release vX.Y.Z
#   scripts/update-nix-release-manifest.sh         use the Cargo.toml version
#
# flake.nix's `prebuilt` / `prebuilt-cli` outputs download the release's
# design-editor-linux-x86_64.tar.gz and de-cli-linux-x86_64.tar.gz and pin them
# by the hashes in the manifest. The hashes exist only once the release is out,
# so the manifest lags Cargo.toml: run this after a release, then commit the
# manifest in a PR (nix-check builds and smoke-tests the prebuilt outputs).
#
# The hashes come from the release's SHA256SUMS.txt, re-encoded as the SRI
# strings Nix expects (sha256-<base64>); nothing is downloaded but that file.

set -euo pipefail

usage() {
    printf 'usage: scripts/update-nix-release-manifest.sh [X.Y.Z]\n' >&2
}

if [[ "$#" -gt 1 ]]; then
    usage
    exit 2
fi

script_dir=$(CDPATH='' cd "$(dirname "$0")" && pwd)
repo_root=$(CDPATH='' cd "$script_dir/.." && pwd)
manifest="$repo_root/nix/release-manifest.json"

version=${1:-$("$repo_root/scripts/workspace-version.sh")}
[[ "$version" =~ ^[0-9]+\.[0-9]+\.[0-9]+$ ]] || {
    printf 'update-nix-release-manifest: not a X.Y.Z version: %s\n' "$version" >&2
    exit 2
}

sums_url="https://github.com/kemalcalak/design-editor/releases/download/v$version/SHA256SUMS.txt"
sums=$(curl -fsSL --retry 3 "$sums_url") || {
    printf 'update-nix-release-manifest: cannot download %s\n' "$sums_url" >&2
    exit 1
}

# sha256sum lines are "<hex>  <name>" (or "<hex> *<name>" in binary mode).
sri_for() {
    local asset=$1 hex
    hex=$(printf '%s\n' "$sums" | awk -v name="$asset" '$2 == name || $2 == "*" name { print $1 }')
    [[ "$hex" =~ ^[0-9a-f]{64}$ ]] || {
        printf 'update-nix-release-manifest: v%s lists no %s in SHA256SUMS.txt\n' "$version" "$asset" >&2
        return 1
    }
    printf 'sha256-%s' "$(perl -MMIME::Base64 -e 'print encode_base64(pack("H*", $ARGV[0]), "")' "$hex")"
}

desktop_hash=$(sri_for design-editor-linux-x86_64.tar.gz)
cli_hash=$(sri_for de-cli-linux-x86_64.tar.gz)

cat > "$manifest" <<JSON
{
  "version": "$version",
  "desktopHash": "$desktop_hash",
  "cliHash": "$cli_hash"
}
JSON
printf 'update-nix-release-manifest: nix/release-manifest.json now names v%s\n' "$version"
