#!/usr/bin/env bash
# Verify that the extension's bundled DOM extractor is byte-identical to the
# canonical Rust asset.
#
# `vendor/snapshot-extractor.js` MUST stay a verbatim copy of
# `crates/op-html/assets/snapshot-extractor.js`: that file is the contract
# `op_html::import_snapshot` parses (and whose markers
# `op-html/src/snapshot.rs` pins in a test). A symlink is not an option —
# Chrome's "Load unpacked" reads the directory as plain files — so the copy
# is checked instead.
#
# Usage:
#   scripts/check-extractor-sync.sh          # verify (exit 1 on drift)
#   scripts/check-extractor-sync.sh --fix    # re-copy the canonical asset

set -euo pipefail

script_dir=$(CDPATH= cd "$(dirname "$0")" && pwd -P)
extension_dir=$(CDPATH= cd "$script_dir/.." && pwd -P)
repo_root=$(CDPATH= cd "$extension_dir/../.." && pwd -P)

canonical="$repo_root/crates/op-html/assets/snapshot-extractor.js"
copy="$extension_dir/vendor/snapshot-extractor.js"

if [[ ! -f "$canonical" ]]; then
    printf 'extractor-sync: canonical asset not found: %s\n' "$canonical" >&2
    exit 1
fi

if [[ "${1:-}" == "--fix" ]]; then
    mkdir -p "$(dirname "$copy")"
    cp "$canonical" "$copy"
    printf 'extractor-sync: refreshed %s\n' "${copy#"$repo_root"/}"
    exit 0
fi

if [[ ! -f "$copy" ]]; then
    printf 'extractor-sync: bundled copy is missing: %s\n' "$copy" >&2
    printf 'extractor-sync: run scripts/check-extractor-sync.sh --fix\n' >&2
    exit 1
fi

if ! cmp -s "$canonical" "$copy"; then
    printf 'extractor-sync: %s has drifted from %s\n' \
        "${copy#"$repo_root"/}" "${canonical#"$repo_root"/}" >&2
    diff -u "$canonical" "$copy" >&2 || true
    printf 'extractor-sync: run scripts/check-extractor-sync.sh --fix\n' >&2
    exit 1
fi

printf 'extractor-sync: ok (%s)\n' "${copy#"$repo_root"/}"
