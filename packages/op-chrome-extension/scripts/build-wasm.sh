#!/usr/bin/env bash
# Build the extension's Rust logic core and install it into the extension
# directory.
#
# Mirrors the workspace's other wasm gates (`tools/check-wasm-bundle.sh` for
# op-host-web, `crates/op-web-sdk/tools/build-wasm.sh` for the web SDK):
# cargo build → wasm-bindgen → wasm-opt, no wasm-pack (0.13.1 is broken on
# stable Rust, and pinning the CLI from Cargo.lock is the only version story
# that holds).
#
# Steps:
#   1. Verify prerequisites: cargo, wasm-bindgen. wasm-opt and node are
#      optional here — see below.
#   2. cargo build -p op-chrome-extension-core \
#        --target wasm32-unknown-unknown --release
#   3. wasm-bindgen --target web → crates/op-chrome-extension-core/pkg/
#   4. Assert 0 env.* imports (LinkError guard) when node is available.
#   5. wasm-opt -Oz when binaryen is installed; skipped with a notice
#      otherwise. Unlike the web bundles this artifact is loaded from local
#      disk by the popup, never over a network, so size is a nicety rather
#      than a gate — there is no gzip ceiling to fail.
#   6. Guard the freshly generated shim (no eval, no remote code), then copy
#      it and the .wasm into `wasm/` inside the extension. The guard runs on
#      the file in pkg/ BEFORE the copy, so a shim that fails it can never
#      reach the directory Chrome loads — a guard applied after the copy
#      would fail the build while leaving the rejected file installed.
#   7. Assert the service worker's static import graph carries no dynamic
#      `import(`, which ServiceWorkerGlobalScope forbids. Runs last because
#      the graph includes the shim installed by step 6.
#
# The extension loads the copy in `wasm/`, which is a build product and is
# gitignored (same policy as `crates/op-host-web/pkg/` and
# `packages/op-web-sdk/wasm/`). Run this before "Load unpacked", and again
# after changing anything under `crates/op-chrome-extension-core/`.
#
# Exit semantics:
#   0   build succeeded.
#   1   a check failed — the message names which one.
#   2   a required prerequisite is missing.

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
EXTENSION_DIR="$(cd "${SCRIPT_DIR}/.." && pwd)"
WORKSPACE_ROOT="$(cd "${EXTENSION_DIR}/../.." && pwd)"

CRATE_NAME="op-chrome-extension-core"
# wasm-bindgen names its output after the `[lib] name` in Cargo.toml.
LIB_NAME="op_chrome_extension_core"

CRATE_DIR="${WORKSPACE_ROOT}/crates/${CRATE_NAME}"
PKG_DIR="${CRATE_DIR}/pkg"
DEST_DIR="${EXTENSION_DIR}/wasm"
TARGET_WASM="${WORKSPACE_ROOT}/target/wasm32-unknown-unknown/release/${LIB_NAME}.wasm"
WASM_RAW="${PKG_DIR}/${LIB_NAME}_bg.wasm"
WASM_OPT="${PKG_DIR}/${LIB_NAME}_bg.opt.wasm"

# rustc-emitted WebAssembly feature flags. Kept as CANDIDATES because binaryen
# versions disagree on their spelling: newer binaryen (v117+) split
# bulk-memory into `bulk-memory` + `bulk-memory-opt`, while older releases
# predate the split and their single `--enable-bulk-memory` already covers
# memory.copy/fill. Passing an unknown flag hard-fails wasm-opt, so the
# invocation below filters this list down to what the installed one knows.
WASM_OPT_CANDIDATE_FEATURES=(
  --enable-bulk-memory
  --enable-bulk-memory-opt
  --enable-nontrapping-float-to-int
)

step() { printf '\n[step %d/7] %s\n' "$1" "$2"; }
fail() { printf 'FAIL: %s\n' "$1" >&2; exit 1; }
need() { command -v "$1" >/dev/null 2>&1 || { printf 'missing prerequisite: %s\n' "$1" >&2; exit 2; }; }
have() { command -v "$1" >/dev/null 2>&1; }

step 1 "Verify prerequisites"
need cargo
need wasm-bindgen
if ! rustup target list --installed 2>/dev/null | grep -qx 'wasm32-unknown-unknown'; then
  printf 'missing prerequisite: rustup target add wasm32-unknown-unknown\n' >&2
  exit 2
fi
printf '  ✓ cargo, wasm-bindgen (%s), wasm32-unknown-unknown\n' "$(wasm-bindgen --version)"

step 2 "cargo build -p ${CRATE_NAME} --target wasm32-unknown-unknown --release"
cd "${WORKSPACE_ROOT}"
cargo build -p "${CRATE_NAME}" --target wasm32-unknown-unknown --release

step 3 "wasm-bindgen --target web → ${PKG_DIR}/"
# Clear the previous run's output first. wasm-bindgen overwrites the files it
# emits but never removes ones it no longer emits, so a renamed [lib] name or
# a dropped export would otherwise leave a stale shim in pkg/ that step 6
# could still install. `_bg.opt.wasm` in particular is written by step 5 and
# is not a wasm-bindgen output at all.
rm -rf "${PKG_DIR}"
mkdir -p "${PKG_DIR}"
wasm-bindgen --target web --out-dir "${PKG_DIR}" "${TARGET_WASM}"

step 4 "Verify 0 env.* imports (LinkError guard)"
if have node; then
  env_count="$(node -e '
const fs = require("fs");
const buf = fs.readFileSync(process.argv[1]);
WebAssembly.compile(buf).then(mod => {
  const env = WebAssembly.Module.imports(mod).filter(i => i.module === "env");
  console.log(env.length);
}).catch(e => { console.error("compile failed:", e); process.exit(1); });
' "${WASM_RAW}")"
  if [ "${env_count}" != "0" ]; then
    fail "env.* import count = ${env_count} (must be 0); a dep pulled non-wasm-clean code"
  fi
  printf '  ✓ 0 env.* imports\n'
else
  printf '  – node not installed; skipping the import guard\n'
fi

step 5 "wasm-opt -Oz (optional)"
if have wasm-opt; then
  wasm_opt_help="$(wasm-opt --help 2>&1 || true)"
  WASM_OPT_FEATURES=()
  for flag in "${WASM_OPT_CANDIDATE_FEATURES[@]}"; do
    if printf '%s\n' "${wasm_opt_help}" | grep -qF -- "${flag}"; then
      WASM_OPT_FEATURES+=("${flag}")
    fi
  done
  wasm-opt "${WASM_OPT_FEATURES[@]}" -Oz "${WASM_RAW}" -o "${WASM_OPT}"
  # Replace the raw wasm in place so pkg/ is ready to install as-is.
  cp "${WASM_OPT}" "${WASM_RAW}"
  printf '  ✓ optimised (%s bytes)\n' "$(wc -c <"${WASM_RAW}" | tr -d ' ')"
else
  printf '  – wasm-opt (binaryen) not installed; shipping the unoptimised module\n'
fi

step 6 "Guard + install into ${DEST_DIR#"${WORKSPACE_ROOT}"/}/"
# The shim must not reach for anything outside the extension package: MV3
# forbids remote code, and the popup's CSP would block it anyway. `--target
# web` never emits either of these, so this is a regression guard on the
# generator, not a fix for it.
#
# It runs against pkg/, not against the installed copy: "refusing to install
# it" has to mean the extension directory is left exactly as it was. Guarding
# after `cp` would fail the build with the rejected shim already in place,
# and the next "Load unpacked" would run it.
if grep -qE '\beval\(|new Function\(|https?://' "${PKG_DIR}/${LIB_NAME}.js"; then
  fail "the generated shim contains eval / remote code — refusing to install it"
fi

mkdir -p "${DEST_DIR}"
# Remove the previous install before writing the new one, for the same reason
# step 3 clears pkg/: nothing stale may survive a rename.
rm -f "${DEST_DIR}/${LIB_NAME}.js" "${DEST_DIR}/${LIB_NAME}_bg.wasm"
# Only the two files the popup loads. The generated `.d.ts` files are for TS
# consumers, and there are none here.
cp "${PKG_DIR}/${LIB_NAME}.js" "${DEST_DIR}/${LIB_NAME}.js"
cp "${WASM_RAW}" "${DEST_DIR}/${LIB_NAME}_bg.wasm"
printf '  ✓ %s.js + %s_bg.wasm installed, no eval or remote fetch\n' "${LIB_NAME}" "${LIB_NAME}"

step 7 "Verify the service worker imports no module dynamically"
# `import()` is disallowed on ServiceWorkerGlobalScope by the HTML spec
# (w3c/ServiceWorker#1356), and nothing else catches a violation: the
# extension loads, the popup works, and only the pick flow fails, at runtime,
# inside the worker. See scripts/check-sw-imports.mjs.
if have node; then
  node "${EXTENSION_DIR}/scripts/check-sw-imports.mjs" || \
    fail "the service worker's import graph contains a dynamic import()"
else
  printf '  – node not installed; skipping the service-worker import guard\n'
fi

printf '\nop-chrome-extension core built. Reload the extension in chrome://extensions.\n'
