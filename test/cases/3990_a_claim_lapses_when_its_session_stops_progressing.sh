# majordomus-timeout: 300
# majordomus-covers: none
# An exclusive mesh claim lives by its holder's progress, not only by its runtime's beat (I3990).
#
# On 2026-10-22 the executor of seat 5d held the integration lease — an exclusive mesh claim —
# for hours and never answered: its server beat on while the worker behind it waited on
# nobody, and the stdio bridge's 20 s pings looked like a live client. The owner ordered that
# no work may stall because a peer waits on a dialog, crashed or hung. This case serves a
# runtime whose progress ttl is 8 s, opens an MCP session over plain HTTP — no special case for
# it — and proves:
#
#   1. an exclusive claim taken over MCP holds while the session keeps making tool calls, and
#      a command-line claim kept by `mesh beat --phase "waiting on CI"` holds with it, its
#      phase shown by `mesh peers`; over MCP a client beats for itself and is refused
#      (`not_own`) when it names another session;
#   2. when the session only pings, its exclusive claim reads `suspended` (refusing nobody)
#      while the runtime's own stream stays alive; its advisory claim and its session are
#      untouched, nothing was closed, and the beating claim still holds;
#   3. a command-line claim whose session never beats still holds — the old rule;
#   4. a second exclusive claimer over the suspended scope is admitted;
#   5. the beating claim is suspended once its process stops beating.
. "$ROOT/test/lib.sh"
command -v jq >/dev/null 2>&1 || skip "jq absent"
command -v curl >/dev/null 2>&1 || skip "curl not installed"
RB="$(rust_bin)" || rust_bin_exit $?
export MAJORDOMUS_SHARE="$ROOT/share" MAJORDOMUS_BIN="$RB"

PIDS=""
trap 'for p in $PIDS; do kill "$p" 2>/dev/null || true; done' EXIT

R="$T/repo"
mkdir -p "$R"
cd "$R" || exit 1
git init -q .
git config user.email t@example.com
git config user.name t
"$MJ" init >/dev/null
"$MJ" update >/dev/null
mkdir -p lib docs ci tools && echo a > lib/a && echo d > docs/d && echo c > ci/c && echo t > tools/t
git add -A && git commit -qm base
mkdir -p "$T/state"
export XDG_STATE_HOME="$T/state"

# A runtime of its own: one heartbeat a second, a long stream expiry, and a progress ttl of
# 8 s (four heartbeats is the least the declaration admits).
mkdir -p .ai/repo/mesh
printf 'schema: mesh/v1\nkind: mesh-declaration\nid: majordomus\nenabled: true\n' > .ai/repo/mesh/majordomus.yaml
printf 'multicast:\n  enabled: false\ntailscale:\n  enabled: false\ntrust:\n  policy: tofu\n' >> .ai/repo/mesh/majordomus.yaml
printf 'cooperation:\n  heartbeat_seconds: 1\n  expiry_seconds: 30\n  session_ttl_seconds: 8\n  repository: case-3990\n' \
  >> .ai/repo/mesh/majordomus.yaml
git add -A && git commit -qm "mesh enabled"
( exec "$RB" serve --port 0 --idle 0 --discovery filesystem </dev/null >"$T/serve.log" 2>&1 ) &
PIDS="$PIDS $!"
ready() { [ -n "$(jq -r '.url // empty' .ai/local/state/mcp/server.json 2>/dev/null)" ]; }
i=0; until ready; do i=$((i + 1)); [ "$i" -lt 200 ] || { cat "$T/serve.log"; exit 1; }; sleep 0.2; done
url="$(jq -r '.url' .ai/local/state/mcp/server.json)"

mcp() {   # mcp ID METHOD PARAMS [CURL-ARGS...]
  local id="$1" method="$2" params="$3"; shift 3
  curl -fsS --connect-timeout 5 --max-time 30 -X POST "$url/mcp" \
    -H 'content-type: application/json' -H 'accept: application/json, text/event-stream' \
    "$@" -d "{\"jsonrpc\":\"2.0\",\"id\":$id,\"method\":\"$method\",\"params\":$params}"
}
mcp 1 initialize '{"protocolVersion":"2025-06-18","capabilities":{},"clientInfo":{"name":"case3990","version":"0"}}' \
  -D "$T/mcp.headers" > "$T/mcp.init" || { echo "    MCP initialize failed"; cat "$T/mcp.init"; exit 1; }
sid="$(tr -d '\r' < "$T/mcp.headers" | sed -n 's/^[Mm]cp-[Ss]ession-[Ii]d: //p' | head -n 1)"
[ -n "$sid" ] || { echo "    the MCP endpoint opened no session:"; cat "$T/mcp.headers"; exit 1; }
mcp 2 notifications/initialized '{}' -H "mcp-session-id: $sid" >/dev/null 2>&1 || true

state() { "$RB" mesh state --full --format json 2>/dev/null; }
# The claims over one path, as compact objects: key, session, mode, standing and its detail.
claims_over() {
  state | jq -c --arg p "$1" '[.state.claims[] | select(.scope == [$p])
    | {key, session, mode, state: .state.state, detail: (.state.detail // "")}]'
}
beat_waiter() { "$RB" mesh beat --session waiter --phase "waiting on CI" >/dev/null 2>&1 \
  || { echo "    mesh beat failed:"; "$RB" mesh beat --session waiter --phase "waiting on CI"; exit 1; }; }

# ---------------------------------------------------------------- 1. claimed, and held while working
mcp 3 tools/call '{"name":"majordomus_mesh_claim","arguments":{"scope":["lib"],"intent":"hold the lease","mode":"exclusive"}}' \
  -H "mcp-session-id: $sid" > "$T/claim.out" || { echo "    the MCP claim failed"; cat "$T/claim.out"; exit 1; }
grep -q '"isError":true' "$T/claim.out" && { echo "    the MCP claim was refused:"; cat "$T/claim.out"; exit 1; }
mcp 4 tools/call '{"name":"majordomus_announce","arguments":{"intent":"document the lease","scope":["docs"]}}' \
  -H "mcp-session-id: $sid" > "$T/announce.out" || { echo "    the announcement failed"; cat "$T/announce.out"; exit 1; }
C="$(claims_over lib)"
jq -e 'length == 1 and .[0].state == "held" and (.[0].session | test("/board-p[0-9]+$"))' <<<"$C" >/dev/null \
  || { echo "    the board session does not hold lib: $C"; state | head -60; exit 1; }
session="$(jq -r '.[0].session' <<<"$C")"

# a client beats for itself over MCP, and never for another session
mcp 5 tools/call '{"name":"majordomus_mesh_beat","arguments":{"phase":"reading the lease"}}' \
  -H "mcp-session-id: $sid" > "$T/beat.out" || { echo "    the MCP beat failed"; cat "$T/beat.out"; exit 1; }
grep -q '"isError":true' "$T/beat.out" && { echo "    the MCP beat was refused:"; cat "$T/beat.out"; exit 1; }
mcp 6 tools/call '{"name":"majordomus_mesh_beat","arguments":{"session":"legacy","phase":"not mine"}}' \
  -H "mcp-session-id: $sid" > "$T/beat-other.out" || true
grep -q 'not_own' "$T/beat-other.out" \
  || { echo "    an MCP client beat for another session:"; cat "$T/beat-other.out"; exit 1; }

# a command-line claim whose session never beats, and one a long-running process keeps
expect_exit 0 "$RB" mesh claim tools --session legacy
expect_grep '^written '
expect_exit 0 "$RB" mesh claim ci --session waiter
expect_grep '^written '
beat_waiter

n=10
for _ in 1 2 3 4 5 6; do
  n=$((n + 1))
  mcp "$n" tools/call '{"name":"majordomus_mesh_cooperation","arguments":{}}' -H "mcp-session-id: $sid" >/dev/null \
    || { echo "    a tool call failed"; exit 1; }
  beat_waiter
  sleep 2
done
C="$(claims_over lib)"
jq -e '.[0].state == "held"' <<<"$C" >/dev/null \
  || { echo "    a working session lost its claim after 12 s of tool calls (ttl 8 s): $C"; exit 1; }
beats="$("$RB" mesh events --newest --limit 1000 --beats --format json 2>/dev/null \
  | jq '[.events[] | select(.body.kind == "session_beat")] | length')"
[ "${beats:-0}" -ge 3 ] || { echo "    working sessions wrote ${beats:-0} session beat(s), expected several"; exit 1; }
hidden="$("$RB" mesh events --newest --limit 1000 --format json 2>/dev/null \
  | jq '[.events[] | select(.body.kind == "session_beat")] | length')"
[ "$hidden" = 0 ] || { echo "    mesh events shows $hidden session beat(s) without --beats"; exit 1; }
"$RB" mesh peers --full --format json 2>/dev/null \
  | jq -e '[.. | objects | select((.key? // "") | type == "string" and endswith("/waiter"))
      | select(.phase == "waiting on CI" and (.progress_ms | type == "number"))] | length == 1' >/dev/null \
  || { echo "    mesh peers does not show the waiting session's phase"; "$RB" mesh peers --full --format json | head -80; exit 1; }

# ---------------------------------------------------------------- 2. only pings: the exclusive claim is suspended
closed_before="$("$RB" mesh events --newest --limit 1000 --format json 2>/dev/null \
  | jq '[.events[] | select(.body.kind == "session_closed")] | length')"
for _ in 1 2 3 4 5 6 7 8; do
  n=$((n + 1))
  mcp "$n" ping '{}' -H "mcp-session-id: $sid" >/dev/null || { echo "    a ping failed"; exit 1; }
  beat_waiter
  sleep 2
done
C="$(claims_over lib)"
jq -e '.[0].state == "suspended" and (.[0].detail | test("stopped making progress"))' <<<"$C" >/dev/null \
  || { echo "    a session that only pinged for 16 s does not read suspended (ttl 8 s): $C"; state | head -80; exit 1; }
A="$(claims_over docs)"
jq -e 'length == 1 and .[0].mode == "advisory" and .[0].state == "held"' <<<"$A" >/dev/null \
  || { echo "    the silent session's advisory claim did not stay held: $A"; exit 1; }
S="$(state | jq -c --arg k "$session" '[.state.sessions[] | select(.key == $k)][0]')"
jq -e '.state == "active"' <<<"$S" >/dev/null \
  || { echo "    the silent session was closed; an idle session is healthy: $S"; exit 1; }
closed_after="$("$RB" mesh events --newest --limit 1000 --format json 2>/dev/null \
  | jq '[.events[] | select(.body.kind == "session_closed")] | length')"
[ "$closed_after" = "$closed_before" ] || { echo "    a suspension wrote a session close"; exit 1; }
state | jq -e '[.streams[] | select(.liveness == "own")] | length == 1' >/dev/null \
  || { echo "    the runtime's own stream is not alive: the claim was suspended for the wrong reason"; exit 1; }
W="$(claims_over ci)"
jq -e '.[0].state == "held"' <<<"$W" >/dev/null \
  || { echo "    a claim kept by mesh beat --phase \"waiting on CI\" did not hold: $W"; exit 1; }

# ---------------------------------------------------------------- 3. a session that never beat keeps the old rule
L="$(claims_over tools)"
jq -e '.[0].state == "held"' <<<"$L" >/dev/null \
  || { echo "    a command-line claim that never beat lost its standing: $L"; exit 1; }

# ---------------------------------------------------------------- 4. the suspended scope admits the next claimant
expect_exit 0 "$RB" mesh claim lib --session next
expect_grep '^written '
C="$(claims_over lib)"
jq -e '[.[] | select(.state == "held" and (.session | endswith("/next")))] | length == 1' <<<"$C" >/dev/null \
  || { echo "    the next claimant does not hold lib: $C"; exit 1; }

# ---------------------------------------------------------------- 5. a beating process that stops is suspended too
sleep 10
W="$(claims_over ci)"
jq -e '.[0].state == "suspended"' <<<"$W" >/dev/null \
  || { echo "    a session that beat and then stopped still excludes: $W"; exit 1; }
exit 0
