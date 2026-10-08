# majordomus-covers: none
# claims: intent-refused-at-plan-start
# plan start asks the binding, in both engines, and says the same thing.
#
# A task is one way work begins and a plan transition is the other. `start` asking what the
# task serves would be a gate with a door beside it if `plan start` still moved an issue to
# ACTIVE on its status alone (ADR 0111). This case builds an intent whose plan was never
# critiqued and a maintenance milestone through a fresh `init`, and moves issues through the
# shell engine (`majordomus plan start`) and through the capability (`plan.transition` over
# MCP, the Rust engine):
#
#   policy key absent   both engines start an issue of the uncritiqued intent, as before
#   required            both refuse it, with the same sentence, and write nothing; a
#                       maintenance issue starts in both; once the plan is critiqued the
#                       refused issues start in both
#   required, blind     the shell engine, unable to reach the executable, refuses: unknown
#                       is never a pass
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
reviewed_by: case 962
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


tool() { # <tool name> <arguments json>: sets TOOL_ERR and TOOL_TEXT
  local name="$1" args="$2" out="$T/tool.out"
  {
    printf '{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2025-06-18","capabilities":{},"clientInfo":{"name":"case-962","version":"0"}}}\n'
    printf '{"jsonrpc":"2.0","method":"notifications/initialized"}\n'
    printf '{"jsonrpc":"2.0","id":2,"method":"tools/call","params":{"name":"%s","arguments":%s}}\n' "$name" "$args"
  } | "$RB" mcp 2>"$T/mcp.err" > "$out" || { echo "    the server failed on $name:"; sed 's/^/    | /' "$T/mcp.err"; exit 1; }
  TOOL_ERR="$(tail -n 1 "$out" | jq -r '.result.isError')"
  TOOL_TEXT="$(tail -n 1 "$out" | jq -r '.result.content[0].text // ""')"
}
move() { tool majordomus_plan_transition "$(jq -nc --arg i "$1" '{issue:$i,transition:"start"}')"; }
same() { [ "$2" = "$3" ] || { echo "    $1: expected $2, got $3"; exit 1; }; }
status() { "$MJ" plan list | awk -v i="$1" '$1==i{print $2}'; }

# four issues of the intent, alike in everything a transition reads, and two of maintenance
issue I0003 probe src/probe 'serves:
  - probe#works'
issue I0004 probe src/probe 'serves:
  - probe#works'
issue I0005 probe src/probe 'serves:
  - probe#works'
issue I0006 ops src/ops 'serves: []'
rm .ai/repo/project/critiques/probe.yaml
policy_intent absent
commit "an intent nobody critiqued, and no intent policy"

# --- key absent: both engines start work on the uncritiqued intent, exactly as before
expect_exit 0 "$MJ" plan start I0001
move I0003; [ "$TOOL_ERR" = false ] || { echo "    with no policy the capability refused: $TOOL_TEXT"; exit 1; }
commit "started with no policy"

# --- required: both refuse, with one sentence, and neither writes
policy_intent required; commit "required"
before="$(cat .ai/repo/project/issues/*.yaml | cksum)"
expect_exit 15 "$MJ" plan start I0004
shell_said="$(printf '%s\n' "$LAST_OUT" | grep 'may not start' | sed 's/^majordomus: //; s/^error: //')"
case "$shell_said" in "I0004 may not start: "*"never critiqued"*) ;; *) echo "    the shell engine said: $LAST_OUT"; exit 1 ;; esac
move I0005
[ "$TOOL_ERR" = true ] || { echo "    the capability started an issue of an uncritiqued intent"; exit 1; }
case "$TOOL_TEXT" in *"I0005 may not start: "*"never critiqued"*) ;; *) echo "    the capability said: $TOOL_TEXT"; exit 1 ;; esac
# the same sentence, the issue id aside
rust_said="$(printf '%s' "$TOOL_TEXT" | grep -o 'I0005 may not start: .*' | head -n 1)"
[ "$(printf '%s' "$shell_said" | sed 's/I0004/ID/g')" = "$(printf '%s' "$rust_said" | sed 's/I0005/ID/g')" ] \
  || { echo "    the engines refuse in different words:"; echo "    | shell: $shell_said"; echo "    | rust:  $rust_said"; exit 1; }
[ "$before" = "$(cat .ai/repo/project/issues/*.yaml | cksum)" ] || { echo "    a refused start wrote a record"; exit 1; }
same "refused work is still READY (shell)" READY "$(status I0004)"
same "refused work is still READY (capability)" READY "$(status I0005)"

# --- required, and the shell engine cannot reach the executable: unknown is not a pass
expect_exit 15 env MAJORDOMUS_BIN="$T/no-such-executable" "$MJ" plan start I0004
expect_grep 'unknown is never a pass'

# --- required: maintenance starts in both
expect_exit 0 "$MJ" plan start I0002
move I0006; [ "$TOOL_ERR" = false ] || { echo "    the capability refused maintenance: $TOOL_TEXT"; exit 1; }
commit "maintenance started"

# --- required: once the plan is critiqued, the refused issues start in both
critique planned; commit "critiqued"
expect_exit 0 "$MJ" plan start I0004
move I0005; [ "$TOOL_ERR" = false ] || { echo "    the capability refused a critiqued plan: $TOOL_TEXT"; exit 1; }
same "started (shell)" ACTIVE "$(status I0004)"
same "started (capability)" ACTIVE "$(status I0005)"
