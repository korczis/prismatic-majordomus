# majordomus-covers: none
# claims: none
# A finished task does not satisfy an intent: completion discharges obligations, and only the
# evidence settles a criterion (ADR 0107, project.completion-never-decides-satisfaction).
#
# The fixture puts every input of the old plan in place at once: the milestone the intent
# serves is DONE, and a task is finished with outcome `completed`. The criterion's test has no
# recorded run. The finish must succeed and change nothing about the intent: the stage stays
# `verifying`, the criterion stays `not_run`, and no file under the intents directory moves.
# Then a passing run is recorded, and that alone satisfies the intent — so the fixture can be
# satisfied, and the finish is not failing to satisfy it for a reason of its own. The verdict,
# which the criteria alone derive (ADR 0107), moves with the evidence and never with the finish:
# `unsatisfied` before and after it, `satisfied` once the run is recorded.
#
# It never skips: an executable is required, so a machine without one fails here instead of
# reporting a pass it never measured.
. "$ROOT/test/lib.sh"
command -v jq >/dev/null 2>&1 || { echo "    jq is required"; exit 1; }
RB="$(rust_bin)" || { echo "    no executable: install cargo or set MAJORDOMUS_BIN"; exit 1; }
[ -x "$RB" ] || { echo "    the build produced no executable at $RB"; exit 1; }
MAJORDOMUS_SHARE="$ROOT/share"; export MAJORDOMUS_SHARE

"$MJ" init >/dev/null; "$MJ" update >/dev/null
mkdir -p .ai/repo/project/milestones .ai/repo/project/intents test/cases lib
# A DONE milestone: its own evidence closes it, as in case 367.
cat > .ai/repo/project/milestones/probe.yaml <<'YAML'
id: probe
title: The probe reaches its outcome
slug: probe
order: 0
priority: p1
problem: "A problem."
outcome: "An outcome."
acceptance_criteria:
  - It is reached
validation:
  - "true"
evidence_required:
  - reached
evidence:
  - covers: reached
    type: command
    command: "true"
YAML
printf '. "$ROOT/test/lib.sh"\ntrue\n' > test/cases/01_probe.sh
cat > .ai/repo/project/intents/probe.yaml <<'YAML'
id: probe
title: The probe's outcome is true
statement: "The outcome is true for the people it is for."
invariants:
  - The probe stays a valid repository
milestones:
  - probe
satisfaction:
  - id: probe-passes
    criterion: The probe's case passes
    evidence: test
    ref: test/cases/01_probe.sh
YAML
echo a > lib/a
git add -A >/dev/null && git commit -qm install

stage() { "$RB" intent show probe --format json | jq -r '.stage'; }
state() { "$RB" intent show probe --format json | jq -r '.satisfaction[0].state'; }
verdict() { "$RB" intent show probe --format json | jq -r '.verdict.state'; }
same() { [ "$2" = "$3" ] || { printf '    %s: expected %s, got %s\n' "$1" "$2" "$3"; exit 1; }; }
D="$(mktemp -d "${TMPDIR:-/tmp}/mj-895.XXXXXX")"; trap 'rm -rf "$D"' EXIT
intents_digest() { # every intent file, names and contents, as one digest
  find .ai/repo/project/intents -type f | sort | while read -r f; do echo "$f"; cat "$f"; done \
    > "$D/intents"
  sha256_of_file "$D/intents"
}

expect_exit 0 "$RB" intent validate
same "a DONE milestone and no recorded run" "verifying" "$(stage)"
same "no recorded run" "not_run" "$(state)"
same "the verdict with no recorded run" "unsatisfied" "$(verdict)"
before="$(intents_digest)"

# --- a task is started, does its work and is finished as completed
"$MJ" start "advance the probe" --scope lib >/dev/null
echo b >> lib/a
git add -A >/dev/null && git commit -qm "advance the probe"
printf '# Objective\no\n# Current State\nc\n# Next Action\nn\n' | "$MJ" handover >/dev/null
expect_exit 0 "$MJ" finish --outcome completed --verify-command "grep -q b lib/a"
expect_grep '^outcome: completed$' .ai/local/state/current.yaml

# --- and the intent is exactly as the plan and the evidence derive it: not satisfied
same "after a completed finish, the stage" "verifying" "$(stage)"
same "after a completed finish, the criterion" "not_run" "$(state)"
same "after a completed finish, the verdict" "unsatisfied" "$(verdict)"
# closed work and an unsatisfied verdict disagree, and the realization names the criterion
expect_exit 0 "$RB" intent realization --intent probe
expect_grep 'WARN closed_work_not_satisfied  probe: .*verdict is unsatisfied: held back by `probe-passes` \(test not_run\)'
same "the intents directory after the finish" "$before" "$(intents_digest)"
[ -z "$(git status --porcelain -- .ai/repo/project/intents)" ] \
  || { echo "    the finish left a change under .ai/repo/project/intents:"
       git status --porcelain -- .ai/repo/project/intents | sed 's/^/      /'; exit 1; }

# --- a recorded passing run of the criterion's test is what satisfies it
digest="sha256:$(sha256_of_file test/cases/01_probe.sh)"
commit="$(git rev-parse HEAD)"
mkdir -p .ai/repo/evidence
jq -n --arg d "$digest" --arg c "$commit" '{version: 1, executions: [{
    test: "suite:01_probe", runner: "suite", source: "test/cases/01_probe.sh",
    outcome: "pass", seconds: 1, commit: $c, working_tree: "clean", digest: $d,
    at: "2026-10-05T00:00:00Z", origin: "local", command: "bash test/run.sh 01_probe"}]}' \
  > .ai/repo/evidence/ledger.json
git add -A >/dev/null && git commit -qm "record the probe's run"
same "a recorded passing run" "current" "$(state)"
same "a DONE milestone and current evidence" "satisfied" "$(stage)"
same "the verdict once the passing run is recorded" "satisfied" "$(verdict)"
