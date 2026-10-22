#!/usr/bin/env bash
# Build one desktop release target with required ABI 3 auth inputs, then
# prove Cargo did not silently select the auth stub.

set -euo pipefail

release_target=${DESIGN_EDITOR_RELEASE_TARGET:-}
export -n release_target

script_dir=$(CDPATH='' cd "$(dirname "$0")" && pwd)
repo_root=$(CDPATH='' cd "$script_dir/.." && pwd)

if [[ "$#" -ne 0 ]]; then
    printf 'usage: DESIGN_EDITOR_RELEASE_TARGET=<triple> %s\n' "$0" >&2
    exit 2
fi
[[ -n "$release_target" ]] || {
    printf 'error: DESIGN_EDITOR_RELEASE_TARGET is required\n' >&2
    exit 2
}
[[ -z ${FORCE_SKIA_BINARIES_DOWNLOAD:-} && ${SKIA_BINARIES_URL:-} == file://* ]] || {
    printf 'error: release builds require the verified local Skia binary cache\n' >&2
    exit 2
}
[[ -z ${FORCE_SKIA_BINARIES_DOWNLOAD+x} ]] || {
    printf 'error: FORCE_SKIA_BINARIES_DOWNLOAD must be unset for release builds\n' >&2
    exit 2
}
command -v cargo >/dev/null 2>&1 || {
    printf 'error: cargo is unavailable\n' >&2
    exit 1
}

cd "$repo_root"
cargo clean -p de-auth-bridge --target "$release_target" --release
cargo clean -p skia-bindings --target "$release_target" --release
"$repo_root/tools/pinned-release-tools.sh" verify-skia \
    desktop "$release_target" "$SKIA_BINARIES_URL"
cargo build \
    -p de-host-desktop -p de-cli -p de-host-web-server \
    --target "$release_target" --release --locked \
    --features \
      de-host-desktop/pinned-skia-binaries,de-host-web-server/pinned-skia-binaries
OP_AUTH_CARGO_TARGET=$release_target tools/check-op-auth-cargo-build.sh
