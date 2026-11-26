#!/usr/bin/env bash
# tools/check-text-measure.sh — chrome text must be measured in the family
# it is painted in.
#
# Two ways a widget gets this wrong, both silent:
#
# 1. It measures FAMILY-BLIND. `RenderBackend::measure_text` (and its
#    `_weighted` / `_styled` siblings) resolve the backend's default typeface
#    — the bundled Roboto on native — while every chrome string is DRAWN as a
#    named run. The chrome face is wider than Roboto at the same point size,
#    so the blind call under-reports the painted width.
#
# 2. It measures in ONE NAMED FAMILY and paints in ANOTHER (#441): a label
#    fitted / centred in Manrope (`text_metrics::CHROME_FONT_FAMILY`) but
#    drawn as a literal `"system-ui"` run (SF Pro on macOS). Both calls are
#    family-aware, so rule 1 cannot see it.
#
# Nothing errors when either happens. An ellipsizer believes a string fits
# and emits no `…`, so the content clip shears the last glyph in half; a
# centred label lands off centre; a tooltip bubble sized as
# `measured + padding` is born too narrow for its own text; a caret drifts
# off the glyph being edited. None of it reproduces in CI by eye, which is
# what makes this worth a build gate.
#
# The sanctioned path is `crate::widgets::text_metrics` (`measure_chrome` /
# `fit_chrome` / `centered_text_x` / `measure_in_family`) or
# `label_widths::LabelWidths`, with the family named by a constant that the
# paint call names too. The tests in `widgets/chrome_family_paint_tests/`
# check that paint and measurement agree per surface; this gate keeps the
# literals that let them drift apart out of widget code.
#
# Rules (widget code = crates/de-editor-ui/src/widgets):
#   R1  no family-blind measurement: `.measure_text(`, `.measure_text_weighted(`,
#       `.measure_text_styled(`.
#   R2  no `"system-ui"` literal: chrome paints in CHROME_FONT_FAMILY.
#   R3  no string-literal font family in a paint run (`TextLayout::single_run`)
#       or a family-aware measurement (`measure_text_family[_styled]`,
#       `measure_in_family[_weighted]`, `fit_in_family`, `LabelWidths::styled`
#       / `wrap`): a family is a named constant, so the run and its
#       measurement can be seen to name the same one. `""` (the backend's
#       default face, used by canvas mock-widget labels that are not
#       measured in a family) is allowed.
#
# Exit semantics:
#   0   PASS — no violation in widget code.
#   1   FAIL — a rule above was broken; each hit is listed.

set -euo pipefail

WIDGETS="crates/de-editor-ui/src/widgets"
status=0

report() {
  local title="$1" hits="$2" help="$3"
  printf 'FAIL: %s\n\n%s\n\n%b\n\n' "${title}" "${hits}" "${help}" >&2
  status=1
}

# ---------------------------------------------------------------------
# R1 — family-blind measurement.
#
# Files that legitimately name the blind calls:
#   text_metrics.rs          — defines the sanctioned wrappers; its tests
#                              compare blind against family-aware.
#   text_input_backend.rs    — a decorating `RenderBackend` that must forward
#                              every trait method, blind ones included.
#   test_family_gap_backend.rs / test_capture_backend.rs
#                            — test backends implementing the trait.
#   text_metrics_paint_tests.rs
#                            — the cross-panel guard's negative control
#                              paints a deliberately blind fitter.
#   canvas_viewport_widget.rs
#                            — canvas mock-widget tab labels: document
#                              content painted in the default face (`""`),
#                              so the default-face measurement IS the
#                              painted family.
R1_ALLOWED='^crates/de-editor-ui/src/widgets/(text_metrics|text_metrics_paint_tests|text_input_backend|test_family_gap_backend|test_capture_backend|canvas_viewport_widget)\.rs:'

r1_hits="$(grep -RInE '\.[[:space:]]*measure_text(_weighted|_styled)?[[:space:]]*\(' "${WIDGETS}" 2>/dev/null \
  | grep -vE "${R1_ALLOWED}" \
  || true)"

if [ -n "${r1_hits}" ]; then
  report "family-blind text measurement in widget code" "${r1_hits}" \
'RenderBackend::measure_text[_weighted|_styled] resolves the backend default
font, not the family these runs are painted with. Measure through
crate::widgets::text_metrics instead:

  cx.backend.measure_text(s, size)
    -> text_metrics::measure_chrome(cx.backend, s, size)
  cx.backend.measure_text_weighted(s, size, weight)
    -> text_metrics::measure_chrome_weighted(cx.backend, s, size, weight)
  ellipsize_to_width(s, w, |t| cx.backend.measure_text(t, size))
    -> text_metrics::fit_chrome(cx.backend, s, w, size)
  rect.origin.x + (rect.size.x - measured) / 2.0
    -> text_metrics::centered_text_x(cx.backend, s, size, rect)

For a run drawn in some other family (a jian component painting in
"Inter", a monospace readout), name that family:
  text_metrics::measure_in_family / fit_in_family.'
fi

# ---------------------------------------------------------------------
# R2 — the "system-ui" literal.
#
#   chrome_family_paint_tests/mod.rs
#     — the negative controls paint a deliberately mismatched
#       "system-ui" run to prove the paint/measure check can fail.
R2_ALLOWED='^crates/de-editor-ui/src/widgets/chrome_family_paint_tests/mod\.rs:'

r2_hits="$(grep -RIn '"system-ui"' "${WIDGETS}" 2>/dev/null \
  | grep -vE "${R2_ALLOWED}" \
  || true)"

if [ -n "${r2_hits}" ]; then
  report 'chrome painted or measured in a literal "system-ui"' "${r2_hits}" \
'The chrome font is Manrope (text_metrics::CHROME_FONT_FAMILY), and every
chrome measurer (measure_chrome, fit_chrome, centered_text_x, LabelWidths)
measures in it. A run drawn as "system-ui" paints SF Pro on macOS, so it is
fitted, centred and hit-tested against a face it is not drawn in (#441).

  TextLayout::single_run(text, "system-ui", ...)
    -> TextLayout::single_run(text, text_metrics::CHROME_FONT_FAMILY, ...)

A run that is deliberately not chrome (document text) names its own
constant, e.g. de_editor_core::text_defaults::FONT_FAMILY.'
fi

# ---------------------------------------------------------------------
# R3 — string-literal families in paint runs and family-aware measurement.
#
# Test modules build fixtures with ad-hoc family names ("Wide Sans",
# "Arial", …) to model a font gap; they are not chrome. Test files are
# skipped by name; the one inline `#[cfg(test)]` fixture that does it in a
# non-test file is listed.
#   canvas_text_edit.rs — its tests module measures a made-up "Wide Sans".
R3_SKIP_FILE='(_tests\.rs|/[a-z_]*_tests/[^/]*\.rs|/test_[a-z_]*\.rs|/canvas_text_edit\.rs)$'

r3_files=()
while IFS= read -r file; do
  if ! printf '%s\n' "${file}" | grep -qE "${R3_SKIP_FILE}"; then
    r3_files+=("${file}")
  fi
done < <(find "${WIDGETS}" -name '*.rs' | sort)

# For each call, split its top-level arguments (strings and nested brackets
# respected) and report the family argument when it is a non-empty string
# literal. `name => [arg index, required arg count or 0 for any]`.
r3_hits="$(perl -0777 -ne '
  my %family_arg = (
    "single_run"                 => [1, 0],
    "measure_text_family"        => [2, 0],
    "measure_text_family_styled" => [2, 0],
    "measure_in_family"          => [3, 0],
    "measure_in_family_weighted" => [3, 0],
    "fit_in_family"              => [4, 0],
    "styled"                     => [2, 4],
    "wrap"                       => [3, 5],
  );
  my $src = $_;
  while ($src =~ /(?<!fn )\b(single_run|measure_text_family_styled|measure_text_family|measure_in_family_weighted|measure_in_family|fit_in_family|styled|wrap)\s*\(/g) {
    my ($name, $start) = ($1, pos($src));
    my ($depth, $i, $cur) = (1, $start, "");
    my @args;
    while ($i < length($src) && $depth > 0) {
      my $c = substr($src, $i, 1);
      if ($c eq "\"") {
        my $j = $i + 1;
        while ($j < length($src)) {
          my $d = substr($src, $j, 1);
          if ($d eq "\\") { $j += 2; next; }
          last if $d eq "\"";
          $j++;
        }
        $cur .= substr($src, $i, $j - $i + 1);
        $i = $j + 1;
        next;
      }
      if ($c =~ /[\(\[\{]/) { $depth++; }
      elsif ($c =~ /[\)\]\}]/) { $depth--; last if $depth == 0; }
      elsif ($c eq "," && $depth == 1) { push @args, $cur; $cur = ""; $i++; next; }
      $cur .= $c;
      $i++;
    }
    push @args, $cur if $cur =~ /\S/;
    my ($index, $count) = @{ $family_arg{$name} };
    next if $count && scalar(@args) != $count;
    next if $index >= scalar(@args);
    my $arg = $args[$index];
    $arg =~ s/^\s+|\s+$//g;
    next unless $arg =~ /^"(.+)"$/s;
    my $line = 1 + (substr($src, 0, $start) =~ tr/\n//);
    print "$ARGV:$line: $name(.., \"$1\", ..)\n";
  }
' "${r3_files[@]}" || true)"

if [ -n "${r3_hits}" ]; then
  report "string-literal font family in a widget paint run or measurement" "${r3_hits}" \
'Name the family through a constant that the paint run and its measurement
both use, so they cannot drift apart:

  chrome                    text_metrics::CHROME_FONT_FAMILY
  hex / code readouts       text_metrics::MONO_FONT_FAMILY
  a jian component s face   a local const naming it, with a comment
                            (e.g. SELECT_TRIGGER_FAMILY in text_metrics.rs)'
fi

if [ "${status}" -ne 0 ]; then
  exit "${status}"
fi

echo "PASS: text measurement check"
