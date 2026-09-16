# majordomus-covers: plan
# majordomus-negative: plan
# `done` ends the lifecycle; it does not replace it — in both engines.
#
# Until this case neither transition engine checked the status an issue was in before
# completing it. A READY issue could be moved straight to DONE, with no plan_start ever
# recorded, and the shell engine wrote completed_at onto a CANCELLED issue that the
# executable refused. Two writers of one record may exist only while they are one guard, so
# every refusal is asserted through both: `majordomus plan done` (lib/plan.sh) and the
# plan.transition capability over MCP (plan.rs), which is also what HTTP reaches.
#
# A refusal writes nothing, in either engine — not the record, not the ledger — and the
# legal move (ACTIVE -> DONE) still works in both.
. "$ROOT/test/lib.sh"
command -v jq >/dev/null 2>&1 || { echo "    skip: no jq"; exit 0; }
RB="$(rust_bin)" || rust_bin_exit $?
[ -x "$RB" ] || { echo "    the build produced no executable at $RB"; exit 1; }
MAJORDOMUS_SHARE="$ROOT/share"; export MAJORDOMUS_SHARE
S="$(mktemp -d "${TMPDIR:-/tmp}/mj376.XXXXXX")"; trap 'rm -rf "$S"' EXIT
L=.ai/local/state/ledger.jsonl
I=.ai/repo/project/issues

fail=0
note() { printf '    %s\n' "$1"; fail=1; }

tool() { # <tool> <arguments-json>; isError in $TOOL_ERR, the message in $TOOL_TEXT
  {
    printf '{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2025-06-18","capabilities":{},"clientInfo":{"name":"case376","version":"0"}}}\n'
    printf '{"jsonrpc":"2.0","method":"notifications/initialized"}\n'
    printf '{"jsonrpc":"2.0","id":2,"method":"tools/call","params":{"name":"%s","arguments":%s}}\n' "$1" "$2"
  } | "$RB" mcp 2>"$S/mcp.err" > "$S/frames" || { sed 's/^/    | /' "$S/mcp.err"; return 1; }
  TOOL_ERR="$(tail -n 1 "$S/frames" | jq -r '.result.isError')"
  TOOL_TEXT="$(tail -n 1 "$S/frames" | jq -r '.result.content[0].text // ""')"
}
move() { tool majordomus_plan_transition "$(jq -nc --arg i "$1" --arg t "$2" '{issue:$i,transition:$t}')"; }

"$MJ" init >/dev/null 2>&1
pj_init
pj_milestone M000 0
pj_issue I0001 M000   # READY, with its evidence: only its status stands between it and DONE
pj_issue I0002 M000   # CANCELLED, with its evidence
pj_issue I0003 M000   # ACTIVE through the shell, done through the shell
pj_issue I0004 M000   # ACTIVE through the shell, done through the capability
printf 'cancelled: true\n' >> "$I/I0002.yaml"
git add -A >/dev/null 2>&1; git commit -qm fixture >/dev/null 2>&1
for i in I0001 I0002; do
  "$MJ" plan evidence "$i" --covers proof --type test --command true --result ok >/dev/null 2>&1 \
    || note "attaching evidence to $i failed"
done
for i in I0003 I0004; do
  "$MJ" plan start "$i" >/dev/null 2>&1 || note "plan start $i failed"
  "$MJ" plan evidence "$i" --covers proof --type test --command true --result ok >/dev/null 2>&1 \
    || note "attaching evidence to $i failed"
done
git add -A >/dev/null 2>&1; git commit -qm evidence >/dev/null 2>&1

records() { cat "$I"/*.yaml | cksum; }
before_records="$(records)"; before_ledger="$(cksum < "$L")"

# ---------------------------------------------------------------- the refusals, both engines
refused() { # <issue> <status>
  expect_exit 15 "$MJ" plan "done" "$1" || fail=1
  expect_grep "$1 is $2; only an ACTIVE or VERIFY issue can move to DONE" || fail=1
  move "$1" "done" || { note "the MCP server failed on done $1"; return; }
  [ "$TOOL_ERR" = true ] || { note "the capability completed $2 $1"; return; }
  case "$TOOL_TEXT" in
    *"$1 is $2; only an ACTIVE or VERIFY issue can move to DONE"*) ;;
    *) note "the capability refused done on $2 $1 with: $TOOL_TEXT" ;;
  esac
}
refused I0001 READY
refused I0002 CANCELLED
[ "$(pj_status I0001)" = READY ] || note "a refused done moved I0001 to $(pj_status I0001)"
[ "$(pj_status I0002)" = CANCELLED ] || note "a refused done moved I0002 to $(pj_status I0002)"
[ "$(records)" = "$before_records" ] || note "a refused done changed a record"
[ "$(cksum < "$L")" = "$before_ledger" ] || note "a refused done appended to the ledger"
grep -q '^completed_at' "$I/I0002.yaml" && note "the cancelled issue was given a completed_at"

# ---------------------------------------------------------------- the legal move, both engines
expect_exit 0 "$MJ" plan "done" I0003 || fail=1
[ "$(pj_status I0003)" = DONE ] || note "ACTIVE I0003 done through the shell is $(pj_status I0003)"
git add -A >/dev/null 2>&1; git commit -qm "I0003 done" >/dev/null 2>&1
move I0004 "done" || note "the MCP server failed on done I0004"
[ "$TOOL_ERR" = false ] || note "the capability refused done on ACTIVE I0004: $TOOL_TEXT"
[ "$(pj_status I0004)" = DONE ] || note "ACTIVE I0004 done through the capability is $(pj_status I0004)"

# ...and DONE is not a status done may be applied to again: the stamp would be rewritten.
git add -A >/dev/null 2>&1; git commit -qm "I0004 done" >/dev/null 2>&1
expect_exit 15 "$MJ" plan "done" I0004 || fail=1
expect_grep 'I0004 is DONE; only an ACTIVE or VERIFY issue can move to DONE' || fail=1
move I0003 "done" || note "the MCP server failed on done I0003"
[ "$TOOL_ERR" = true ] || note "the capability completed DONE I0003 a second time"
expect_exit 0 "$MJ" plan validate || fail=1

[ "$fail" = 0 ] || exit 1
echo "    done requires a started issue in both engines: READY, CANCELLED and DONE are refused and write nothing"
