# A run in flight is judged by a capability before anybody cancels it
# (project.an-expedited-landing-is-the-owners-word, clause 7; ADR 0155).
#
# Until this case the judgement was a session reading `gh run list`: one misreading cancels
# the only verdict a commit on the trunk will ever get, or a deployment in flight.
# `monitor.stale-runs` is that judgement as a read. It cancels nothing.
#
# What is held here, through the built executable and its MCP tool:
#
#   1  a recorded listing is judged: a run of a merged pull request, a run of a head that
#      moved and a second run of one commit on the trunk are stale, each with the command
#      that cancels exactly that run;
#   2  what the rule never cancels is kept and carries no command: a release run, a
#      deployment, the trunk's only run of a commit, a run somebody dispatched;
#   3  the forge is asked through `gh` when no listing is given: a branch with no pull
#      request has its head asked for, a branch the forge says is gone is stale, and a
#      branch whose head cannot be read is kept;
#   4  a forge that cannot be asked is a state with its reason and no run, never "nothing
#      is stale";
#   5  the capability is a read: it declares no write, and the fixture `gh` is never asked
#      to cancel.
#
# Skips itself when there is neither cargo nor MAJORDOMUS_BIN, as every Rust case does.
. "$ROOT/test/lib.sh"
command -v jq >/dev/null 2>&1 || skip "no jq"
RB="$(rust_bin)" || rust_bin_exit $?
[ -x "$RB" ] || { echo "    the build produced no executable at $RB"; exit 1; }
MAJORDOMUS_SHARE="$ROOT/share"; export MAJORDOMUS_SHARE
S="$(mktemp -d "${TMPDIR:-/tmp}/mj3005.XXXXXX")"; trap 'rm -rf "$S"' EXIT
MJ="$ROOT/bin/majordomus"

# One MCP tool call over the real pipes, as case 2978 makes it.
tool() { # <arguments-json> -> structuredContent on stdout, isError in $TOOL_ERR
  local args="${1:-{\}}" out="$S/frames"
  {
    printf '{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2025-06-18","capabilities":{},"clientInfo":{"name":"case3005","version":"0"}}}\n'
    printf '{"jsonrpc":"2.0","method":"notifications/initialized"}\n'
    printf '{"jsonrpc":"2.0","id":2,"method":"tools/call","params":{"name":"majordomus_monitor_stale_runs","arguments":%s}}\n' "$args"
  } | "$RB" mcp 2>"$S/mcp.err" > "$out" || {
    printf '    the server failed:\n' >&2; sed 's/^/    | /' "$S/mcp.err" >&2; return 1; }
  TOOL_ERR="$(tail -n 1 "$out" | jq -r '.result.isError')"
  tail -n 1 "$out" | jq '.result.structuredContent'
}

fail=0
note() { printf '    %s\n' "$1"; fail=1; }

"$MJ" init >/dev/null 2>&1
mkdir -p test/fixtures/stale-runs
cp "$ROOT/test/fixtures/stale-runs/listing.json" test/fixtures/stale-runs/listing.json

# The fixture gh: it answers the three questions the capability asks and records every
# call, so that a cancel would be seen.
mkdir -p "$S/bin"
cat > "$S/bin/gh" <<'GH'
#!/bin/sh
printf '%s\n' "$*" >> "$GH_CALLS"
case "$1 $2" in
  "run list") cat "$GH_FIXTURE/runs.json" ;;
  "pr list") cat "$GH_FIXTURE/prs.json" ;;
  "api repos/{owner}/{repo}/branches/fix/bare") echo '{"commit":{"sha":"cccc000000"}}' ;;
  "api repos/{owner}/{repo}/branches/fix/gone") echo "gh: Branch not found (HTTP 404)" >&2; exit 1 ;;
  *) echo "gh: the forge is unreachable" >&2; exit 1 ;;
esac
GH
chmod +x "$S/bin/gh"
GH_CALLS="$S/calls"; GH_FIXTURE="$S"; export GH_CALLS GH_FIXTURE
: > "$GH_CALLS"

# ---------------------------------------------------------------- 1, 2. a recorded listing
tool '{"snapshot":"test/fixtures/stale-runs/listing.json"}' > "$S/rec.json" || note "the tool failed on a recorded listing"
[ "$TOOL_ERR" = false ] || note "the tool refused a recorded listing"
[ "$(jq -r '"\(.state) \(.stale) \(.kept)"' < "$S/rec.json")" = "ok 3 4" ] \
  || note "the recorded listing is judged $(jq -c '[.state,.stale,.kept]' < "$S/rec.json"), not ok 3 4"
got="$(jq -r '[.runs[]|select(.standing=="stale")|"\(.id):\(.reason)"]|join(" ")' < "$S/rec.json")"
[ "$got" = "38392595807:duplicate 38394992358:pull_request_merged 38390000004:head_moved" ] \
  || note "the stale runs are: $got"
[ "$(jq '[.runs[]|select(.standing=="stale" and .cancel != "gh run cancel \(.id)")]|length' < "$S/rec.json")" = 0 ] \
  || note "a stale run does not carry the command that cancels exactly it"
got="$(jq -r '[.runs[]|select(.standing=="kept")|.reason]|sort|join(" ")' < "$S/rec.json")"
[ "$got" = "asked_for deployment release trunk_only_run" ] || note "the kept runs are kept for: $got"
[ "$(jq '[.runs[]|select(.standing=="kept" and has("cancel"))]|length' < "$S/rec.json")" = 0 ] \
  || note "a kept run carries a command"
[ "$(jq '[.runs[]|select(.status=="completed")]|length' < "$S/rec.json")" = 0 ] \
  || note "a completed run is in the answer"
# stale first
[ "$(jq -r '[.runs[].standing]|join(" ")' < "$S/rec.json")" = "stale stale stale kept kept kept kept" ] \
  || note "the stale runs do not come first"
tool '{"snapshot":"test/fixtures/stale-runs/listing.json","branch":"master"}' > "$S/trunk.json"
[ "$(jq -r '"\(.stale) \(.kept) \([.runs[].branch]|unique|join(","))"' < "$S/trunk.json")" = "1 3 master" ] \
  || note "one branch asked for answers $(jq -c '[.stale,.kept]' < "$S/trunk.json")"
[ ! -s "$GH_CALLS" ] || note "a recorded listing still asked the forge: $(tr '\n' ';' < "$GH_CALLS")"

# ---------------------------------------------------------------- 3. the forge, through gh
cat > "$S/runs.json" <<'J'
[{"databaseId":1,"workflowName":"validate","status":"in_progress","event":"push","headBranch":"fix/bare","headSha":"cccc000000","createdAt":"2026-10-13T10:00:00Z"},
 {"databaseId":2,"workflowName":"validate","status":"queued","event":"push","headBranch":"fix/gone","headSha":"dddd000000","createdAt":"2026-10-13T10:01:00Z"},
 {"databaseId":3,"workflowName":"validate","status":"queued","event":"push","headBranch":"fix/unread","headSha":"eeee000000","createdAt":"2026-10-13T10:02:00Z"},
 {"databaseId":4,"workflowName":"validate","status":"queued","event":"pull_request","headBranch":"fix/open","headSha":"ffff000000","createdAt":"2026-10-13T10:03:00Z"}]
J
echo '[{"number":7,"state":"OPEN","headRefName":"fix/open","headRefOid":"ffff000000"}]' > "$S/prs.json"
PATH="$S/bin:$PATH" tool '{}' > "$S/live.json" || note "the tool failed asking the forge"
got="$(jq -r '[.runs[]|"\(.id):\(.reason)"]|sort|join(" ")' < "$S/live.json")"
[ "$got" = "1:current_head 2:branch_gone 3:head_unknown 4:current_head" ] || note "asked through gh, the runs are: $got"
[ "$(jq -r '"\(.state) \(.stale) \(.kept)"' < "$S/live.json")" = "ok 1 3" ] \
  || note "asked through gh, the answer is $(jq -c '[.state,.stale,.kept]' < "$S/live.json")"
grep -q 'branches/fix/open' "$GH_CALLS" && note "a branch its pull request decides had its head asked for"
grep -q 'branches/fix/unread' "$GH_CALLS" || note "a branch with no pull request did not have its head asked for"

# ---------------------------------------------------------------- 4. a forge that cannot be asked
rm "$S/runs.json"
PATH="$S/bin:$PATH" tool '{}' > "$S/down.json" || note "the tool failed on an unreachable forge"
[ "$(jq -r '"\(.state) \(.runs|length)"' < "$S/down.json")" = "error 0" ] \
  || note "an unreachable forge answers $(jq -c '[.state,(.runs|length)]' < "$S/down.json")"
[ -n "$(jq -r '.reason // ""' < "$S/down.json")" ] || note "an unreachable forge gives no reason"
tool '{"snapshot":"../outside.json"}' > "$S/outside.json"
[ "$(jq -r .state < "$S/outside.json")" = error ] || note "a listing outside the repository is read"
tool '{"snapshot":"test/fixtures/stale-runs/none.json"}' > "$S/none.json"
[ "$(jq -r .state < "$S/none.json")" = absent ] || note "a listing that is not there answers $(jq -r .state < "$S/none.json")"

# ---------------------------------------------------------------- 5. a read that cancels nothing
grep -q 'cancel' "$GH_CALLS" && note "the capability asked gh to cancel: $(grep cancel "$GH_CALLS" | tr '\n' ';')"
effect="$("$RB" capabilities describe monitor.stale-runs --format json 2>/dev/null | jq -r '.execution.effect // .capability.execution.effect // empty')"
case "$effect" in read|read_only|Read) ;; *) note "monitor.stale-runs declares the effect '$effect', not a read" ;; esac

[ "$fail" = 0 ] || exit 1
echo "    a run in flight is judged before it is cancelled: three stale with their commands, four kept without one, a gone branch stale, an unread one kept, an unreachable forge a state, and nothing cancelled"
