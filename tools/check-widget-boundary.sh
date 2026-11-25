#!/usr/bin/env bash
# tools/check-widget-boundary.sh — Step 1b §1.4 widget boundary invariant.
#
# Per spec §1.4: widget logic (Widget impls, layout/paint/access_node)
# lives in `crates/de-editor-ui/src/widgets/`. The forward checks that
# kept widget logic out of the browser host (`de-host-web`) went with
# that crate (#401); what remains is the reverse direction:
# de-editor-ui/src/widgets/ MUST contain the remaining OP-owned primitive
# impl files (tree, prop_row, text_input) AND each file must carry a real
# `impl Widget for X` — a stale file with the impl removed must not
# silently pass.
#
# Exit semantics:
#   0   PASS — the invariant holds.
#   1   FAIL — the de-editor-ui widget impl files are missing / empty.

set -euo pipefail

CORE_WIDGETS="crates/de-editor-ui/src/widgets"

fail_lines=()

# ---------------------------------------------------------------------
# Reverse R1: each expected widget impl file exists AND
# carries a real `impl Widget for X` line that is NOT a Rust
# line-comment.
#
# Codex B4 R2 CONCERN-3 introduced R1; R3 CONCERN refined the impl
# grep so a commented-out `// impl Widget for TextInput` stub does
# not silently satisfy the check. The stripping uses sed to drop
# `//`-prefixed content (after optional leading whitespace) before
# grepping; block comments (`/* … */`) are not handled because the
# Rust style in de-editor-ui/src/widgets/ uses line comments only.
# ---------------------------------------------------------------------
required_widgets=("tree" "prop_row" "text_input")
for w in "${required_widgets[@]}"; do
  file="${CORE_WIDGETS}/${w}.rs"
  if [ ! -f "${file}" ]; then
    fail_lines+=("Reverse R1: ${file} missing")
    continue
  fi
  uncommented="$(sed -E 's@[[:space:]]*//.*@@' "${file}")"
  if ! printf '%s\n' "${uncommented}" \
       | grep -qE 'impl(<[^>]+>)?[[:space:]]+([[:alnum:]_]+::)*Widget[[:space:]]+for[[:space:]]'; then
    fail_lines+=("Reverse R1: ${file} has no live \`impl Widget for X\` (file exists but stale, or impl is commented out)")
  fi
done

# ---------------------------------------------------------------------
# Report.
# ---------------------------------------------------------------------
if [ "${#fail_lines[@]}" -gt 0 ]; then
  printf 'FAIL: widget boundary check\n' >&2
  printf '\n' >&2
  for line in "${fail_lines[@]}"; do
    printf '%s\n' "${line}" >&2
  done
  exit 1
fi

echo "PASS: widget boundary check"
