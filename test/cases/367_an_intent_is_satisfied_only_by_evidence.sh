# majordomus-covers: none
# claims: none
# An intent is satisfied only by evidence, and never by the work around it being finished.
#
# An intent names the milestones that realise it and the tests that settle it. Its stage is
# derived from the plan and its satisfaction from the evidence ledger, and nothing stores
# either (ADR 0070). So this case moves each input on its own and watches the derived stage:
# a recorded passing run meets the criterion but satisfies nothing while the milestone is
# open; closing the milestone satisfies it; changing the test after its run takes the
# satisfaction away again, with no file about the intent touched. Then it breaks a link and
# asserts the refusal names it.
#
# It never skips: an executable is required, so a machine without one fails here instead of
# reporting a pass it never measured.
. "$ROOT/test/lib.sh"
command -v jq >/dev/null 2>&1 || { echo "    jq is required"; exit 1; }
RB="$(rust_bin)" || { echo "    no executable: install cargo or set MAJORDOMUS_BIN"; exit 1; }
[ -x "$RB" ] || { echo "    the build produced no executable at $RB"; exit 1; }
MAJORDOMUS_SHARE="$ROOT/share"; export MAJORDOMUS_SHARE

# --- a repository with the layer: `init` seeds the intent class, which is half of what an
#     initialised repository needs to read the kind at all
"$MJ" init >/dev/null
grep -q "kind: intent" .ai/repo/knowledge/sources.yaml \
  || { echo "    init did not seed the intent source class"; exit 1; }

mkdir -p .ai/repo/project/milestones .ai/repo/project/intents test/cases
milestone() { # <evidence-block>
  cat > .ai/repo/project/milestones/probe.yaml <<YAML
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
$1
YAML
}
milestone ""
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
git add -A >/dev/null && git commit -qm install

stage() { "$RB" intent show probe --format json | jq -r '.stage'; }
state() { "$RB" intent show probe --format json | jq -r '.satisfaction[0].state'; }
same() { [ "$2" = "$3" ] || { printf '    %s: expected %s, got %s\n' "$1" "$2" "$3"; exit 1; }; }

expect_exit 0 "$RB" intent validate
expect_grep '1 intent\(s\), 0 failure'
same "nothing recorded" "not_run" "$(state)"
same "an open milestone and no evidence" "planned" "$(stage)"

# --- a passing run of the case that is in the tree, recorded
record() { # <outcome>
  local digest commit
  digest="sha256:$(sha256_of_file test/cases/01_probe.sh)"
  commit="$(git rev-parse HEAD)"
  mkdir -p .ai/repo/evidence
  jq -n --arg d "$digest" --arg c "$commit" --arg o "$1" '{version: 1, executions: [{
      test: "suite:01_probe", runner: "suite", source: "test/cases/01_probe.sh",
      outcome: $o, seconds: 1, commit: $c, working_tree: "clean", digest: $d,
      at: "2026-09-15T00:00:00Z", origin: "local", command: "bash test/run.sh 01_probe"}]}' \
    > .ai/repo/evidence/ledger.json
  git add -A >/dev/null && git commit -qm "record $1"
}
record pass
same "a recorded passing run" "current" "$(state)"
same "evidence while the milestone is open" "planned" "$(stage)"

# --- the milestone's own evidence closes it; now, and only now, the intent is satisfied
milestone 'evidence:
  - covers: reached
    type: command
    command: "true"'
git add -A >/dev/null && git commit -qm "close the milestone"
same "a closed milestone and current evidence" "satisfied" "$(stage)"

# --- the test changes after its run: the run proves nothing about the test that is there
printf '. "$ROOT/test/lib.sh"\nfalse\n' > test/cases/01_probe.sh
git add -A >/dev/null && git commit -qm "change the probe"
same "a run of a test that has since changed" "stale" "$(state)"
same "stale evidence" "verifying" "$(stage)"

# --- a failing run is not evidence either
record fail
same "a failing run" "failing" "$(state)"

# --- a link that resolves to nothing is refused, named, with a non-zero exit
sed 's#test/cases/01_probe.sh#test/cases/404_absent.sh#' .ai/repo/project/intents/probe.yaml > "$T/probe.yaml"
cp "$T/probe.yaml" .ai/repo/project/intents/probe.yaml
git add -A >/dev/null && git commit -qm "break the reference"
expect_exit 10 "$RB" intent validate
expect_grep 'FAIL unresolved_evidence_ref  probe'
expect_grep 'invalid'
same "an unresolved reference" "unresolved" "$(state)"
