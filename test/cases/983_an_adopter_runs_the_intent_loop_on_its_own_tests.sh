# majordomus-covers: start evidence
# majordomus-negative: start
# claim: an-adopter-runs-the-intent-loop-on-its-own-tests
# A repository that adopted the tool runs the whole intent loop on its own tests, with only
# documented commands and its own layout (ADR 0121).
#
# The repository here is fresh from `init` and keeps its tests where it always has, in
# tests/, as *_test.sh — not in test/cases/, which is where the tool's own repository keeps
# them. Nothing in this case names the tool's repository; everything the loop needs comes from
# what this repository declares:
#
#   1. with the skeleton's policy, `start` is what it always was and nothing asks an intent
#   2. without a declaration, a criterion naming tests/greet_test.sh names no test, and says
#      how to declare where the tests are
#   3. the policy declares the suite's root and suffix; the intent validates
#   4. a critique is recorded, the binding is turned on, and an unbound start is refused while
#      a start naming the issue is bound with its pins
#   5. the adopter's own run is recorded with `evidence stamp` and `evidence record`, and the
#      criterion is met and the verdict satisfied — while the stage still says the milestone is
#      open
#   6. the test changes: the evidence is stale, the verdict unsatisfied, and a handover written
#      before the change resolves to `evidence_moved`
. "$ROOT/test/lib.sh"
command -v jq >/dev/null 2>&1 || { echo "    jq is required"; exit 1; }
RB="$(rust_bin)" || { echo "    no executable: install cargo or set MAJORDOMUS_BIN"; exit 1; }
MAJORDOMUS_BIN="$RB"; export MAJORDOMUS_BIN
unset MAJORDOMUS_PROVIDER_SESSION CLAUDE_CODE_SESSION_ID

"$MJ" init >/dev/null; "$MJ" update >/dev/null
sed 's/^  ensure_server_on_start: true /  ensure_server_on_start: false /' .ai/repo/policy.yaml > "$T.pol" \
  && cat "$T.pol" > .ai/repo/policy.yaml && rm -f "$T.pol"
mkdir -p src tests .ai/repo/project/intents .ai/repo/project/milestones .ai/repo/project/issues .ai/repo/project/critiques
printf 'echo hello\n' > src/greet.sh
printf 'test "$(sh src/greet.sh)" = hello\n' > tests/greet_test.sh
commit() { git add -A >/dev/null && git commit -qm "$1"; }
commit "an adopter with its own tests"

# ---------------------------------------------------------------- 1. the skeleton's policy
expect_exit 0 "$MJ" start "ordinary work" --scope src
grep -q '^binding:' .ai/local/state/current.yaml && { echo "    a start under the skeleton's policy asked an intent"; exit 1; }
"$MJ" handover --derive --close >/dev/null 2>&1 || true

# ---------------------------------------------------------------- 2. no declaration
cat > .ai/repo/project/milestones/greet.yaml <<'YAML'
id: greet
title: Users are greeted
slug: greet
order: 1
priority: p1
problem: "Nobody is greeted."
outcome: "Everybody is greeted."
acceptance_criteria:
  - It greets
validation:
  - "sh tests/greet_test.sh"
evidence_required: []
YAML
cat > .ai/repo/project/intents/greet.yaml <<'YAML'
id: greet
title: Every user is greeted
statement: "Every user is greeted, politely."
invariants:
  - The greeting stays polite
milestones:
  - greet
satisfaction:
  - id: says-hello
    criterion: The program says hello
    evidence: test
    ref: tests/greet_test.sh
YAML
cat > .ai/repo/project/issues/I0001.yaml <<'YAML'
id: I0001
milestone: greet
title: Say hello
slug: say-hello
priority: p1
profile: implementation
objective: "Say hello."
scope:
  - src
serves:
  - greet#says-hello
acceptance_criteria:
  - It says hello
validation:
  - "sh tests/greet_test.sh"
evidence_required:
  - greeted
YAML
commit "an intent over the adopter's own test"
"$RB" intent validate > "$T.v" 2>&1 && { echo "    a criterion naming an undeclared test layout validated"; exit 1; }
grep -q 'says-hello.*tests/greet_test.sh.*evidence.tests' "$T.v" \
  || { echo "    the refusal does not say how to declare the layout:"; sed 's/^/    | /' "$T.v"; exit 1; }

# ---------------------------------------------------------------- 3. the declaration
cat >> .ai/repo/policy.yaml <<'YAML'

evidence:
  tests:
    - runner: suite
      root: tests
      suffix: _test.sh
      run: [sh, "{path}"]
YAML
commit "the adopter declares where its tests are"
"$RB" intent validate > "$T.v" 2>&1 || true
grep -q 'unresolved_evidence_ref' "$T.v" && { echo "    the declared layout still does not resolve:"; sed 's/^/    | /' "$T.v"; exit 1; }

# ---------------------------------------------------------------- 4. critique, binding, start
cat > .ai/repo/project/critiques/greet.yaml <<'YAML'
intent: greet
reviewed_at: HEAD
reviewed_by: case 983
findings: []
YAML
awk '/^intent:[[:space:]]*$/ { skip=1; next } skip && /^[^[:space:]#]/ { skip=0 } !skip { print }' \
  .ai/repo/policy.yaml > "$T.pol" && cat "$T.pol" > .ai/repo/policy.yaml && rm -f "$T.pol"
printf '\nintent:\n  binding: required\n' >> .ai/repo/policy.yaml
commit "critiqued, and work is bound"
expect_exit 0 "$RB" intent validate
grep -q '^  binding: required$' .ai/repo/policy.yaml || { echo "    the policy does not require binding:"; sed 's/^/    | /' .ai/repo/policy.yaml; exit 1; }
expect_exit 15 "$MJ" start "unrelated work" --scope docs
expect_exit 0 "$MJ" start "say hello" --issue I0001 --scope src
PLAN="$(sed -n 's/^plan_revision: //p' .ai/local/state/current.yaml)"
[ -n "$PLAN" ] || { echo "    the bound start recorded no pin"; exit 1; }
"$MJ" handover --derive >/dev/null 2>&1 || { echo "    the handover was refused"; exit 1; }

# ---------------------------------------------------------------- 5. the adopter's own run
verdict() { "$RB" intent show greet --format json | jq -r '.verdict.state'; }
criterion() { "$RB" intent show greet --format json | jq -r '.satisfaction[] | select(.id == "says-hello") | .state'; }
[ "$(criterion)" = not_run ] || { echo "    before any run the criterion is $(criterion)"; exit 1; }
sh tests/greet_test.sh || { echo "    the adopter's own test fails"; exit 1; }
printf 'greet\tok\t1\tserial\n' > "$T.suite.tsv"
expect_exit 0 "$RB" evidence stamp --producer suite --report "$T.suite.tsv" --out "$T.prov.json"
expect_exit 0 "$RB" evidence record --suite "$T.suite.tsv" --provenance "$T.prov.json"
[ "$(criterion)" = current ] || { echo "    after the recorded run the criterion is $(criterion)"; exit 1; }
repro="$("$RB" intent show greet --format json | jq -r '.satisfaction[] | select(.id == "says-hello") | .reproduce // empty')"
[ "$repro" = "sh tests/greet_test.sh" ] || { echo "    the reproduce line is not the declared run: '$repro'"; exit 1; }
[ "$(verdict)" = satisfied ] || { echo "    after the recorded run the verdict is $(verdict)"; exit 1; }
stage="$("$RB" intent show greet --format json | jq -r '.stage')"
[ "$stage" != satisfied ] || { echo "    the stage says satisfied while the milestone is open"; exit 1; }

# ---------------------------------------------------------------- 6. drift
printf 'test "$(sh src/greet.sh)" = "hello"\n' > tests/greet_test.sh
commit "the test is edited"
[ "$(criterion)" = stale ] || { echo "    after the test changed the criterion is $(criterion)"; exit 1; }
[ "$(verdict)" = unsatisfied ] || { echo "    after the test changed the verdict is $(verdict)"; exit 1; }
"$MJ" handover --resolve > "$T.r" 2>&1 || true
grep -q '^Intent: evidence_moved' "$T.r" || { echo "    the resolved handover does not say the evidence moved:"; sed 's/^/    | /' "$T.r"; exit 1; }
rm -f "$T.v" "$T.r" "$T.suite.tsv" "$T.prov.json"
exit 0
