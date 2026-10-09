# majordomus-covers: none
# claims: none
# A review of another plan authorises nothing.
#
# A critique of last month's plan authorised today's, and a record nobody ran anything over
# read the same as a review (ADR 0112). This case stamps a review under a fresh `init` and
# then asks `majordomus start` and `majordomus plan start`, under policy
# `intent.binding: required`, after each kind of change:
#
#   a title, an objective, an issue finished   the review stands, and work starts
#   a criterion added to the intent            critique_stale: the work is refused until the
#                                              review is run and stamped again
#   a structural failure after the stamp       plan_rejected, although the stamp is current,
#                                              because the structural half is derived at the ask
#   an unstamped critique                      a warning where the policy does not require
#                                              opposition, and work starts; under
#                                              `intent.opposition: required`, a failure of
#                                              `intent validate` and opposition_not_executed
#
# and reads the review's state in the context of a bound task.
#
# It never skips: an executable is required, so a machine without one fails here instead of
# reporting a pass it never measured.
. "$ROOT/test/lib.sh"
command -v jq >/dev/null 2>&1 || { echo "    jq is required"; exit 1; }
RB="$(rust_bin)" || { echo "    no executable: install cargo or set MAJORDOMUS_BIN"; exit 1; }
[ -x "$RB" ] || { echo "    the build produced no executable at $RB"; exit 1; }
MAJORDOMUS_SHARE="$ROOT/share"; export MAJORDOMUS_SHARE
MAJORDOMUS_BIN="$RB"; export MAJORDOMUS_BIN

"$MJ" init >/dev/null
pj_init
mkdir -p .ai/repo/project/intents .ai/repo/project/critiques test/cases src/probe src/ops src/loose
printf '. "$ROOT/test/lib.sh"\ntrue\n' > test/cases/01_probe.sh
milestone() { # <id>
  cat > ".ai/repo/project/milestones/$1.yaml" <<YAML
id: $1
title: Milestone $1
slug: $1
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
}
issue() { # <id> <milestone> <scope> <serves-block>
  cat > ".ai/repo/project/issues/$1.yaml" <<YAML
id: $1
milestone: $2
title: Issue $1
slug: issue-$1
priority: p1
profile: implementation
objective: "Do the work of $1."
scope:
  - $3
$4
acceptance_criteria:
  - It is done
validation:
  - "true"
evidence_required:
  - done
YAML
}
critique() { # <resolution-state>
  cat > .ai/repo/project/critiques/probe.yaml <<YAML
intent: probe
reviewed_at: HEAD
reviewed_by: case 964
findings:
  - id: thin
    class: insufficient_work
    subject: probe#works
    finding: One case may not be enough
    blocking: true
    resolution:
      state: $1
      issue: I0001
YAML
}
policy_intent() { # <binding value, or "absent">: replace the policy's intent block
  # the block init wrote from the skeleton, or the one this function wrote, goes first: a
  # top-level `intent:` and the indented lines under it
  awk '/^intent:[[:space:]]*$/ { skip=1; next } skip && /^[^[:space:]#]/ { skip=0 } !skip { print }' \
    .ai/repo/policy.yaml > "$T/policy" && cat "$T/policy" > .ai/repo/policy.yaml
  [ "$1" = absent ] || cat >> .ai/repo/policy.yaml <<YAML
intent:
  binding: $1
  exemptions:
    - id: emergency
      description: Restoring a broken trunk
YAML
}
commit() { git add -A >/dev/null && git commit -qm "$1"; }
cur() { sed -n "s/^$1: //p" .ai/local/state/current.yaml; }
end_task() { "$MJ" handover --derive --close >/dev/null 2>&1 || true; }
started() { # the last task.started event, as JSON
  grep '"event":"task.started"' .ai/local/state/ledger.jsonl | tail -n 1
}

milestone probe
milestone ops
cat > .ai/repo/project/intents/probe.yaml <<'YAML'
id: probe
title: The probe's outcome is true
statement: "The outcome is true for the people it is for."
invariants:
  - The probe stays a valid repository
non_goals:
  - Speed
milestones:
  - probe
satisfaction:
  - id: works
    criterion: The probe works
    evidence: test
    ref: test/cases/01_probe.sh
YAML
issue I0001 probe src/probe 'serves:
  - probe#works'
issue I0002 ops src/ops 'serves: []'
critique planned
commit "one intent, one maintenance milestone"
expect_exit 0 "$RB" intent validate


same() { [ "$2" = "$3" ] || { echo "    $1: expected $2, got $3"; exit 1; }; }
opp() { "$RB" intent oppose probe --format json 2>/dev/null; }
field() { opp | jq -r "$1"; }
stamp() { "$RB" intent stamp probe --format json > "$T/stamp.json" 2>"$T/stamp.err"; }
C=.ai/repo/project/critiques/probe.yaml

policy() { # <binding> <opposition|absent>
  awk '/^intent:[[:space:]]*$/ { skip=1; next } skip && /^[^[:space:]#]/ { skip=0 } !skip { print }' \
    .ai/repo/policy.yaml > "$T/policy" && cat "$T/policy" > .ai/repo/policy.yaml
  { printf 'intent:\n  binding: %s\n' "$1"; [ "$2" = absent ] || printf '  opposition: %s\n' "$2"; } >> .ai/repo/policy.yaml
}
start_probe() { "$MJ" start "probe work" --scope src/probe --issue I0001; }

# --- unstamped, opposition not required: a warning, and work starts as it always did
policy required absent; commit "binding required"
expect_exit 0 "$RB" intent validate
expect_grep 'WARN critique_not_stamped  probe'
expect_exit 0 start_probe
end_task

# --- unstamped, opposition required: validation fails, and the work is refused
policy required required; commit "opposition required"
expect_exit 10 "$RB" intent validate
expect_grep 'FAIL critique_not_stamped  probe'
expect_exit 15 start_probe
expect_grep 'opposition_not_executed'
expect_grep 'intent stamp probe'
expect_exit 15 "$MJ" plan start I0001
expect_grep 'never stamped'

# --- and a required change nobody is named as having accepted is not accepted
expect_exit 10 "$RB" intent validate
expect_grep 'FAIL resolution_names_no_resolver  probe:thin'
sed 's/^      issue: I0001$/      issue: I0001\
      resolved_by: case 964/' "$C" > "$T/critique" && cat "$T/critique" > "$C"
commit "the resolution names its resolver"

# --- stamped: it validates, binds, and the worker is told where the review stands
stamp || { echo "    the stamp failed:"; cat "$T/stamp.err"; exit 1; }
commit "stamped"
expect_exit 0 "$RB" intent validate
expect_exit 0 start_probe
expect_exit 0 "$MJ" context
expect_grep '^review       probe  current  accept_with_required_changes  stamped [0-9a-f]{12}$'
end_task

# --- what a review does not judge leaves it standing: a title, an objective, an issue moving
sed -e 's/^title: Issue I0001/title: Issue I0001, renamed/' -e 's/^objective: .*/objective: "Do it well."/' \
  .ai/repo/project/issues/I0001.yaml > "$T/issue" && cat "$T/issue" > .ai/repo/project/issues/I0001.yaml
commit "a title and an objective"
same "after a title edit" current "$(field .review.state)"
expect_exit 0 "$MJ" plan start I0001
commit "the issue is ACTIVE"
same "after the issue started" current "$(field .review.state)"
expect_exit 0 start_probe
end_task

# --- a criterion is added, with an issue that serves it: the plan is another plan
cat >> .ai/repo/project/intents/probe.yaml <<'YAML'
  - id: stays-fast
    criterion: The probe answers within a second
    evidence: test
    ref: test/cases/01_probe.sh
YAML
issue I0007 probe src/probe 'serves:
  - probe#stays-fast'
commit "a second criterion"
same "after a criterion was added" stale "$(field .review.state)"
expect_exit 10 "$RB" intent validate
expect_grep 'FAIL critique_stale  probe'
expect_exit 15 start_probe
expect_grep 'critique_stale'
expect_grep 'reviewed another plan'
expect_exit 15 "$MJ" plan start I0007
# reviewed again and stamped, it starts
stamp || { echo "    the second stamp failed:"; cat "$T/stamp.err"; exit 1; }
commit "reviewed again"
expect_exit 0 start_probe
end_task

# --- a structural failure after the stamp refuses although the stamp is current: the test a
#     criterion names is deleted. Nothing a review hashes changed, so the stamp still names
#     this plan; the reference no longer resolves, and that is derived at the ask
git rm -q test/cases/01_probe.sh
commit "the criterion's test is gone"
same "the stamp is still of this plan" current "$(field .review.state)"
same "the opposition now" reject "$(field .disposition)"
opp | jq -e '[.structural[] | select(.blocking) | .id] | index("unresolved_evidence_ref")' >/dev/null \
  || { echo "    the unresolved reference is not a blocking structural finding:"; opp | jq .structural; exit 1; }
expect_exit 15 start_probe
expect_grep 'plan_rejected'
expect_no_grep 'critique_stale'
