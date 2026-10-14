# majordomus-covers: none
# The five scenes of ADR 0160 through a real server over a fixture `majordomus init`
# installs, and the one oracle that matters for them: every number on a scene is a count
# its source answered.
#
#   1. the kiosk and the screen carry the five scenes (proof claims surface waves matrix),
#      each requiring one capability, in the one inventory;
#   2. the page names the scenes script and the drawing library, so a display that has p5
#      can draw them and one that has not can say so;
#   3. a scene is available with a model or unavailable with none and a reason;
#   4. the model of an available scene is the count of the same capability's answer, counted
#      a second time here with jq (an oracle that called the implementation would agree with
#      any mistake in it);
#   5. the scenes script is an asset the server serves, and is not a second copy of the
#      director: it registers scenes and holds no `mount`.
. "$ROOT/test/lib.sh"
RB="$(rust_bin)" || rust_bin_exit $?
[ -x "$RB" ] || { echo "    the build produced no executable at $RB"; exit 1; }
command -v jq >/dev/null 2>&1 || skip "jq absent"
MAJORDOMUS_SHARE="$ROOT/share"; export MAJORDOMUS_SHARE
unset MAJORDOMUS_HTTP_HOST

"$MJ" init >/dev/null
git add -A >/dev/null && git commit -qm install

projection() {
  grep -oE 'data-observatory="[^"]*"' "$1" | head -1 \
    | sed -e 's/^data-observatory="//' -e 's/"$//' \
          -e 's/&quot;/"/g' -e 's/&#34;/"/g' -e "s/&#39;/'/g" -e "s/&#x27;/'/g" \
          -e 's/&lt;/</g' -e 's/&gt;/>/g' -e 's/&amp;/\&/g' > "$2"
  jq -e . "$2" >/dev/null 2>&1 || { echo "    $1 carries no projection"; return 1; }
}
# answer <capability>: the output of one capability, asked here
answer() { "$RB" run "$1" --input '{}' --format json --quiet 2>/dev/null | jq '.output'; }

SRV=""
trap 'serve_down' EXIT
serve_up serve.out serve.err || exit 1
code() { curl -s --max-time 120 -o "$2" -w '%{http_code}' "$U$1"; }

[ "$(code /ui/kiosk kiosk.html)" = 200 ] || { echo "    /ui/kiosk did not answer 200"; exit 1; }
projection kiosk.html kiosk.json || exit 1

# --- 1. the five scenes, each with its source
for pair in proof:rules.report claims:evidence.report surface:quality.report waves:plan.waves matrix:distribution.status; do
  id="${pair%%:*}"; cap="${pair#*:}"
  jq -e --arg id "$id" --arg cap "$cap" '.scenes[] | select(.id == $id)
      | (.character == "semantic") and (.requires == [$cap])' kiosk.json >/dev/null \
    || { echo "    scene $id is missing, or does not require exactly $cap:"; jq -c --arg id "$id" '.scenes[] | select(.id == $id) | del(.model)' kiosk.json; exit 1; }
done

# --- 2. the page names what draws them
grep -qE 'data-observatory-companion="[^"]*observatory-kiosk\.js' kiosk.html \
  || { echo "    the page does not name the scenes script"; exit 1; }
grep -qE 'data-p5-lib="[^"]*vendor/p5\.min\.js' kiosk.html \
  || { echo "    the page does not name the drawing library"; exit 1; }

# --- 3. available with a model, or unavailable with none and a reason
jq -e '[.scenes[] | select(.id | IN("proof","claims","surface","waves","matrix"))]
    | all(if .available then .model != null
          else (.model == null and (.reason | type == "string" and length > 0)) end)' kiosk.json >/dev/null \
  || { echo "    a proof scene is available without a model or unavailable without a reason:"
       jq -c '.scenes[] | select(.id | IN("proof","claims","surface","waves","matrix")) | {id, available, reason}' kiosk.json; exit 1; }

# --- 4. the counts are the sources' own, counted again
avail() { jq -e --arg id "$1" '.scenes[] | select(.id == $id) | .available' kiosk.json >/dev/null; }
model() { jq -c --arg id "$1" '.scenes[] | select(.id == $id) | .model' kiosk.json; }
checked=0
if avail proof; then
  answer rules.report > rules.json
  want="$(jq '.rules | length' rules.json)"
  got="$(model proof | jq '.total')"
  [ "$got" = "$want" ] || { echo "    the proof lattice counts $got rule(s); rules.report lists $want"; exit 1; }
  for st in $(jq -r '[.rules[].state] | unique[]' rules.json); do
    w="$(jq --arg s "$st" '[.rules[] | select(.state == $s)] | length' rules.json)"
    g="$(model proof | jq --arg s "$st" '.states[$s]')"
    [ "$g" = "$w" ] || { echo "    the proof lattice counts $g rule(s) $st; rules.report has $w"; exit 1; }
  done
  checked=$((checked+1))
fi
if avail claims; then
  answer evidence.report > evidence.json
  want="$(jq '.claims | length' evidence.json)"
  got="$(model claims | jq '.total')"
  [ "$got" = "$want" ] || { echo "    the claim ledger counts $got claim(s); evidence.report lists $want"; exit 1; }
  sum="$(model claims | jq '[.groups[].total] | add')"
  [ "$sum" = "$want" ] || { echo "    the claim ledger's groups hold $sum claim(s), not $want"; exit 1; }
  checked=$((checked+1))
fi
if avail surface; then
  answer quality.report > quality.json
  want="$(jq '.report.operations.canonical' quality.json)"
  got="$(model surface | jq '.operations.canonical')"
  [ "$got" = "$want" ] || { echo "    the surface atlas states $got canonical operation(s); quality.report measured $want"; exit 1; }
  checked=$((checked+1))
fi
if avail waves; then
  answer plan.waves > waves.json
  want="$(jq '[.waves[].issues | length] | add' waves.json)"
  got="$(model waves | jq '[.waves[].issues] | add')"
  [ "$got" = "$want" ] || { echo "    the wave front holds $got issue(s); plan.waves lists $want"; exit 1; }
  checked=$((checked+1))
fi
if avail matrix; then
  answer distribution.status > distribution.json
  want="$(jq '.checks | length' distribution.json)"
  got="$(model matrix | jq '.checks | length')"
  [ "$got" = "$want" ] || { echo "    the release matrix draws $got check(s); distribution.status lists $want"; exit 1; }
  checked=$((checked+1))
fi

# --- 5. the scenes script is served and is not a second director
[ "$(code /ui/assets/observatory-kiosk.js kiosk.js)" = 200 ] || { echo "    the scenes script is not served"; exit 1; }
grep -q 'registry.scenes' kiosk.js || { echo "    the scenes script registers no scene"; exit 1; }
if grep -qE 'function mount|mjObservatory *= *\{' kiosk.js; then
  echo "    the scenes script holds a second director"; exit 1
fi
echo "    five proof scenes: inventory, sources, page names, $checked count(s) re-counted, script served"
