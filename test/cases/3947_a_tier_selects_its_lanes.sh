# majordomus-covers: none
# A tier selects its lanes (I3947, slice S4 of I2350, ADR 0186).
#
# Every gate of the CI model is in a lane (I3946), and the model's `tiers` name the lanes each
# moment a plan is asked at runs: local and pull-request run feedback, integration adds proof,
# release runs all three. A plan asked at a tier leaves out every gate whose lane the tier does
# not run, says `lane L: not in the T tier`, and lets nothing bring it back: neither implies
# nor requires reaches past the tier, and a subsuming gate the tier left out subsumes nothing,
# so the gate it would have replaced still runs. Both readers decide this at the same point and
# in the same words: scripts/ci-plan --tier and the executable's gates.completion `tier`.
# Without a tier every lane is planned exactly as before; a tier the model does not declare,
# and any tier of a model without tiers, is refused by both in the same words.
. "$ROOT/test/lib.sh"
BIN="$(rust_bin)" || rust_bin_exit $?
PLAN="$ROOT/scripts/ci-plan"
M="$ROOT/.ai/repo/ci/gates.yaml"
tab="$(printf '\t')"

# The executable reads the model of the repository it is pointed at, so the fixture is made a
# repository of the layer and each model is copied there. A managed repository carries no
# share/ of its own, so the executable is told where the tool's is, as case 131 tells it.
"$MJ" init >/dev/null
export MAJORDOMUS_SHARE="$ROOT/share"
mkdir -p .ai/repo/ci

# one line per gate, `id<TAB>run|skip<TAB>reason`, sorted, as each reader plans a model at a tier
sh_plan() { # model, tier, paths...
  model="$1"; tier="$2"; shift 2
  printf '%s\n' "$@" | "$PLAN" --model "$model" --files - ${tier:+--tier "$tier"} \
    | jq -r '.gates[] | "\(.id)\t\(if .selected then "run" else "skip" end)\t\(.reason)"' | LC_ALL=C sort
}
rs_plan() { # model, tier, paths...
  model="$1"; tier="$2"; shift 2
  cp "$model" .ai/repo/ci/gates.yaml
  input="$(printf '%s\n' "$@" | jq -R . | jq -sc --arg t "$tier" '{changed: .} + (if $t == "" then {} else {tier: $t} end)')"
  "$BIN" run gates.completion --input "$input" --quiet --format json --repo . 2>/dev/null \
    | jq -r '.output.plan | ((.selected | to_entries[] | "\(.key)\trun\t\(.value)"), (.excluded | to_entries[] | "\(.key)\tskip\t\(.value)"))' \
    | LC_ALL=C sort
}
# both readers, compared; the agreed plan is left in $agreed
both() {
  sh_plan "$@" > sh.plan; rs_plan "$@" > rs.plan
  [ -s rs.plan ] && cmp -s sh.plan rs.plan || {
    echo "    the readers plan $1 at the tier '${2:-(none)}' differently:"; diff sh.plan rs.plan | head -10; exit 1; }
  agreed="$(cat sh.plan)"
}
says() { # gate, run|skip, reason: what the agreed plan says of one gate
  printf '%s\n' "$agreed" | grep -qxF "$1$tab$2$tab$3" || {
    echo "    $1 is not '$2: $3'; the plan says: $(printf '%s\n' "$agreed" | awk -F"$tab" -v g="$1" '$1 == g { print $2 ": " $3 }')"; exit 1; }
}

# ---------------------------------------------------------------- a model small enough to read
# lint (feedback) always runs; build (feedback) implies registry (proof); probe (feedback)
# requires site (proof); cases (feedback) is subsumed by suite (proof)
cat > tiers.yaml <<'YAML'
version: 1
tiers:
  pull-request: [feedback]
  integration: [feedback, proof]
gates:
  - id: lint
    job: structure
    lane: feedback
    always: true
    runs: "true"
  - id: build
    job: rust
    lane: feedback
    implies: [registry]
    runs: "true"
  - id: registry
    job: rust
    lane: proof
    runs: "true"
  - id: probe
    job: site
    lane: feedback
    requires: [site]
    runs: "true"
  - id: site
    job: site
    lane: proof
    runs: "true"
  - id: cases
    job: rust
    lane: feedback
    subsumed-by: suite
    runs: "true"
  - id: suite
    job: suite
    lane: proof
    runs: "true"
classes:
  - id: code
    paths: [apps/**]
    gates: [build, cases, suite]
  - id: web
    paths: [site/**]
    gates: [probe]
YAML
st=0; "$PLAN" --model tiers.yaml --check > /dev/null 2>&1 || st=$?
[ "$st" = 0 ] || { echo "    ci-plan --check refuses the fixture ($st)"; exit 1; }

# the pull-request tier: the proof gates are out, and nothing brings them back
both tiers.yaml pull-request apps/x.rs site/a.html
says lint run always
says build run 'class code'
says probe run 'class web'
says registry skip 'lane proof: not in the pull-request tier'
says site skip 'lane proof: not in the pull-request tier'
says suite skip 'lane proof: not in the pull-request tier'
# the subsuming gate the tier left out subsumes nothing: the cases still run
says cases run 'class code'

# the integration tier runs proof too: implies, requires and subsumed-by apply as without tiers
both tiers.yaml integration apps/x.rs site/a.html
says registry run 'implied by build'
says site run 'required by probe'
says suite run 'class code'
says cases skip 'subsumed by suite: the planned suite runs every case this gate runs, so planning it too would run them twice'
with_integration="$agreed"

# no tier plans every lane, exactly as the model without lanes and tiers does
both tiers.yaml '' apps/x.rs site/a.html
[ "$agreed" = "$with_integration" ] || { echo "    the plan without a tier is not the plan of every lane"; exit 1; }
grep -v -E '^tiers:$|^  (pull-request|integration): \[|^    lane: ' tiers.yaml > untiered.yaml
both untiered.yaml '' apps/x.rs site/a.html
[ "$agreed" = "$with_integration" ] || { echo "    the lanes changed the plan of a change asked at no tier"; exit 1; }

# a tier is decided first: it is the reason an on-demand gate gives, and --on-demand cannot
# bring back a gate the tier left out
awk '{ print } $0 == "    lane: proof" { print "    on-demand: true" }' tiers.yaml > on-demand.yaml
grep -q 'on-demand: true' on-demand.yaml || { echo "    the on-demand mutation did not take"; exit 1; }
printf '%s\n' apps/x.rs | "$PLAN" --model on-demand.yaml --files - --tier pull-request --on-demand > od.json
[ "$(jq -r '.gates[] | select(.id == "suite") | "\(.selected) \(.reason)"' od.json)" = "false lane proof: not in the pull-request tier" ] \
  || { echo "    --on-demand brought back a gate the tier left out: $(jq -c '.gates[] | select(.id == "suite")' od.json)"; exit 1; }

# ---------------------------------------------------------------- the repository's own model
# every gate whose lane a tier does not run is out of that tier's full plan with the reason,
# and every other gate is planned as without a tier; both readers agree at every tier
"$PLAN" --full > none.json
for tier in local pull-request integration release; do
  lanes=" $(awk -v t="$tier" '$0 ~ "^  " t ": \\[" { sub(/^[^[]*\[/, ""); sub(/\].*/, ""); gsub(/,/, " "); print }' "$M") "
  "$PLAN" --full --tier "$tier" > tier.json
  wrong="$(jq -r --arg lanes "$lanes" --arg t "$tier" --slurpfile none none.json '
    ($none[0].gates | map({key: .id, value: .}) | from_entries) as $n
    | .gates[]
    | .lane as $l
    | if ($lanes | contains(" " + $l + " ")) then
        select(.selected != $n[.id].selected or .reason != $n[.id].reason) | "\(.id): planned unlike the plan without a tier"
      else
        select(.selected or .reason != "lane \(.lane): not in the \($t) tier") | "\(.id): \(.selected) \(.reason)"
      end' tier.json)"
  [ -z "$wrong" ] || { echo "    the $tier tier of the repository's model:"; printf '      %s\n' "$wrong" | head; exit 1; }
  both "$M" "$tier" apps/majordomus-cli/src/lib.rs docs/CLI.md
done

# ---------------------------------------------------------------- the refusals, in both readers
refused() { # model, tier, the refusal both readers must give
  sh_st=0; printf 'apps/x.rs\n' | "$PLAN" --model "$1" --files - --tier "$2" > /dev/null 2> sh.err || sh_st=$?
  [ "$sh_st" = 2 ] || { echo "    ci-plan --tier $2 exited $sh_st on $1, expected 2"; cat sh.err; exit 1; }
  [ "$(sed 's/^ci-plan: //' sh.err)" = "$3" ] || { echo "    ci-plan refuses --tier $2 with: $(cat sh.err)"; echo "      expected: $3"; exit 1; }
  cp "$1" .ai/repo/ci/gates.yaml
  rs_st=0; "$BIN" run gates.completion --input "$(jq -nc --arg t "$2" '{changed: ["apps/x.rs"], tier: $t}')" \
    --quiet --format json --repo . > rs.out 2> rs.err || rs_st=$?
  [ "$rs_st" != 0 ] || { echo "    gates.completion planned the tier $2 of $1"; exit 1; }
  grep -qF "$3" rs.out rs.err || { echo "    gates.completion refuses the tier $2 without: $3"; cat rs.out rs.err | head -5; exit 1; }
}
refused tiers.yaml nightly 'tier nightly: the model declares no such tier; it declares integration, pull-request'
refused untiered.yaml pull-request 'tier pull-request: the model declares no tiers, so it cannot plan one'

echo "    a tier plans only its lanes, nothing implies, requires or subsumes past it, no tier plans every lane, and both readers agree and refuse alike"
