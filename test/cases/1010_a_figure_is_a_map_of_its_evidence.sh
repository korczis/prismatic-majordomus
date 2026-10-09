# majordomus-covers: capability:entity.show
# A figure is a map of its evidence (ADR 0122), proven over a real socket on a fixture layer.
#
# The component's unit tests prove the markup of a figure built by hand. What only a served
# page can show is that a figure built from a capability's answer keeps the grammar:
#
#   1. every element the entity page draws has an explanation beside it;
#   2. the legend names exactly the claims the drawing makes, none more and none fewer;
#   3. a backlink is drawn derived and a declared reference declared, in word and dash;
#   4. an object that names an artefact the tree does not hold is drawn missing, not
#      declared and not left out;
#   5. the page carries its data as tables, and the script and stylesheet it names are
#      served with the figure's behaviour and styles in them;
#   6. an object joined to nothing gets no figure and says so in words.
. "$ROOT/test/lib.sh"
command -v curl >/dev/null 2>&1 || skip "no curl"
RB="$(rust_bin)" || rust_bin_exit $?
[ -x "$RB" ] || { echo "    the build produced no executable at $RB"; exit 1; }
MAJORDOMUS_SHARE="$ROOT/share"; export MAJORDOMUS_SHARE
S="$(mktemp -d "${TMPDIR:-/tmp}/mj1010.XXXXXX")"
SRV=""; trap 'rm -rf "$S"; [ -n "$SRV" ] && kill "$SRV" 2>/dev/null' EXIT

"$MJ" init >/dev/null
mkdir -p .ai/repo/rules/project
rule() { # id, depends_on, tests
  cat > ".ai/repo/rules/project/$1.v1.md" <<EOF
---
id: project.$1
version: 1
kind: rule
title: The $1 rule of this fixture
description: A rule the case draws.
statement: A fixture MUST be drawn.
status: active
class: advisory
depends_on: [$2]
tags: [fixture]

x-majordomus:
  tests: [$3]
---

# Rationale

A fixture.
EOF
}
rule base "" "test/cases/0_base.sh"
rule alpha "project.base@1" "test/cases/0_absent.sh"
mkdir -p test/cases && printf '# base\n' > test/cases/0_base.sh
git add -A >/dev/null && git commit -qm layer

serve_up "$S/out.txt" "$S/err.txt" || exit 1
fail=0
note() { echo "    $1"; fail=1; }

# ---------------------------------------------------------------- alpha: names base, names a missing test
curl -s "$U/cockpit/objects/rule/project-alpha-1" > "$S/alpha.html"
grep -q 'data-mj-figure="relations"' "$S/alpha.html" || { echo "    the entity page of project.alpha draws no figure"; sed -n '1,5p' "$S/alpha.html"; exit 1; }

# 1. every drawn element explains itself
drawn="$(grep -o '<g class="mj-flow-[a-z]* [^"]*" data-k="[^"]*"' "$S/alpha.html" | sed 's/.*data-k="\([^"]*\)"/\1/' | sort -u)"
[ -n "$drawn" ] || note "the figure draws no element"
for k in $drawn; do
  grep -q "<template data-mj-note=\"$k\">" "$S/alpha.html" || note "the drawn element $k has no explanation"
done

# 2. the legend names exactly the claims drawn
made="$(grep -o 'data-claim="[a-z]*"' "$S/alpha.html" | sed 's/.*="\(.*\)"/\1/' | sort -u | tr '\n' ' ')"
named="$(grep -o 'data-mj-claim="[a-z]*"' "$S/alpha.html" | sed 's/.*="\(.*\)"/\1/' | sort -u | tr '\n' ' ')"
[ "$made" = "$named" ] || note "the drawing makes claims [$made] and the legend names [$named]"

# 3/4. the reference to base is declared and solid; alpha's own evidence is missing
grep -q 'class="mj-flow-edge mj-status--declared" data-k="e0" data-claim="declared" data-from="self" data-to="out-depends_on"' "$S/alpha.html" \
  || note "the declared reference to project.base is not drawn as a declared line out of the subject"
grep -q 'class="mj-flow-node mj-status--missing mj-flow-node--focus" data-k="self"' "$S/alpha.html" \
  || note "project.alpha names a test the tree does not hold and its box is not drawn missing"

# ---------------------------------------------------------------- base: named by alpha, so a backlink
curl -s "$U/cockpit/objects/rule/project-base-1" > "$S/base.html"
grep -q 'data-from="in-depends_on" data-to="self"' "$S/base.html" \
  || note "project.base is named by project.alpha and the figure draws no line into it"
grep -o '<g class="mj-flow-edge mj-status--derived"[^>]*data-from="in-depends_on"[^>]*>' "$S/base.html" | grep -q . \
  || note "the backlink from project.alpha is not drawn derived"
grep -q 'stroke-dasharray="6 4"' "$S/base.html" || note "the derived backlink is not dashed"
grep -q 'class="mj-flow-node mj-status--declared mj-flow-node--focus" data-k="self"' "$S/base.html" \
  || note "project.base holds every artefact it names and its box is not drawn declared"

# 5. the data, the script and the styles
grep -q '<details class="mj-figure-data" open>' "$S/alpha.html" || note "the figure carries no open data block"
grep -q '<th scope="col">Relation</th>' "$S/alpha.html" || note "the relations are not also a table"
js="$(grep -o '/cockpit/assets/flow.js[^"]*' "$S/alpha.html" | head -1)"
[ -n "$js" ] || note "the page does not load flow.js"
[ -z "$js" ] || curl -s "$U$js" | grep -q 'data-mj-figure-info' || note "flow.js is not served with the information box's behaviour"
css="$(grep -o '/cockpit/assets/cockpit.css[^"]*' "$S/alpha.html" | head -1)"
curl -s "$U$css" | grep -q 'mj-flow-box' || note "the stylesheet the page names carries no figure styles"

# 6. an object joined to nothing draws nothing, and says why in words
printf -- '---\nid: project.lonely\nversion: 1\nkind: rule\ntitle: Alone\ndescription: Joined to nothing.\nstatement: A fixture MUST be alone.\nstatus: active\nclass: advisory\ndepends_on: []\ntags: [fixture]\n---\n\n# Rationale\n\nAlone.\n' \
  > .ai/repo/rules/project/lonely.v1.md
git add -A >/dev/null && git commit -qm lonely
curl -s "$U/cockpit/objects/rule/project-lonely-1" > "$S/lonely.html"
if grep -q 'data-mj-figure=' "$S/lonely.html"; then note "an object joined to nothing drew a figure"; fi
grep -q 'This entity declares no reference.' "$S/lonely.html" || note "an object joined to nothing does not say so"

serve_down
[ "$fail" = 0 ] || exit 1
echo "    the served figure explains every element; its legend, claims, data and assets hold"
