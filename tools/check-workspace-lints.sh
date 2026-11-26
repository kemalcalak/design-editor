#!/usr/bin/env bash
# Every member crate must inherit the root Cargo.toml's [workspace.lints].
#
# A crate that leaves out `[lints] workspace = true` silently drops those
# lints, e.g. clippy::unchecked_time_subtraction, which keeps
# `Instant - Duration` (a panic on a freshly booted Windows runner, #434) out
# of the tree. A new crate copied from an older manifest would do exactly that.
set -euo pipefail

root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$root"

missing=()
for manifest in crates/*/Cargo.toml; do
  # The `workspace = true` line must sit inside the `[lints]` table.
  if ! awk '
    /^\[/ { in_lints = ($0 ~ /^\[lints\][[:space:]]*$/) ; next }
    in_lints && /^[[:space:]]*workspace[[:space:]]*=[[:space:]]*true[[:space:]]*(#.*)?$/ { found = 1 }
    END { exit found ? 0 : 1 }
  ' "$manifest"; then
    missing+=("$manifest")
  fi
done

if ((${#missing[@]})); then
  echo "::error::crates that do not inherit the workspace lints (add '[lints]' + 'workspace = true'):"
  printf '  %s\n' "${missing[@]}"
  exit 1
fi
echo "workspace lints OK (every crates/*/Cargo.toml has [lints] workspace = true)"
