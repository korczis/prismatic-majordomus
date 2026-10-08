# majordomus-covers: none
# claims: none
# A resumed task is told when the intent it was working to moved.
#
# A handover carried a commit and nothing of what the work was for, so the next worker
# learned how far the tree had moved and not that the criteria had been rewritten, or the
# critique reopened, in between (ADR 0111). This case starts a bound task under a fresh
# `init`, writes a handover, and resolves it four times against the built executable:
#
#   nothing changed          Intent: unchanged, and an ordinary commit keeps it so
#   the criterion is edited  Intent: plan_changed
#   the critique is edited   Intent: plan_changed
#   the test is recorded     Intent: evidence_moved, with the plan revision untouched
#
# and checks what the record may and may not hold: the names and two pins in its front
# matter, a body that forges a pin refused like every other computed key, a checkpoint
# carrying neither, an unbound task's handover carrying neither and resolving with no
# Intent line, and an unreadable binding answered `unknown`, never `unchanged`.
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
reviewed_by: case 961
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


policy_intent required; commit "required"
handover() { printf '# Objective\nx\n# Current State\ny\n# Next Action\nz\n' | "$MJ" handover "$@"; }
latest() { ls -1 .ai/local/state/handovers/*.md | LC_ALL=C sort | tail -n 1; }
fm() { awk -v k="$1" 'NR==1{next} /^---$/{exit} index($0,k": ")==1{print substr($0,length(k)+3)}' "$(latest)"; }

# --- an unbound task: no pin in the record, no Intent line on resolve
expect_exit 0 "$MJ" start "ops work" --scope src/ops
expect_exit 0 handover --close
[ -z "$(fm bound_issue)$(fm plan_revision)$(fm evidence_standing)" ] \
  || { echo "    an unbound handover carries a pin:"; sed -n 1,20p "$(latest)"; exit 1; }
expect_exit 0 "$MJ" handover --resolve
expect_no_grep '^Intent:'

# --- a bound task: the names and the two pins, and nothing of the intent itself
expect_exit 0 "$MJ" start "probe work" --scope src/probe --issue I0001
plan_pin="$(cur plan_revision)"
# a checkpoint is a note inside the task, not a resumption point: it carries neither
printf 'progress\n' | "$MJ" checkpoint >/dev/null
cp_file="$(ls -1 .ai/local/state/checkpoints/*.md | LC_ALL=C sort | tail -n 1)"
expect_no_grep '^(bound_issue|plan_revision|evidence_standing):' "$cp_file"
# a body that forges a pin is refused, like every other computed key
expect_exit 10 sh -c 'printf "plan_revision: abc\n# Objective\nx\n# Current State\ny\n# Next Action\nz\n" | "$0" handover' "$MJ"
expect_grep 'identity fields'
expect_exit 0 handover --close
[ "$(fm bound_issue)" = I0001 ] && [ "$(fm plan_revision)" = "$plan_pin" ] && [ "${#plan_pin}" = 64 ] \
  || { echo "    the handover does not pin the binding:"; sed -n 1,20p "$(latest)"; exit 1; }
[ "$(fm evidence_standing | wc -c | tr -d ' ')" = 65 ] || { echo "    no evidence pin"; exit 1; }
expect_no_grep '^(stage|verdict|criteria|statement):' "$(latest)"

# --- nothing changed
expect_exit 0 "$MJ" handover --resolve
expect_grep '^Intent: unchanged \(I0001; standing bound\)$'
# an ordinary commit that touches no intent, link or critique keeps it unchanged
echo x > src/loose/file; commit "unrelated work"
expect_exit 0 "$MJ" handover --resolve
expect_grep '^Intent: unchanged'

# --- an unreadable binding is unknown: "unchanged" is a claim, and a claim needs both ends
expect_exit 0 env MAJORDOMUS_BIN="$T/no-such-executable" "$MJ" handover --resolve
expect_grep '^Intent: unknown'

# --- the critique is edited: the plan the work was reviewed against is not the one pinned
cp .ai/repo/project/critiques/probe.yaml "$T/critique.keep"
sed 's/finding: One case may not be enough/finding: One case is not enough and a second is required/' \
  "$T/critique.keep" > .ai/repo/project/critiques/probe.yaml
commit "the critique is revised"
expect_exit 0 "$MJ" handover --resolve
expect_grep '^Intent: plan_changed '
expect_grep 'intent binding --issue I0001'
cp "$T/critique.keep" .ai/repo/project/critiques/probe.yaml; commit "the critique as it was"
expect_exit 0 "$MJ" handover --resolve
expect_grep '^Intent: unchanged'

# --- the evidence moves and the plan does not: the criterion's test is run and recorded
printf '01_probe\tok\t0\tserial\n' > "$T/report.tsv"
"$RB" evidence record --suite "$T/report.tsv" >/dev/null
commit "record 01_probe"
expect_exit 0 "$MJ" handover --resolve
expect_grep '^Intent: evidence_moved '
expect_no_grep 'plan_changed'

# --- the criterion is rewritten: plan_changed outranks whatever the evidence did
sed 's/criterion: The probe works/criterion: The probe works on every platform/' \
  .ai/repo/project/intents/probe.yaml > "$T/intent" && cat "$T/intent" > .ai/repo/project/intents/probe.yaml
commit "the criterion is rewritten"
expect_exit 0 "$MJ" handover --resolve
expect_grep '^Intent: plan_changed '

# --- and a task that starts now is pinned to the plan as it now stands
expect_exit 0 "$MJ" start "probe work again" --scope src/probe --issue I0001
[ "$(cur plan_revision)" != "$plan_pin" ] || { echo "    a rewritten criterion left the plan revision where it was"; exit 1; }
