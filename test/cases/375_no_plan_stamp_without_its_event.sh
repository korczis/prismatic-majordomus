# majordomus-covers: plan
# majordomus-negative: plan
# No plan stamp without its event (project.no-plan-stamp-without-its-event, ADR 0072).
#
# An issue's status is derived from the stamps in its record, and the transition is the only
# writer allowed to put one there: it checks the move, writes the stamp and its seal, and
# appends the plan_* event. Before this case a stamp written into the YAML by hand derived
# DONE, `plan validate` exited 0 and the ledger held nothing. The ledger is checkout-local,
# so the proof a clone can check is the seal the transition writes beside the stamp.
#
#   1. the CLI moves an issue and the record it leaves validates, sealed, with its events;
#   2. a hand-written completed_at (with started_at, verified_at and an evidence entry) and
#      no event is refused by `plan validate` — exit 10, one finding per stamp naming the
#      issue and the missing event — in the shell engine and in the executable, and derives
#      no progress;
#   3. a seal copied from another stamp does not prove this one;
#   4. the capability surface (MCP, and HTTP through the same handler) writes the seal and
#      the event too: no surface writes a stamp without them;
#   5. the transition heals a forged start; a record's unsealed_stamps excuses only the
#      stamps it names, and none written at or after the instant seals began.
. "$ROOT/test/lib.sh"
command -v jq >/dev/null 2>&1 || { echo "    skip: no jq"; exit 0; }
RB="$(rust_bin)" || rust_bin_exit $?
[ -x "$RB" ] || { echo "    the build produced no executable at $RB"; exit 1; }
MAJORDOMUS_SHARE="$ROOT/share"; export MAJORDOMUS_SHARE
S="$(mktemp -d "${TMPDIR:-/tmp}/mj375.XXXXXX")"; trap 'rm -rf "$S"' EXIT
L=.ai/local/state/ledger.jsonl
I=.ai/repo/project/issues

fail=0
note() { printf '    %s\n' "$1"; fail=1; }

tool() { # <tool> <arguments-json> -> structuredContent; isError in $TOOL_ERR
  {
    printf '{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2025-06-18","capabilities":{},"clientInfo":{"name":"case375","version":"0"}}}\n'
    printf '{"jsonrpc":"2.0","method":"notifications/initialized"}\n'
    printf '{"jsonrpc":"2.0","id":2,"method":"tools/call","params":{"name":"%s","arguments":%s}}\n' "$1" "$2"
  } | "$RB" mcp 2>"$S/mcp.err" > "$S/frames" || { sed 's/^/    | /' "$S/mcp.err"; return 1; }
  TOOL_ERR="$(tail -n 1 "$S/frames" | jq -r '.result.isError')"
  tail -n 1 "$S/frames" | jq '.result.structuredContent'
}

"$MJ" init >/dev/null 2>&1
pj_init
pj_milestone M000 0
pj_issue I0001 M000   # moved by the CLI
pj_issue I0002 M000   # forged by hand
pj_issue I0003 M000   # moved by the capability
pj_issue I0004 M000   # forged start, healed by the transition
git add -A >/dev/null 2>&1; git commit -qm fixture >/dev/null 2>&1

# ---------------------------------------------------------------- 1. the CLI, sealed
"$MJ" plan start I0001 >/dev/null 2>&1 || note "plan start I0001 failed"
"$MJ" plan evidence I0001 --covers proof --type test --command true --result ok >/dev/null 2>&1 \
  || note "plan evidence I0001 failed"
"$MJ" plan "done" I0001 >/dev/null 2>&1 || note "plan done I0001 failed"
[ "$(pj_status I0001)" = DONE ] || note "I0001 moved through the CLI is $(pj_status I0001), not DONE"
for f in started completed; do
  grep -qE "^${f}_event: sha256:[0-9a-f]{64}$" "$I/I0001.yaml" || note "the CLI wrote no ${f}_event seal"
done
grep -q '"event":"plan_start","head":.*"issue":"I0001"' "$L" || note "no plan_start event for I0001"
grep -q '"event":"plan_done","head":.*"issue":"I0001"' "$L" || note "no plan_done event for I0001"
expect_exit 0 "$MJ" plan validate || fail=1

# ---------------------------------------------------------------- 2. a forged completion
cat >> "$I/I0002.yaml" <<'Y'
started_at: 2026-09-20T10:00:00Z
verified_at: 2026-09-20T10:00:01Z
completed_at: 2026-09-20T10:00:02Z
evidence:
  - covers: proof
    type: test
    command: "true"
    result: "written by hand"
Y
git add -A >/dev/null 2>&1; git commit -qm forged >/dev/null 2>&1
before_ledger="$(cksum < "$L")"
expect_exit 10 "$MJ" plan validate || fail=1
for pair in started_at:plan_start verified_at:plan_verify completed_at:plan_done; do
  printf '%s\n' "$LAST_OUT" | grep -q "FAIL stamp_without_event I0002 — ${pair%%:*} .* has no ${pair#*:} event" \
    || note "plan validate did not name I0002's ${pair%%:*} and its missing ${pair#*:} event"
done
[ "$(pj_status I0002)" = READY ] || note "a forged completion derived $(pj_status I0002), not READY"
[ "$(cksum < "$L")" = "$before_ledger" ] || note "validating appended to the ledger"

# the executable's derivation refuses it identically
tool majordomus_plan_validate '{}' > "$S/validate.json" || note "majordomus_plan_validate failed"
jq -e '[.findings[] | select(.level == "FAIL" and .code == "stamp_without_event" and .subject == "I0002")] | length == 3' \
  "$S/validate.json" >/dev/null || note "the executable did not report three unproven stamps on I0002"
tool majordomus_plan_issues '{}' | jq -e '.issues[] | select(.id == "I0002") | .status == "READY"' >/dev/null \
  || note "the executable derived progress from a forged stamp"

# ---------------------------------------------------------------- 3. a borrowed seal
# I0001's completed_event, copied onto I0002: it seals another issue's stamp, not this one.
seal="$(sed -n 's/^completed_event: //p' "$I/I0001.yaml")"
[ -n "$seal" ] || note "I0001 carries no completed_event to borrow"
awk -v s="completed_event: $seal" '/^evidence:/ && !d { print s; d = 1 } { print }' "$I/I0002.yaml" > "$S/i2"
cat "$S/i2" > "$I/I0002.yaml"
expect_exit 10 "$MJ" plan validate || fail=1
expect_grep 'stamp_without_event I0002 — completed_at .* has no plan_done event' || fail=1
rm -f "$I/I0002.yaml"; git add -A >/dev/null 2>&1; git commit -qm "the forgery goes" >/dev/null 2>&1

# ---------------------------------------------------------------- 4. the capability seals too
tool majordomus_plan_transition '{"issue":"I0003","transition":"start"}' > "$S/start.json" || note "capability start failed"
[ "$TOOL_ERR" = false ] || note "the capability refused to start READY I0003"
grep -qE '^started_event: sha256:[0-9a-f]{64}$' "$I/I0003.yaml" || note "the capability wrote a stamp without its seal"
grep -q '"event":"plan_start","head":.*"issue":"I0003"' "$L" || note "the capability wrote a stamp without its event"
# The seal is the digest of the event, the issue and the stamp — recomputed here, not read back.
at="$(sed -n 's/^started_at: //p' "$I/I0003.yaml")"
want="sha256:$(printf 'majordomus.plan-event/v1\nplan_start\nI0003\n%s\n' "$at" | { sha256sum 2>/dev/null || shasum -a 256; } | cut -d' ' -f1)"
[ "$(sed -n 's/^started_event: //p' "$I/I0003.yaml")" = "$want" ] || note "the capability's seal is not the digest of plan_start, I0003 and $at"
expect_exit 0 "$MJ" plan validate || fail=1

# ---------------------------------------------------------------- 5. healing, and the legacy list
printf 'started_at: 2026-09-20T09:00:00Z\n' >> "$I/I0004.yaml"
expect_exit 10 "$MJ" plan validate || fail=1
[ "$(pj_status I0004)" = READY ] || note "a forged start derived $(pj_status I0004), not READY"
expect_exit 0 "$MJ" plan start I0004 || fail=1
expect_exit 0 "$MJ" plan validate || fail=1

# An entry excuses the exact stamp it names, and only one from before seals existed.
pj_issue I0005 M000
printf 'started_at: 2026-09-01T00:00:00Z\nunsealed_stamps:\n  - started_at 2026-09-01T00:00:00Z\n' >> "$I/I0005.yaml"
expect_exit 0 "$MJ" plan validate || fail=1
[ "$(pj_status I0005)" = ACTIVE ] || note "a listed legacy stamp derived $(pj_status I0005), not ACTIVE"
sed 's/2026-09-01T00:00:00Z/2026-09-16T00:00:00Z/' "$I/I0005.yaml" > "$S/i5"
cat "$S/i5" > "$I/I0005.yaml"
expect_exit 10 "$MJ" plan validate || fail=1
expect_grep 'stamp_without_event I0005 — started_at 2026-09-16T00:00:00Z has no plan_start event' || fail=1
git add -A >/dev/null 2>&1; git commit -qm "listed stamps" >/dev/null 2>&1
tool majordomus_plan_validate '{}' | jq -e '[.findings[] | select(.code == "stamp_without_event")] | map(.subject) == ["I0005"]' >/dev/null \
  || note "the executable excused a stamp at SEALED_FROM, or refused a sealed one"

[ "$fail" = 0 ] || exit 1
echo "    a stamp is a fact only with its seal: the CLI and the capability seal and record it, a hand-written one is refused by name in both engines"
