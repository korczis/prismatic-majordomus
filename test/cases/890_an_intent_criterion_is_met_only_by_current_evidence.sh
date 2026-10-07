# majordomus-covers: none
# claims: none
# An intent criterion is met only by evidence that is current at the checkout.
#
# The false `met` this case exists for: a passing run recorded on a dirty tree, whose case
# still hashes to what ran, read as current evidence, and an intent whose work is all closed
# read satisfied on it. Currency is the evidence module's own judgement (evidence::freshness,
# read through evidence::current), so:
#
#   close the work over a pass recorded on a dirty tree -> stale, verifying, exit 10
#   a pass recorded on a clean tree                     -> satisfied, exit 0
#   the code under test breaks, the case untouched      -> stale, exit 10; run, failing
#   repaired and run                                    -> satisfied again
#   any later change                                    -> stale: nothing rules it out
#   the case changes                                    -> stale, exit 10
#   the changed case runs and passes                    -> satisfied again
#
# The case proves no claim, so the repository declares nothing about the code it tests: its
# own file is not that code, and a pass of it is current only while nothing has changed since
# the run. A criterion over a test whose claim names its implementation stays met across a
# change it does not name; tests/intent.rs holds that route.
#
# Every run is the runner's own report recorded by `majordomus evidence record`, so the tree
# word each verdict rests on is the recorder's, never a hand-written ledger line. The intent
# file is never touched after it is declared.
#
# It never skips: an executable is required, so a machine without one fails here instead of
# reporting a pass it never measured.
. "$ROOT/test/lib.sh"
command -v jq >/dev/null 2>&1 || { echo "    jq is required"; exit 1; }
RB="$(rust_bin)" || { echo "    no executable: install cargo or set MAJORDOMUS_BIN"; exit 1; }
[ -x "$RB" ] || { echo "    the build produced no executable at $RB"; exit 1; }
MAJORDOMUS_SHARE="$ROOT/share"; export MAJORDOMUS_SHARE
# the runner's report and the declared copy live outside the repository: a file written inside
# it is a pending change, and the recorder would then stamp every run `dirty`, which is not
# current evidence of anything
W="$(mktemp -d "${TMPDIR:-/tmp}/mj890.XXXXXX")"; trap 'rm -rf "$W"' EXIT

same() { [ "$2" = "$3" ] || { printf '    %s: expected %s, got %s\n' "$1" "$2" "$3"; exit 1; }; }
commit() { git add -A >/dev/null && git commit -qm "$1" >/dev/null; }

"$MJ" init >/dev/null; "$MJ" update >/dev/null
"$MJ" capture install >/dev/null

# ---------------------------------------------------------------- declare
pj_init
mkdir -p .ai/repo/project/intents test/cases lib
cat > .ai/repo/project/milestones/probe.yaml <<'Y'
id: probe
title: The probe reaches its outcome
slug: probe
order: 0
priority: p1
problem: "A behaviour is missing."
outcome: "The behaviour exists and is proven."
acceptance_criteria:
  - The behaviour exists
validation:
  - "true"
evidence_required:
  - reached
Y
cat > .ai/repo/project/issues/I0001.yaml <<'Y'
id: I0001
milestone: probe
title: Behaviour a
slug: behaviour-a
priority: p1
profile: implementation
objective: "Make behaviour a exist."
scope:
  - lib/a
serves:
  - probe#a
acceptance_criteria:
  - The behaviour exists
validation:
  - "true"
evidence_required:
  - proof
Y
printf 'grep -qx ok lib/a\n' > test/cases/01_a.sh
printf 'notes\n' > notes.md
cat > .ai/repo/project/intents/probe.yaml <<'Y'
id: probe
title: Behaviour a is true for the people who rely on it
statement: "Behaviour a exists, and the recorded run of its case says so at the checkout."
invariants:
  - No stage of this intent is written anywhere
milestones:
  - probe
satisfaction:
  - id: a
    criterion: Behaviour a exists
    evidence: test
    ref: test/cases/01_a.sh
Y
commit "declare the intent"
cp .ai/repo/project/intents/probe.yaml "$W/declared.yaml"

real() { "$RB" intent realization --intent probe --format json; }
stage() { real | jq -r '.intents[0].stage'; }
crit() { real | jq -r '[.intents[0].unmet[] | "\(.id):\(.state)"] | join(",")'; }
# run the case the way the runner does, record its report with the real recorder, and commit
# whatever the tree held (the pending change included) with the record
run() {
  local word=ok
  bash test/cases/01_a.sh || word=FAIL
  printf '01_a\t%s\t0\tserial\n' "$word" > "$W/report.tsv"
  "$RB" evidence record --suite "$W/report.tsv" >/dev/null
  commit "record 01_a: $word"
}
tree_of_latest() { jq -r '[.executions[] | select(.test=="suite:01_a")] | last | .working_tree' \
  .ai/repo/evidence/ledger.json; }

expect_exit 0 "$RB" intent validate
same "nothing is realised yet" "a:not_run" "$(crit)"

# ---------------------------------------------------------------- closed over a dirty-tree pass
"$MJ" start "realise the probe intent" --scope lib,test/cases,notes.md,.ai/repo >/dev/null
"$MJ" plan start I0001 >/dev/null
echo ok > lib/a
commit "behaviour a"
echo "an edit pending while the case ran" >> notes.md
run
same "the recorder saw the pending edit" "dirty" "$(tree_of_latest)"
"$MJ" plan evidence I0001 --covers proof --type test --command "bash test/cases/01_a.sh" --result ok >/dev/null
"$MJ" plan "done" I0001 >/dev/null
"$MJ" plan evidence probe --covers reached --type test --command "bash test/run.sh" --result ok >/dev/null
commit "close the issue and the milestone"
same "the issue is closed" "DONE" "$("$MJ" plan list | awk '$1=="I0001"{print $2}')"
same "a pass on a dirty tree is not current evidence" "a:stale" "$(crit)"
same "closed work over it is not a satisfied intent" "verifying" "$(stage)"
expect_exit 10 "$RB" intent realization --intent probe
expect_grep 'closed_work_contradicted  probe: .*criterion `a`'
expect_grep 'ran on a tree that was not its commit'

# ---------------------------------------------------------------- a clean pass
run
same "the recorder saw a clean tree" "clean" "$(tree_of_latest)"
same "a clean pass is current evidence" "satisfied" "$(stage)"
expect_exit 0 "$RB" intent realization --intent probe

# ---------------------------------------------------------------- the code under test breaks
# the case's file is byte-identical, and the code it tests is not what ran
echo broken > lib/a
commit "the code under test breaks"
same "a pass of the old code is not evidence of the broken one" "a:stale" "$(crit)"
same "closed work over it is not a satisfied intent" "verifying" "$(stage)"
expect_exit 10 "$RB" intent realization --intent probe
expect_grep 'closed_work_contradicted  probe: .*criterion `a`'
expect_grep 'names no code under test and something changed since'
run
same "the broken code, run, fails" "a:failing" "$(crit)"
expect_exit 10 "$RB" intent realization --intent probe
echo ok > lib/a
commit "the code under test is repaired"
run
same "the repaired code, run and passing, satisfies it again" "satisfied" "$(stage)"
expect_exit 0 "$RB" intent realization --intent probe

# a change outside the code the serving issue declares (lib/a) is not a change to the code
# under test: the pass stays met, resting on unchanged inputs rather than on a run at this
# revision (the Rust case a_test_that_names_no_code_under_test_is_current_only_at_the_commit_it_ran_on
# proves the other side: with nothing declared, any such change stales it)
echo "a later note" >> README.md
commit "a later change"
same "a change outside the declared code under test" "satisfied" "$(stage)"
same "and nothing it names is unmet" "" "$(crit)"
run
same "run at the checkout, it is still met" "satisfied" "$(stage)"

# ---------------------------------------------------------------- the case changes
printf 'grep -qx ok lib/a\ntrue\n' > test/cases/01_a.sh
commit "the case changes"
same "a pass of the old case is not evidence of the new one" "a:stale" "$(crit)"
expect_exit 10 "$RB" intent realization --intent probe
expect_grep 'closed_work_contradicted  probe: .*criterion `a`'
run
same "the changed case, run and passing, satisfies it again" "satisfied" "$(stage)"
expect_exit 0 "$RB" intent realization --intent probe

# nothing about the intent was ever written: every stage above was derived
cmp -s .ai/repo/project/intents/probe.yaml "$W/declared.yaml" \
  || { echo "    the intent record changed; a stage was written somewhere"; exit 1; }
