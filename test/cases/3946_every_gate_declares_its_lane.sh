# majordomus-covers: none
# Every gate declares its lane (I3946, slice S2 of I2350, ADR 0186).
#
# ADR 0186 splits one validation into lanes: feedback, which a pull request's required verdict
# needs green, proof, which judges the combined top before it lands, and release. The model
# (.ai/repo/ci/gates.yaml) now gives every gate a `lane` and declares the `tiers` that run
# them. A lane the planner cannot read is a gate whose verdict nobody knows where to wait for,
# so both readers of the model refuse the same three things in the same words: a gate without
# a lane once tiers are declared, a lane that is none of the three, and a tier naming a lane
# that does not exist. scripts/ci-plan refuses them under --check; the executable refuses to
# load the model (src/gates/model.rs) and says why where gates.model reports it unreadable.
# The lanes select nothing yet: the plan of every change is the plan without them.
. "$ROOT/test/lib.sh"
BIN="$(rust_bin)" || rust_bin_exit $?
PLAN="$ROOT/scripts/ci-plan"
M="$ROOT/.ai/repo/ci/gates.yaml"

# what each reader says of a model, with its own prefix taken off: ci-plan's stderr under
# --check (into $sh, its exit into $sh_st), and the reason the executable gives for not
# reading it. sh_says runs in this shell, never in a substitution, so the exit survives.
# The executable reads the model of the repository it is pointed at, so the fixture is made a
# repository of the layer and the copy goes there. A managed repository carries no share/ of
# its own, so the executable is told where the tool's is, as case 131 tells it.
"$MJ" init >/dev/null
export MAJORDOMUS_SHARE="$ROOT/share"
mkdir -p .ai/repo/ci
sh_says() { sh_st=0; "$PLAN" --model "$1" --check > /dev/null 2> sh.err || sh_st=$?; sh="$(sed 's/^ci-plan: //' sh.err)"; }
rs_says() {
  cp "$1" .ai/repo/ci/gates.yaml
  "$BIN" run gates.model --input '{}' --quiet --format json --repo . 2>/dev/null > rs.json \
    || { echo "    gates.model did not answer for $1"; exit 1; }
  jq -r '.output.unreadable // empty' rs.json | sed 's|^\.ai/repo/ci/gates\.yaml: ||'
}

# ---------------------------------------------------------------- the repository's own model
st=0; "$PLAN" --check > own.out 2>&1 || st=$?
[ "$st" = 0 ] || { echo "    ci-plan --check refuses the repository's own model ($st)"; cat own.out; exit 1; }
grep -q ' 4 tier(s), lanes [0-9]* feedback, [0-9]* proof, [0-9]* release, ' own.out \
  || { echo "    ci-plan --check does not count the tiers and lanes"; cat own.out; exit 1; }
for t in local pull-request integration release; do
  grep -q "^  $t: \[" "$M" || { echo "    the model declares no tier $t"; exit 1; }
done
[ -z "$(rs_says "$M")" ] || { echo "    the executable cannot read the repository's own model: $(rs_says "$M")"; exit 1; }
# every gate is in a lane, as each reader exposes it
sh_unlaned="$("$PLAN" --full --on-demand | jq -r '.gates[] | select(.lane == null) | .id')"
rs_unlaned="$(jq -r '.output.gates[] | select(.lane == null) | .id' rs.json)"
[ -z "$sh_unlaned$rs_unlaned" ] || { echo "    gates without a lane: ci-plan: $sh_unlaned; executable: $rs_unlaned"; exit 1; }
n_gates="$(awk '/^gates:$/ { g = 1; next } /^[a-z]/ { g = 0 } g && /^  - id: / { n++ } END { print n + 0 }' "$M")"; n_rs="$(jq '.output.gates | length' rs.json)"
[ "$n_rs" -gt 0 ] && [ "$(jq '[.output.gates[] | select(.lane != null)] | length' rs.json)" = "$n_rs" ] \
  || { echo "    the executable reports $n_rs gates, not every one with a lane"; exit 1; }
[ "$("$PLAN" --full --on-demand | jq '[.gates[] | select(.lane != null)] | length')" -ge "$n_rs" ] \
  || { echo "    ci-plan reports fewer laned gates than the executable"; exit 1; }
# the two readers agree on every gate's lane
"$PLAN" --full --on-demand | jq -r '.gates[] | "\(.id) \(.lane)"' | LC_ALL=C sort > sh.lanes
jq -r '.output.gates[] | "\(.id) \(.lane)"' rs.json | LC_ALL=C sort > rs.lanes
cmp -s sh.lanes rs.lanes || { echo "    the readers give different lanes:"; diff sh.lanes rs.lanes | head; exit 1; }
[ "$n_gates" -gt 0 ] && [ "$(wc -l < sh.lanes | tr -d ' ')" = "$n_gates" ] && [ "$n_rs" = "$n_gates" ] || { echo "    the readers report $(wc -l < sh.lanes | tr -d ' ') and $n_rs lanes, not one for each of the model's $n_gates gates"; exit 1; }

# ---------------------------------------------------------------- the three refusals, in both readers
# one mutation each of a copy of the model, refused by both with the same text
awk '$0 == "  - id: shell-suite" { f = 1 } f && /^    lane: / { f = 0; next } { print }' "$M" > no-lane.yaml
awk '$0 == "  - id: english-only" { f = 1 } f && /^    lane: / { print "    lane: fast"; f = 0; next } { print }' "$M" > fast.yaml
sed 's/^  integration: \[feedback, proof\]$/  integration: [feedback, lane-x]/' "$M" > lane-x.yaml
for f in no-lane.yaml fast.yaml lane-x.yaml; do cmp -s "$f" "$M" && { echo "    the mutation $f changed nothing"; exit 1; }; done
check() { # model, the refusal both readers must give
  sh_says "$1"; rs="$(rs_says "$1")"
  [ "$sh_st" = 10 ] || { echo "    ci-plan --check exited $sh_st on $1, expected 10"; printf '%s\n' "$sh"; exit 1; }
  [ "$sh" = "$2" ] || { echo "    ci-plan refuses $1 with: $sh"; echo "      expected: $2"; exit 1; }
  [ "$rs" = "$2" ] || { echo "    the executable refuses $1 with: ${rs:-(nothing: it read the model)}"; echo "      expected: $2"; exit 1; }
}
check no-lane.yaml 'gate shell-suite declares no lane, and with tiers declared every gate is in one of feedback, proof and release'
check fast.yaml 'gate english-only: lane is feedback, proof or release, not fast'
check lane-x.yaml 'tier integration names a lane that does not exist: lane-x'
# a lane that is none of the three is refused without tiers too: no reader can tell what it means
grep -v -E '^tiers:$|^  (local|pull-request|integration|release): \[' fast.yaml > fast-untiered.yaml
check fast-untiered.yaml 'gate english-only: lane is feedback, proof or release, not fast'

# ---------------------------------------------------------------- without tiers, as before
# a model with no tiers and no lanes is today's model: read, and planned exactly as with them
grep -v -E '^tiers:$|^  (local|pull-request|integration|release): \[|^    lane: ' "$M" > untiered.yaml
sh_says untiered.yaml
[ -z "$sh" ] && [ "$sh_st" = 0 ] || { echo "    ci-plan refuses a model without lanes ($sh_st): $sh"; exit 1; }
[ -z "$(rs_says untiered.yaml)" ] || { echo "    the executable refuses a model without lanes: $(rs_says untiered.yaml)"; exit 1; }
[ "$(jq '[.output.gates[] | select(.lane != null)] | length' rs.json)" = 0 ] || { echo "    lanes read from a model that has none"; exit 1; }
for p in apps/majordomus-cli/src/lib.rs docs/CLI.md site/templates/index.html .ai/repo/ci/gates.yaml \
         share/obligations.yaml nowhere/declared.txt; do
  a="$(printf '%s\n' "$p" | "$PLAN" --files - | jq -c '[.mode, .reason, .selected, [.gates[] | [.id, .selected, .reason]]]')"
  b="$(printf '%s\n' "$p" | "$PLAN" --model untiered.yaml --files - | jq -c '[.mode, .reason, .selected, [.gates[] | [.id, .selected, .reason]]]')"
  [ -n "$a" ] && [ "$a" = "$b" ] || { echo "    the lanes changed the plan of $p"; exit 1; }
done

echo "    every gate of the model is in a lane, and both readers refuse a missing lane, an unknown one and a tier naming one in the same words; without tiers the plan is unchanged"
