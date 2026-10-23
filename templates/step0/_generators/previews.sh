#!/usr/bin/env bash
# Render scene-template previews: one PNG per frame (scale 2) plus a whole-page
# overview (scale 1). scene_preview_cards.py bakes the card thumbnails from these.
set -euo pipefail
R=$(cd "$(dirname "$0")/../../.." && pwd)
BIN=$R/target/release/design-editor
OUT=$R/templates/step0/previews
TMP=$(mktemp -d)
mkdir -p "$OUT"

DEFAULT_TEMPLATES=(screenshot-tutorial knowledge-carousel before-after slide-deck \
         knowledge-card-vertical knowledge-card-square \
         pitch-deck-dark lecture-deck-light \
         minimal-keynote gradient-tech \
         saas-landing-orange product-landing-light \
         punch-quote-card journal-checklist-card \
         data-report-infographic steps-flow-infographic \
         pitfall-list-infographic \
         spine-culture-card metric-single-card quote-frame-card \
         daily-sign-card price-tier-card notice-board-card \
         milestone-timeline-infographic concept-contrast-infographic \
         ranking-board-infographic faq-thread-infographic \
         data-story-infographic challenge-tracker-infographic \
         ecosystem-map-infographic \
         do-dont-comparison \
         myth-truth-comparison \
         pricing-tiers-comparison \
         scenario-guide-comparison \
         spec-table-comparison \
         three-way-comparison \
         time-shift-comparison \
         tradeoff-scale-comparison \
         version-diff-comparison \
         app-onboarding-triptych \
         diy-blueprint-guide \
         photo-composition-tutorial \
         recipe-four-step \
         skincare-routine-cards \
         software-step-tutorial \
         storage-makeover-steps \
         weekly-report-lesson \
         workout-breakdown-guide \
         bookreview-silk-carousel \
         cityguide-film-carousel \
         datareport-grid-carousel \
         opinion-longform-carousel \
         qa-chalkboard-carousel \
         story-night-carousel \
         toolkit-notebook-carousel \
         tutorial-journal-carousel \
         yearreview-mineral-carousel \
         sounding-navy-deck \
         tidemark-slate-deck \
         banxin-rule-deck \
         gridpaper-graphite-deck \
         dossier-linen-deck \
         ledger-tick-deck \
         event-poster-deck \
         brand-concept-sheet \
         logo-qa-board \
         event-invitation-card livestream-teaser-card hiring-poster-card \
         course-enroll-card conference-agenda-card product-launch-card \
         annual-report-card music-fest-poster-card book-club-invite-card \
         ai-support-pitch-deck quarterly-review-deck quicksort-lecture-deck \
         compound-effect-card analytics-metric-card)

if [ "$#" -gt 0 ]; then
  templates=("$@")
else
  templates=("${DEFAULT_TEMPLATES[@]}")
fi

for t in "${templates[@]}"; do
  # The documents themselves are the shipped assets and the source of truth
  # (embedded by the scene template catalogue); edit them directly. This
  # directory keeps only the full-resolution renders made from them.
  op=$R/crates/de-editor-core/assets/scene_templates/$t.de
  rm -rf "$TMP/$t"; mkdir -p "$TMP/$t"
  DESIGN_EDITOR_RENDER_MARGIN=0 "$BIN" --render-shots "$op" "$TMP/$t" 2 >/dev/null

  # Rename <node-id>.png to <template>-NN.png in the .de children order
  python3 - "$op" "$TMP/$t" "$OUT" "$t" <<'PY'
import json, shutil, sys, pathlib
op, shots, out, name = sys.argv[1:5]
doc = json.load(open(op))
kids = doc.get("children") or doc["pages"][0]["children"]
for i, n in enumerate(kids, 1):
    src = pathlib.Path(shots) / f"{n['id']}.png"
    dst = pathlib.Path(out) / (f"{name}.png" if len(kids) == 1
                               else f"{name}-{i:02d}.png")
    shutil.copyfile(src, dst)
    print(f"  {dst.name}  <- {n['name']}")
PY

  # Multi-frame templates also get a whole-page overview (export_item composites
  # every top-level frame onto one canvas)
  n=$(python3 -c "import json;d=json.load(open('$op'));print(len(d.get('children') or d['pages'][0]['children']))")
  if [ "$n" -gt 1 ]; then
    # `--mcp` writes the document back to disk normalised, so it must never
    # open the shipped asset: work on a throwaway copy.
    cp "$op" "$TMP/$t/doc.de"
    printf '%s\n' \
      '{"jsonrpc":"2.0","id":1,"method":"initialize","params":{}}' \
      '{"jsonrpc":"2.0","id":2,"method":"tools/call","params":{"name":"export_item","arguments":{"itemId":"page-1","format":"png","scale":1}}}' \
    | "$BIN" --mcp "$TMP/$t/doc.de" 2>/dev/null | tail -1 \
    | python3 -c "
import sys, json, base64
t = json.loads(json.load(sys.stdin)['result']['content'][0]['text'])
open('$OUT/$t-overview.png','wb').write(base64.b64decode(t['bytes_base64']))
print('  $t-overview.png  <- all frames')
"
  fi
done
rm -rf "$TMP"
echo "--- previews ---"
ls -la "$OUT"
