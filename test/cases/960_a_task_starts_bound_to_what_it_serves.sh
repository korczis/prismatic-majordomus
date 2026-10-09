# majordomus-covers: none
# claims: intent-in-session-context
# A task starts bound to what it serves, and the worker is briefed from the binding.
#
# `intent preflight` could say which intent an issue serves and nothing asked it: `start`
# took a description and a scope, the task record named no work, and the context a worker
# was handed said nothing of the criteria the work was for (ADR 0111). This case builds one
# intent, one maintenance milestone and one exemption class through a fresh `init`, and
# drives the shell tool against the built executable, one policy value at a time:
#
#   key absent   `start` with no new flag behaves exactly as before and asks nothing; a flag
#                is still a question, so `--issue` is asked and a refused answer does not start
#   advisory     a scope no issue covers is reported and the task starts
#   required     the same scope does not start; an issue of a critiqued intent starts bound,
#                with what it named and two pins in the record and the ledger; an intent whose
#                plan was never critiqued refuses the work that serves it; an exemption starts
#                only as a declared class with a reason; an unreadable binding is unknown,
#                and unknown does not start
#
# and reads `context` for the bound task: statement, invariant, the served criterion with
# the state of its evidence, the critique — and for the exempt one, the class and reason.
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
reviewed_by: case 960
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

# --- key absent: nothing is asked, and the record and the event are what they always were
policy_intent absent; commit "no intent policy" 2>/dev/null || true
expect_exit 0 "$MJ" start "loose work" --scope src/loose
expect_no_grep '^binding '
[ -z "$(cur issue)$(cur intent)$(cur binding)$(cur plan_revision)" ] \
  || { echo "    an unasked start wrote binding fields:"; cat .ai/local/state/current.yaml; exit 1; }
started | jq -e 'has("binding") | not' >/dev/null \
  || { echo "    an unasked start put a binding in the ledger:"; started; exit 1; }
end_task

# --- key absent, and a flag is still a question: an issue this plan does not hold is refused
expect_exit 15 "$MJ" start "named work" --scope src/probe --issue I9999
expect_grep 'unknown_issue'
expect_grep '\-\-issue'
[ ! -f .ai/local/state/current.yaml ] || [ "$(cur outcome)" != active ] \
  || { echo "    a refused start left an active task"; exit 1; }
# --because alone names no exemption
expect_exit 2 "$MJ" start "x" --scope src/loose --because "no class"

# --- advisory: a scope no issue covers is reported, and the task starts
policy_intent advisory; commit "advisory"
expect_exit 0 "$MJ" start "loose work" --scope src/loose
expect_grep 'no_issue_covers_paths'
expect_grep 'advisory'
[ "$(cur binding)" = refused ] || { echo "    advisory did not record the refused standing"; cat .ai/local/state/current.yaml; exit 1; }
end_task

# --- required: the same scope does not start, and the refusal names the three ways through
policy_intent required; commit "required"
expect_exit 15 "$MJ" start "loose work" --scope src/loose
expect_grep 'no_issue_covers_paths'
expect_grep '\-\-issue'; expect_grep '\-\-intent'; expect_grep '\-\-exempt'

# --- required: derived maintenance starts, by its scope alone
expect_exit 0 "$MJ" start "ops work" --scope src/ops
expect_grep '^binding  maintenance'
end_task

# --- required: an issue of a critiqued intent starts bound, with what it named and two pins
expect_exit 0 "$MJ" start "probe work" --scope src/probe --issue I0001
expect_grep '^binding  bound  serves probe'
[ "$(cur issue)" = I0001 ] && [ "$(cur binding)" = bound ] \
  || { echo "    the record does not carry what was named:"; cat .ai/local/state/current.yaml; exit 1; }
plan_pin="$(cur plan_revision)"; ev_pin="$(cur evidence_standing)"
[ "${#plan_pin}" = 64 ] && [ "${#ev_pin}" = 64 ] \
  || { echo "    the record carries no pins:"; cat .ai/local/state/current.yaml; exit 1; }
# names and pins, and nothing of the intent itself
expect_no_grep '^(stage|verdict|criteria|statement):' .ai/local/state/current.yaml
started | jq -e --arg p "$plan_pin" '.issue == "I0001" and .binding == "bound" and .plan_revision == $p' >/dev/null \
  || { echo "    the ledger does not carry the binding:"; started; exit 1; }
# the record still loads without a warning about unknown keys
expect_exit 0 "$MJ" check
expect_no_grep 'unknown key|not in the allowlist'

# --- the worker is briefed from the binding
expect_exit 0 "$MJ" context
expect_grep '^## INTENT'
expect_grep '^standing     bound$'
expect_grep '^intent       probe '
expect_grep '^statement    The outcome is true for the people it is for\.$'
expect_grep '^criterion    works  not_run  — The probe works$'
expect_grep '^invariant    The probe stays a valid repository$'
expect_grep '^critique     reviewed at HEAD; open blocking: none$'
order=$(printf '%s\n' "$LAST_OUT" | grep -E '^## ' | tr '\n' ' ')
case "$order" in "## GIT "*"## TASK ## PROFILE"*"## INTENT"*) ;; *) echo "    INTENT is not after PROFILE: $order"; exit 1 ;; esac
"$MJ" --json context > "$T/c.json"
jq -e '[.sections[].id] | index("intent")' "$T/c.json" >/dev/null || { echo "    no intent section in the JSON"; exit 1; }
# a budget that drops history never drops what the work is for
"$MJ" --json context --budget-lines 60 > "$T/d.json" || true
jq -e '(.budget.dropped | index("intent")) == null' "$T/d.json" >/dev/null || { echo "    the intent section was dropped for budget"; exit 1; }

# --- an unreadable binding is said to be unknown in the briefing, never "no intent"
expect_exit 0 env MAJORDOMUS_BIN="$T/no-such-executable" "$MJ" context
expect_grep '^standing     unknown'
expect_grep '^named        I0001$'
end_task

# --- required: the intent named directly resolves to its open issue
expect_exit 0 "$MJ" start "probe work by intent" --scope src/probe --intent probe
[ "$(cur intent)" = probe ] && [ "$(cur plan_revision)" = "$plan_pin" ] \
  || { echo "    naming the intent did not resolve to the same plan revision"; cat .ai/local/state/current.yaml; exit 1; }
end_task
expect_exit 15 "$MJ" start "x" --scope src/probe --intent no-such-intent
expect_grep 'unknown_intent'

# --- required: an exemption is a declared class with a reason, recorded in the ledger
expect_exit 15 "$MJ" start "hotfix" --scope src/loose --exempt whim --because "I want to"
expect_grep 'unknown_exemption'
expect_exit 15 "$MJ" start "hotfix" --scope src/loose --exempt emergency
expect_grep 'exemption_without_reason'
expect_exit 15 "$MJ" start "hotfix" --scope src/probe --issue I0001 --exempt emergency --because "both"
expect_grep 'exemption_names_work'
expect_exit 0 "$MJ" start "hotfix" --scope src/loose --exempt emergency --because "the trunk is red on case 500"
expect_grep '^binding  exempt  emergency — the trunk is red on case 500$'
[ "$(cur exemption)" = emergency ] || { echo "    the record does not carry the exemption"; exit 1; }
started | jq -e '.exemption == "emergency" and .because == "the trunk is red on case 500" and .binding == "exempt"' >/dev/null \
  || { echo "    the ledger does not carry the exemption and its reason:"; started; exit 1; }
expect_exit 0 "$MJ" context
expect_grep '^exempt       emergency — the trunk is red on case 500$'
end_task

# --- required: an unreadable binding is unknown, and unknown does not start
expect_exit 15 env MAJORDOMUS_BIN="$T/no-such-executable" "$MJ" start "probe work" --scope src/probe --issue I0001
expect_grep 'unknown is never a pass'
# under advisory the same blindness is a warning, and the task starts unbound
policy_intent advisory; commit "advisory again"
expect_exit 0 env MAJORDOMUS_BIN="$T/no-such-executable" "$MJ" start "ops work" --scope src/ops
expect_grep 'binding is unknown'
[ -z "$(cur binding)" ] || { echo "    an unknown binding was recorded as a standing"; exit 1; }
end_task
policy_intent required; commit "required again"

# --- required: a plan never critiqued refuses the work that serves it, at start
rm .ai/repo/project/critiques/probe.yaml; commit "no critique"
expect_exit 15 "$MJ" start "probe work" --scope src/probe --issue I0001
expect_grep 'intent_not_critiqued'
# and a blocking finding left open does too
critique open; commit "an open blocker"
expect_exit 15 "$MJ" start "probe work" --scope src/probe --issue I0001
expect_grep 'open_blocking_finding'

# --- doctor has nothing to say about the policy key, the block or the record's new fields
critique planned; commit "critiqued again"
"$MJ" doctor > "$T/doctor.out" 2>&1 || true
expect_no_grep '^FAIL +policy' "$T/doctor.out"
expect_no_grep 'unknown key' "$T/doctor.out"
