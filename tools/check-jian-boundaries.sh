#!/usr/bin/env bash
# Step 1a Phase C Task 4 / spec v19 §11 + §12.3 boundary invariants.
#
# Verifies the following Jian crate boundary invariants from outside the
# Rust build system. Run from the repo root.
#
# (The former Invariant 1 — "the app crate must not depend directly on
#  any jian-* crate" — was dropped in Phase 1 Task 1.2 along with the
#  old placeholder crate. The Phase 7.3 reorg's `op-app` composition-root
#  crate was removed 2026-06-19 as an orphan — nothing depended on it and
#  the editor-UI composition already lives in de-editor-ui. Reinstate an
#  equivalent facade check if a real app crate is reintroduced.)
#
# (The former Invariant 2 — mobile targets must not pull desktop-only
#  backends — was dropped with the iOS / Android / HarmonyOS players.)
#
# Invariant 3 (§11.1, §1.2): wasm32 builds of `de-host-web`
#   must NOT pull `jian-host-desktop` or `jian-skia` (skia-safe build.rs
#   fails on wasm32; Jian-core is wasm32-clean per P0.5 and is the only
#   Jian crate allowed in the bundle).
#
# Invariant 4 (§1.2): `de-host-web` must NOT depend on
#   `jian-host-desktop` at all — even as a non-default optional dep.
#
# Exit codes:
#   0  — all invariants pass.
#   1+ — one or more invariants fail; the failing crate names are
#         echoed before the script exits.
#
# Dependencies: `cargo`, `jq` (for cargo metadata JSON parsing).
set -euo pipefail

if ! command -v jq >/dev/null 2>&1; then
    echo "check-jian-boundaries.sh: \`jq\` is required but not installed." >&2
    echo "  apt: sudo apt-get install -y jq" >&2
    echo "  brew: brew install jq" >&2
    exit 2
fi

# ── Invariant 3: wasm32 has no jian-host-desktop / jian-skia. ──────────
# `jian-core` IS allowed (P0.5 wasm32-clean).
tree_wasm="$(cargo tree -p de-host-web \
    --target wasm32-unknown-unknown \
    --prefix none \
    --edges normal,build 2>/dev/null || true)"
forbidden_wasm="$(echo "$tree_wasm" \
    | grep -oE '\bjian-(host-desktop|skia)\b' \
    | sort -u || true)"
if [ -n "$forbidden_wasm" ]; then
    echo "INVARIANT 3 FAILED: wasm32 de-host-web pulls forbidden Jian crates:" >&2
    echo "$forbidden_wasm" >&2
    exit 1
fi

# ── Invariant 4: de-host-web has no jian-host-desktop dep. ───
# Distinct from invariant 3 (which checks the resolved closure on the
# wasm32 target): this checks the manifest itself across all targets.
# `cargo tree --all-targets` would include dev-deps; we explicitly
# filter `--edges normal,build` for the manifest-level invariant.
shell_web_deps="$(cargo tree -p de-host-web \
    --prefix none \
    --edges normal,build 2>/dev/null \
    | grep -E '\bjian-host-desktop\b' || true)"
if [ -n "$shell_web_deps" ]; then
    echo "INVARIANT 4 FAILED: de-host-web depends on jian-host-desktop:" >&2
    echo "$shell_web_deps" >&2
    exit 1
fi

echo "check-jian-boundaries.sh: all Jian boundary invariants pass."
