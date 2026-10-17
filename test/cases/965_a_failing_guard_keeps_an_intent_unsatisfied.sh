# majordomus-covers: none
# claims: intent-guard-is-judged
# A failing guard keeps an intent unsatisfied, whatever its criteria and its milestones say.
#
# An intent's invariants were prose: printed to the worker and judged by nothing, so what an
# intent said must stay true could stop being true while the intent read satisfied (ADR
# 0113). A guard is an invariant that names its evidence. This case declares one intent with
# one criterion and one guard through a fresh `init`, closes its work, and reads the intent
# from the built executable after each recorded run:
#
#   the criterion passes, the guard was never run   satisfied: a guard nobody ran is not
#                                                   judged, and violates nothing
#   the guard's case fails                          unsatisfied, naming the guard, at stage
#                                                   verifying, with every criterion met and
#                                                   every milestone DONE
#   the guard's case passes again                   satisfied
#   an unrelated commit                             still satisfied: a stale guard is not
#                                                   judged either
#
# and that the record holds none of it: the intent file is byte for byte what was declared.
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
W="$(mktemp -d "${TMPDIR:-/tmp}/mj965.XXXXXX")"; trap 'rm -rf "$W"' EXIT

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
guards:
  - id: b-stays
    invariant: Behaviour b keeps working while a is built
    evidence: test
    ref: test/cases/02_b.sh
milestones:
  - probe
satisfaction:
  - id: a
    criterion: Behaviour a exists
    evidence: test
    ref: test/cases/01_a.sh
Y
printf 'grep -qx ok lib/b\n' > test/cases/02_b.sh
commit "declare the intent"
cp .ai/repo/project/intents/probe.yaml "$W/declared.yaml"

show() { "$RB" intent show probe --format json; }
verdict() { show | jq -r '.verdict.state'; }
stage() { show | jq -r '.stage'; }
guard() { show | jq -r '.guards[0] | "\(.state) \(.violated)"'; }
# run a case the way the runner does, record its report with the real recorder, and commit
run() { # <case name>
  local word=ok
  bash "test/cases/$1.sh" || word=FAIL
  printf '%s\t%s\t0\tserial\n' "$1" "$word" > "$W/report.tsv"
  # measured as the run left it, the way a runner stamps its own report: a record with no
  # stamp carries an unknown tree, and an unknown tree is never current evidence
  "$RB" evidence stamp --producer suite --report "$W/report.tsv" --out "$W/report.provenance.json" >/dev/null
  "$RB" evidence record --suite "$W/report.tsv" --provenance "suite=$W/report.provenance.json" >/dev/null
  commit "record $1: $word"
}

expect_exit 0 "$RB" intent validate

# ---------------------------------------------------------------- the work is done and proven
"$MJ" start "realise the probe intent" --scope lib,test/cases,notes.md,.ai/repo >/dev/null
"$MJ" plan start I0001 >/dev/null
echo ok > lib/a; echo ok > lib/b
commit "behaviours a and b"
run 01_a
"$MJ" plan evidence I0001 --covers proof --type test --command "bash test/cases/01_a.sh" --result ok >/dev/null
"$MJ" plan "done" I0001 >/dev/null
"$MJ" plan evidence probe --covers reached --type test --command "bash test/run.sh" --result ok >/dev/null
commit "close the issue and the milestone"
run 01_a
same "the issue is closed" "DONE" "$("$MJ" plan list | awk '$1=="I0001"{print $2}')"

# ---------------------------------------------------------------- a guard nobody ran
same "a guard nobody ran" "not_run false" "$(guard)"
same "is not judged: the criterion decides" "satisfied" "$(verdict)"
same "and the stage follows the verdict" "satisfied" "$(stage)"

# ---------------------------------------------------------------- the guard fails
echo broken > lib/b
commit "behaviour b breaks"
run 02_b
run 01_a
same "the guard's run failed" "failing true" "$(guard)"
same "every criterion is still met" "true" "$(show | jq -r '.satisfaction[0].met')"
same "and the intent is not satisfied" "unsatisfied" "$(verdict)"
same "at a stage that is not satisfied" "verifying" "$(stage)"
show | jq -e '.verdict.reasons == [] and .verdict.guards == [{"guard":"b-stays","evidence":"test","state":"failing"}]' >/dev/null \
  || { echo "    the verdict does not name the guard:"; show | jq .verdict; exit 1; }
show | jq -e '.guards[0].evaluation.outcome == "fail"' >/dev/null \
  || { echo "    the guard does not name the run it was judged by:"; show | jq '.guards[0]'; exit 1; }
expect_exit 0 "$RB" intent show probe
expect_grep '^  guard       b-stays  violated  test failing  — Behaviour b keeps working while a is built$'
expect_grep '^  violated    b-stays  test failing$'
expect_exit 0 "$RB" intent explain probe
expect_grep 'guard `b-stays` is violated'
# a worker asking what the issue serves is told too (the binding is refused here for another
# reason — nobody critiqued this plan — and says what it found all the same)
"$RB" intent binding --issue I0001 > "$W/binding.out" 2>/dev/null || true
expect_grep 'guard       b-stays  violated' "$W/binding.out"

# ---------------------------------------------------------------- the guard passes again
echo ok > lib/b
commit "behaviour b is repaired"
run 02_b
run 01_a
same "the guard holds" "current false" "$(guard)"
same "and the intent is satisfied again" "satisfied" "$(verdict)"
same "at the satisfied stage" "satisfied" "$(stage)"

# ---------------------------------------------------------------- an unrelated commit
echo more >> notes.md
commit "an unrelated note"
run 01_a
case "$(guard)" in *" false") ;; *) echo "    an unrelated commit violated the guard: $(guard)"; exit 1 ;; esac
same "a guard that is not judged violates nothing" "satisfied" "$(verdict)"

# ---------------------------------------------------------------- nothing was stored
cmp -s .ai/repo/project/intents/probe.yaml "$W/declared.yaml" \
  || { echo "    the intent record changed: something was stored in it"; exit 1; }
