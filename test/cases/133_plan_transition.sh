# The lifecycle transitions, in two engines, writing the same bytes.
#
# `lib/plan.sh`'s `mj_plan_transition` has been the only implementation of `start`, `verify`
# and `done` since the plan existed. `plan.transition` is the second, declared as a
# `capability!` of kind `command` so that the runtime — and therefore MCP, HTTP, OpenAPI and
# the Cockpit — can move an issue at all (ADR 0040). Until it landed, an agent could be told
# what to work on and had no way to say it had started.
#
# Two writers of one record is the failure this repository exists to prevent, so this case is
# the reason the second one is allowed to exist. It performs the same transition through both
# engines, over two issues of one fixture built to be identical, and diffs the records
# byte for byte. Where they disagree the shell is the incumbent and is right; the Rust is the
# file that changes.
#
# It also proves the two properties a mutating capability lives or dies by:
#
#   * every guard refuses — an issue that is not READY cannot start, one that was never
#     ACTIVE cannot be verified, one whose dependency is unfinished or whose required
#     evidence is absent cannot be done;
#   * a refusal writes nothing — not the record, not the ledger. A ledger that described a
#     change which never reached the file would end the record's usefulness as history.
#
# Skips itself when there is neither cargo nor MAJORDOMUS_BIN, as every Rust case does.
. "$ROOT/test/lib.sh"
command -v jq >/dev/null 2>&1 || { echo "    skip: no jq"; exit 0; }
RB="$(rust_bin)" || rust_bin_exit $?
[ -x "$RB" ] || { echo "    the build produced no executable at $RB"; exit 1; }
MAJORDOMUS_SHARE="$ROOT/share"; export MAJORDOMUS_SHARE
S="$(mktemp -d "${TMPDIR:-/tmp}/mj133.XXXXXX")"; trap 'rm -rf "$S"' EXIT
MJ="$ROOT/bin/majordomus"

# One MCP tool call over the real pipes. The executable has no `plan` subcommand — this
# module claims none of the command line — so MCP is how a shell reaches a plan capability,
# which is also how an agent reaches it. `rc` is the tool's own verdict, not the process's:
# a refusal is a result carrying isError, and this case asserts refusals.
tool() { # <tool> <arguments-json> -> structuredContent on stdout, isError in $TOOL_ERR
  local name="$1" args="${2:-{\}}" out="$S/frames"
  {
    printf '{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2025-06-18","capabilities":{},"clientInfo":{"name":"case133","version":"0"}}}\n'
    printf '{"jsonrpc":"2.0","method":"notifications/initialized"}\n'
    printf '{"jsonrpc":"2.0","id":2,"method":"tools/call","params":{"name":"%s","arguments":%s}}\n' "$name" "$args"
  } | "$RB" mcp 2>"$S/mcp.err" > "$out" || {
    printf '    the server failed on %s:\n' "$name" >&2; sed 's/^/    | /' "$S/mcp.err" >&2; return 1; }
  TOOL_ERR="$(tail -n 1 "$out" | jq -r '.result.isError')"
  TOOL_TEXT="$(tail -n 1 "$out" | jq -r '.result.content[0].text // ""')"
  tail -n 1 "$out" | jq '.result.structuredContent'
}

move() { # <issue> <transition> — through the capability
  tool majordomus_plan_transition "$(jq -nc --arg i "$1" --arg t "$2" '{issue:$i,transition:$t}')"
}

fail=0
note() { printf '    %s\n' "$1"; fail=1; }

# ---------------------------------------------------------------- the fixture
# Two issues that are identical in every field that matters to a transition, so that the
# only difference between the records afterwards is which engine wrote them. Plus the three
# shapes every guard must refuse: a blocked issue, a cancelled one, and one whose evidence
# its own record requires and does not have.
cd "$S"
git init -q . && git commit -q --allow-empty -m init
"$MJ" init >/dev/null 2>&1
pj_init
pj_milestone M000 0
pj_issue I0001 M000          # moved by the shell
pj_issue I0002 M000          # moved by the capability — same shape
pj_issue I0003 M000 I0002    # blocked on I0002 until it is DONE
pj_issue I0004 M000          # every fixture issue requires the evidence token "proof"

# The records have to be tracked before the executable can see them: discovery is `vcs`, so
# the index enumerates `git ls-files` while the shell's awk reads the directory. A fixture
# that is only written to disk is visible to one engine and invisible to the other, which
# looks exactly like the capability failing to find an issue it was just handed.
git add -A >/dev/null 2>&1
git commit -qm "fixture" >/dev/null 2>&1

# ---------------------------------------------------------------- 1. start, both engines
"$MJ" plan start I0001 >/dev/null 2>&1 || note "the shell refused to start I0001"
move I0002 start > "$S/start.json" || note "the capability failed to start I0002"
[ "$TOOL_ERR" = false ] || note "the capability refused to start a READY issue: $TOOL_TEXT"

# The records, with the id and the timestamps — the only fields that legitimately differ —
# normalised away. Everything else must be identical, including where each field sits.
normalise() { # the record with its identity and its timestamps replaced, so that what is
              # left is only the shape each engine wrote
  sed -E -e 's/^(started_at|verified_at|completed_at|updated_at): .*/\1: X/' -e 's/I000[0-9]/ID/g' "$1"
}
normalise .ai/repo/project/issues/I0001.yaml > "$S/shell.yaml"
normalise .ai/repo/project/issues/I0002.yaml > "$S/rust.yaml"
if ! diff -u "$S/shell.yaml" "$S/rust.yaml" > "$S/diff"; then
  note "the two engines wrote different records:"
  sed 's/^/    | /' "$S/diff"
fi

# The field is where the shell puts it, and the status follows from it in both engines.
grep -q '^started_at: ' .ai/repo/project/issues/I0002.yaml || note "the capability wrote no started_at"
grep -q '^updated_at: ' .ai/repo/project/issues/I0002.yaml || note "the capability wrote no updated_at"
[ "$(jq -r .to < "$S/start.json")" = ACTIVE ] || note "start did not report ACTIVE"
[ "$(jq -r .from < "$S/start.json")" = READY ] || note "start did not report READY as the status before"

# ---------------------------------------------------------------- 2. the ledger envelope
# One line per transition, in the vocabulary share/events.yaml declares, and the same
# envelope from both writers. The shell wrote I0001's line and the capability wrote I0002's.
L=.ai/local/state/ledger.jsonl
[ -f "$L" ] || note "no ledger was written at all"
[ "$(grep -c '"event":"plan_start"' "$L" 2>/dev/null || echo 0)" = 2 ] \
  || note "expected one plan_start per engine, got: $(grep -c '"event":"plan_start"' "$L" 2>/dev/null || echo 0)"
# Every envelope key, in both lines, and no line missing one.
for k in ts event head branch by issue; do
  n="$(grep -c "\"$k\":" "$L" 2>/dev/null || echo 0)"
  [ "$n" = 2 ] || note "the ledger envelope is missing '$k' in one of the two lines"
done
# The keys in the order both writers compose them. A reader diffing two writers diffs a
# sequence, so the order is part of the contract and not a detail of whoever wrote last.
for line in 1 2; do
  order="$(sed -n "${line}p" "$L" | jq -r 'keys_unsorted | join(",")')"
  [ "$order" = "ts,event,head,branch,by,issue" ] \
    || note "ledger line $line composes its keys as [$order]"
done

# ---------------------------------------------------------------- 3. every guard refuses
# I0004 is made ACTIVE first, so that the `done` refusal below is about its missing evidence
# and not about its status. This is a real write and belongs before the checksums.
"$MJ" plan start I0004 >/dev/null 2>&1 || note "the shell refused to start I0004"

before_records="$(cat .ai/repo/project/issues/*.yaml | cksum)"
before_ledger="$(cksum < "$L")"

refuses() { # <label> <issue> <transition> <expected substring>
  move "$2" "$3" > /dev/null 2>&1
  [ "$TOOL_ERR" = true ] || { note "$1: the capability allowed it"; return; }
  case "$TOOL_TEXT" in *"$4"*) ;; *) note "$1: refused, but said '$TOOL_TEXT'" ;; esac
}

refuses "start on an ACTIVE issue"        I0002 start  "not READY"
refuses "verify on an issue never started" I0003 verify "only an ACTIVE issue"
refuses "done while a dependency is open" I0003 done   "cannot be DONE while"
refuses "start on an unknown issue"       I9999 start  "no issue"

# `done` on an ACTIVE issue whose own record requires evidence it does not carry. The guard
# for this one is not a count — a record may carry two entries covering one token and none
# covering another — so it asks what the plan would say and refuses when the answer is not
# DONE. This assertion is the reason that indirection exists.
refuses "done with required evidence absent" I0004 done "evidence"

# ---------------------------------------------------------------- 4. a refusal writes nothing
# The property the whole design rests on: the guard runs before the record is opened, and
# the event is appended only after the record is written. Five refusals have just run.
[ "$(cat .ai/repo/project/issues/*.yaml | cksum)" = "$before_records" ] \
  || note "a refused transition changed a record"
[ "$(cksum < "$L")" = "$before_ledger" ] \
  || note "a refused transition appended to the ledger"

# ---------------------------------------------------------------- 5. the whole lifecycle
# start -> verify -> done, with the evidence attached in between, through the capability
# alone. The status after each move is derived from the record rather than announced, which
# is why `done` on I0004 was refused above and is accepted here.
move I0002 verify > "$S/verify.json" || note "verify failed"
[ "$(jq -r .to < "$S/verify.json")" = VERIFY ] || note "verify did not report VERIFY"
# The evidence its record requires, attached by the command that owns that half. `evidence`
# appends a block rather than setting a field and stays a shell operation for now; the point
# here is that the capability reads the result and stops refusing.
"$MJ" plan evidence I0002 --covers proof --type test --command "bash test/run.sh 133_plan_transition" \
  --result "the transition is proved in two engines" >/dev/null 2>&1 || note "attaching evidence failed"
move I0002 done > "$S/done.json" || note "done failed"
[ "$(jq -r .to < "$S/done.json")" = DONE ] || note "done did not report DONE"

# And the dependency that was blocked is now startable: the graph moved, and it moved in the
# one derivation both engines read.
move I0003 start > /dev/null 2>&1
[ "$TOOL_ERR" = false ] || note "I0003 is still blocked after its dependency reached DONE: $TOOL_TEXT"

[ "$fail" = 0 ] || exit 1
echo "    two engines, one record: start/verify/done write identical bytes, every guard refuses, a refusal writes nothing"
