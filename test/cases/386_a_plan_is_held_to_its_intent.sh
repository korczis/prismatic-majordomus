# majordomus-covers: none
# claims: none
# A plan is held to its intent before execution, and a finished milestone is not a plan.
#
# An issue names the intent criterion it serves; coverage from criterion to milestone to
# issue is derived by the intent engine, and the gap a worker observed and the critique of the
# plan are typed records the plan is refused against (ADR 0073). This case builds one intent
# through a fresh `init`, then moves one input at a time and watches `intent validate`:
# a criterion with no work, work with no purpose, a gap that skips a criterion, a condition
# asserted with nothing observed, and work that starts before the plan was critiqued or while
# a blocking finding is open. Each is refused by name with exit 10; each repair returns 0.
#
# It never skips: an executable is required, so a machine without one fails here instead of
# reporting a pass it never measured.
. "$ROOT/test/lib.sh"
command -v jq >/dev/null 2>&1 || { echo "    jq is required"; exit 1; }
RB="$(rust_bin)" || { echo "    no executable: install cargo or set MAJORDOMUS_BIN"; exit 1; }
[ -x "$RB" ] || { echo "    the build produced no executable at $RB"; exit 1; }
MAJORDOMUS_SHARE="$ROOT/share"; export MAJORDOMUS_SHARE

"$MJ" init >/dev/null
for kind in intent gap critique; do
  grep -q "kind: $kind" .ai/repo/knowledge/sources.yaml \
    || { echo "    init did not seed the $kind source class"; exit 1; }
done

mkdir -p .ai/repo/project/milestones .ai/repo/project/issues .ai/repo/project/intents \
  .ai/repo/project/gaps .ai/repo/project/critiques test/cases
printf '. "$ROOT/test/lib.sh"\ntrue\n' > test/cases/01_probe.sh
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
evidence_required: []
YAML
cat > .ai/repo/project/intents/probe.yaml <<'YAML'
id: probe
title: The probe's outcome is true
statement: "The outcome is true for the people it is for."
invariants:
  - The probe stays a valid repository
milestones:
  - probe
satisfaction:
  - id: works
    criterion: The probe works
    evidence: test
    ref: test/cases/01_probe.sh
  - id: documented
    criterion: The probe is documented
    evidence: test
    ref: test/cases/01_probe.sh
YAML
issue() { # <id> <serves-block> [extra-lines]
  cat > ".ai/repo/project/issues/$1.yaml" <<YAML
id: $1
milestone: probe
title: Issue $1
slug: issue-$1
priority: p1
profile: implementation
objective: "Do the work of $1."
scope:
  - src/$1
$2
acceptance_criteria:
  - It is done
validation:
  - "true"
evidence_required:
  - done
${3:-}
YAML
}
commit() { git add -A >/dev/null && git commit -qm "$1"; }
refused() { # <label> <code>
  expect_exit 10 "$RB" intent validate
  expect_grep "FAIL $2 " || { echo "    $1: expected $2"; exit 1; }
}
clean() { # <label>
  "$RB" intent validate >"$T/v.out" 2>&1 || { echo "    $1: intent validate refused:"; sed 's/^/    | /' "$T/v.out"; exit 1; }
}

# --- an intent declared before any work is unplanned, not refused: that is where every
#     intent starts, and it is a different state from a plan that misses a criterion
commit "declare"
clean "an intent nobody has planned yet"
"$RB" intent validate --format json > "$T/v.json"
jq -e '[.findings[] | select(.code == "intent_not_planned" and .subject == "probe")] | length == 1' "$T/v.json" >/dev/null \
  || { echo "    an unplanned intent should be reported once"; cat "$T/v.json"; exit 1; }

# --- once work exists under the intent, a criterion it misses is refused by name
issue I0001 'serves:
  - probe#works'
commit "plan the first"
refused "one criterion uncovered" criterion_uncovered
expect_grep 'probe#documented'

# --- work under the intent's milestone that serves nothing has no purpose
issue I0002 'serves: []'
commit "an issue with no purpose"
refused "a purposeless issue" issue_without_purpose
expect_grep 'I0002'

# --- a link to a criterion that is not there is dangling
issue I0002 'serves:
  - probe#nonexistent'
commit "a dangling link"
refused "a dangling criterion" serves_unknown_criterion

# --- both criteria served: the plan carries the intent
issue I0002 'serves:
  - probe#documented'
commit "plan the second"
clean "a covered plan"
"$RB" intent coverage --format json > "$T/c.json"
jq -e '[.criteria[] | select(.strength == "covered")] | length == 2' "$T/c.json" >/dev/null \
  || { echo "    both criteria should read covered"; cat "$T/c.json"; exit 1; }
jq -e '.issues[] | select(.issue == "I0002") | .origin == "intent" and .serves == ["probe#documented"]' "$T/c.json" >/dev/null \
  || { echo "    I0002 should say why it exists"; exit 1; }

# --- a gap that skips a criterion, or asserts a state with nothing observed, is refused
cat > .ai/repo/project/gaps/probe.yaml <<'YAML'
intent: probe
observed_at: HEAD
recorded_by: case 386
recorded_at: 2026-09-16
observations:
  - id: nothing-yet
    statement: The probe does not exist yet
    source: file:src
conditions:
  - criterion: works
    state: missing
    because: nothing is there
YAML
commit "an incomplete gap"
refused "a skipped criterion" gap_criterion_unanswered
expect_grep 'FAIL condition_without_observation '

cat > .ai/repo/project/gaps/probe.yaml <<'YAML'
intent: probe
observed_at: HEAD
recorded_by: case 386
recorded_at: 2026-09-16
observations:
  - id: nothing-yet
    statement: The probe does not exist yet
    source: file:src
conditions:
  - criterion: works
    state: missing
    because: nothing is there
    observations: [nothing-yet]
  - criterion: documented
    state: unknown
    because: not looked at
work:
  - id: build-it
    title: Build the probe
    serves: [works]
    issue: I0001
YAML
commit "a complete gap"
clean "a complete gap"

# --- work starts before the plan was critiqued: refused
issue I0001 'serves:
  - probe#works' 'started_at: 2026-09-16'
commit "start without a critique"
refused "execution before critique" executing_without_critique

# --- a critique with an open blocking finding still refuses the started work
cat > .ai/repo/project/critiques/probe.yaml <<'YAML'
intent: probe
reviewed_at: HEAD
reviewed_by: case 386
findings:
  - id: no-docs-check
    class: insufficient_work
    subject: probe#documented
    finding: Nothing checks that the documentation exists
    blocking: true
    resolution:
      state: open
YAML
commit "an open blocker"
refused "an open blocking finding" executing_with_open_blocker

# --- planned into the issue that serves the intent: execution may proceed
cat > .ai/repo/project/critiques/probe.yaml <<'YAML'
intent: probe
reviewed_at: HEAD
reviewed_by: case 386
findings:
  - id: no-docs-check
    class: insufficient_work
    subject: probe#documented
    finding: Nothing checks that the documentation exists
    blocking: true
    resolution:
      state: planned
      issue: I0002
YAML
commit "resolve the blocker"
clean "a resolved critique"
