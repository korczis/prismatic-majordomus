# majordomus-covers: none
# claims: none
# An intent is realised across two providers and a handover, and is satisfied only while
# reality says so.
#
# The closed loop of intent-driven development, driven through the lifecycle a worker runs and
# never through a hand-written ledger line (ADR 0075):
#
#   declare an intent with two criteria -> both unmet
#   a Claude Code episode (its own session hook) starts the task, realises criterion a, and
#     hands over with criterion b outstanding
#   a Codex episode (the session entry point its adapter calls) resumes the same task
#   the realization joins both episodes, both providers and the handover to the one intent
#   criterion b's case fails -> every issue and the milestone closed, the intent is NOT satisfied,
#     the realization names the contradiction and exits 10
#   fix -> the recorded run passes -> satisfied
#   break reality (lib/a) -> the recorded run fails -> unsatisfied again, named, exit 10
#   repair -> satisfied again
#
# The evidence is the runner's own report recorded by `majordomus evidence record`, so each
# verdict carries the digest of the case that produced it. The intent file is never touched
# after it is declared: every change of stage is derived.
#
# It never skips: an executable is required, so a machine without one fails here instead of
# reporting a pass it never measured.
. "$ROOT/test/lib.sh"
command -v jq >/dev/null 2>&1 || { echo "    jq is required"; exit 1; }
RB="$(rust_bin)" || { echo "    no executable: install cargo or set MAJORDOMUS_BIN"; exit 1; }
[ -x "$RB" ] || { echo "    the build produced no executable at $RB"; exit 1; }
MAJORDOMUS_SHARE="$ROOT/share"; export MAJORDOMUS_SHARE
# the suite may itself run inside a provider session; this case drives two of its own
unset MAJORDOMUS_PROVIDER_SESSION CLAUDE_CODE_SESSION_ID

same() { [ "$2" = "$3" ] || { printf '    %s: expected %s, got %s\n' "$1" "$2" "$3"; exit 1; }; }
commit() { git add -A >/dev/null && git commit -qm "$1" >/dev/null; }

"$MJ" init >/dev/null; "$MJ" update >/dev/null
"$MJ" capture install >/dev/null
PATH="$(dirname "$MJ"):$PATH"; export PATH
start_claude() { printf '{"session_id":"%s","source":"startup"}' "$1" | ./.claude/hooks/majordomus-session-start; }
end_claude()   { printf '{"session_id":"%s","reason":"clear"}' "$1" | ./.claude/hooks/majordomus-session-end; }

# ---------------------------------------------------------------- declare
pj_init
mkdir -p .ai/repo/project/intents test/cases lib
cat > .ai/repo/project/milestones/probe.yaml <<'Y'
id: probe
title: The probe reaches its outcome
slug: probe
order: 0
priority: p1
problem: "Two behaviours are missing."
outcome: "Both behaviours exist and are proven."
acceptance_criteria:
  - Both behaviours exist
validation:
  - "true"
evidence_required:
  - reached
Y
issue() { # <id> <path> <criterion>
  cat > ".ai/repo/project/issues/$1.yaml" <<Y
id: $1
milestone: probe
title: Behaviour $3
slug: behaviour-$3
priority: p1
profile: implementation
objective: "Make behaviour $3 exist."
scope:
  - $2
serves:
  - probe#$3
acceptance_criteria:
  - The behaviour exists
validation:
  - "true"
evidence_required:
  - proof
Y
}
issue I0001 lib/a a
issue I0002 lib/b b
printf 'grep -qx ok lib/a\n' > test/cases/01_a.sh
printf 'grep -qx ok lib/b\n' > test/cases/02_b.sh
cat > .ai/repo/project/intents/probe.yaml <<'Y'
id: probe
title: Both behaviours are true for the people who rely on them
statement: "Behaviour a and behaviour b both exist, and the recorded runs of their cases say so now."
invariants:
  - No stage of this intent is written anywhere
milestones:
  - probe
satisfaction:
  - id: a
    criterion: Behaviour a exists
    evidence: test
    ref: test/cases/01_a.sh
  - id: b
    criterion: Behaviour b exists
    evidence: test
    ref: test/cases/02_b.sh
Y
commit "declare the intent"
cp .ai/repo/project/intents/probe.yaml "$T/declared.yaml"

real() { "$RB" intent realization --intent probe --format json; }
stage() { real | jq -r '.intents[0].stage'; }
unmet() { real | jq -r '[.intents[0].unmet[] | "\(.id):\(.state)"] | join(",")'; }
# run one probe case the way the runner does and record its report with the real recorder
run() { # <case-name>
  local word=ok
  bash "test/cases/$1.sh" || word=FAIL
  printf '%s\t%s\t0\tserial\n' "$1" "$word" > "$T/report.tsv"
  "$RB" evidence record --suite "$T/report.tsv" >/dev/null
  commit "record $1: $word"
}

expect_exit 0 "$RB" intent validate
same "a declared intent with open work" "planned" "$(stage)"
same "nothing is realised yet" "a:not_run,b:not_run" "$(unmet)"

# ---------------------------------------------------------------- provider A: Claude Code
start_claude win-claude >/dev/null 2>&1
export MAJORDOMUS_PROVIDER_SESSION=win-claude
"$MJ" start "realise the probe intent" --scope lib,test/cases,.ai/repo >/dev/null
"$MJ" plan start I0001 >/dev/null
echo ok > lib/a
commit "behaviour a"
run 01_a
"$MJ" plan evidence I0001 --covers proof --type test --command "bash test/cases/01_a.sh" --result ok >/dev/null
"$MJ" plan "done" I0001 >/dev/null
"$MJ" plan start I0002 >/dev/null
commit "a is done, b is started"
printf '# Objective\nRealise the probe intent.\n# Current State\nBehaviour a exists and is proven; b is started.\n# Next Action\nMake behaviour b exist and prove it.\n' \
  | "$MJ" handover >/dev/null
end_claude win-claude >/dev/null 2>&1
unset MAJORDOMUS_PROVIDER_SESSION

same "one criterion proven while the work is open" "executing" "$(stage)"
same "b is what the handover left" "b:not_run" "$(unmet)"

# ---------------------------------------------------------------- provider B: Codex
"$MJ" session start --provider codex --provider-session win-codex --if-open keep >/dev/null
export MAJORDOMUS_PROVIDER_SESSION=win-codex
# the receiving session resolves what it continues without restating anything: the newest
# handover is the one the Claude episode's end derived, and it names the issues it moved
expect_exit 0 "$MJ" handover --resolve
expect_grep 'The issues this episode touched are I0001, I0002'
"$MJ" checkpoint </dev/null >/dev/null
echo nearly > lib/b
commit "behaviour b, wrong"

J="$(real)"
same "both episodes carried one task" "claude-code,codex" \
  "$(printf '%s' "$J" | jq -r '.intents[0].providers | join(",")')"
# two: the one the Claude worker wrote, and the one its session's end derived from the ledger
same "both handovers are part of the lineage" "2" \
  "$(printf '%s' "$J" | jq -r '.intents[0].work[0].handovers')"
same "the link is observed from the episodes' own plan transitions" "observed" \
  "$(printf '%s' "$J" | jq -r '.intents[0].work[0].provenance')"
same "the task moved both issues" "I0001,I0002" \
  "$(printf '%s' "$J" | jq -r '.work[] | select(.work.kind=="task") | .work.moved_issues | join(",")')"
same "the outstanding criterion names the issue serving it" "I0002" \
  "$(printf '%s' "$J" | jq -r '.intents[0].unmet[0].issues | join(",")')"

# ---------------------------------------------------------------- close the work, reality says no
run 02_b
"$MJ" plan evidence I0002 --covers proof --type test --command "bash test/cases/02_b.sh" --result fail >/dev/null
"$MJ" plan "done" I0002 >/dev/null
"$MJ" plan evidence probe --covers reached --type test --command "bash test/run.sh" --result ok >/dev/null
commit "close every issue and the milestone"
same "every issue and the milestone are closed" "DONE" "$("$MJ" plan list | awk '$1=="I0002"{print $2}')"
same "closed work is not a satisfied intent" "verifying" "$(stage)"
same "the failing criterion is named" "b:failing" "$(unmet)"
expect_exit 10 "$RB" intent realization --intent probe
expect_grep 'closed_work_contradicted  probe: .*criterion `b`'
expect_exit 0 "$RB" intent explain probe
expect_grep 'verifying: every milestone is DONE and 1 of 2 criteria'
expect_grep '`b` is not met: .*did not pass'

# ---------------------------------------------------------------- fix, and reality says yes
echo ok > lib/b
commit "behaviour b, right"
run 02_b
same "every criterion has current evidence" "satisfied" "$(stage)"
expect_exit 0 "$RB" intent realization --intent probe
expect_exit 0 "$RB" intent explain probe
expect_grep 'satisfied: every milestone is DONE and all 2 criteria'

# ---------------------------------------------------------------- reality regresses
echo gone > lib/a
commit "behaviour a breaks"
run 01_a
same "a regression takes the satisfaction away" "verifying" "$(stage)"
same "the criterion that stopped holding is named" "a:failing" "$(unmet)"
expect_exit 10 "$RB" intent realization --intent probe
expect_grep 'closed_work_contradicted  probe: .*criterion `a`'

# ---------------------------------------------------------------- repair
echo ok > lib/a
commit "behaviour a, repaired"
run 01_a
same "repaired reality satisfies it again" "satisfied" "$(stage)"
expect_exit 0 "$RB" intent realization --intent probe

# nothing about the intent was ever written: every stage above was derived
cmp -s .ai/repo/project/intents/probe.yaml "$T/declared.yaml" \
  || { echo "    the intent record changed; a stage was written somewhere"; exit 1; }
same "the list and the realization agree" "$("$RB" intent list --format json | jq -r '.intents[0].stage')" "$(stage)"
