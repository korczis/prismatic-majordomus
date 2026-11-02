# majordomus-timeout: 300
# majordomus-covers: none
# A session that waits on a person says so on the mesh (I3992).
#
# The owner's order of 2026-10-22: nothing may stall because a peer waits on a dialog. I3990
# suspends such a session's exclusive claims once its progress ttl passes; this case proves the
# other half, that peers see the wait the moment it starts and why. It serves a runtime whose
# progress ttl is 8 s, opens the provider session `claude-code-s1` as the start event does, and
# drives the repository's own hook (.claude/hooks/majordomus-session-wait) with the payloads
# Claude Code hands it:
#
#   1. a permission dialog (Notification) marks the session blocked, with the reason, at once:
#      `mesh peers` shows `blocked` and the phase, and the mark is a journal event every linked
#      runtime receives; the hook says nothing and exits 0;
#   2. the next tool call (PreToolUse) clears the mark at once; the end of a turn (Stop) marks
#      it `blocked: awaiting input`;
#   3. the mark is information: a claim of a session that only ever waited keeps the stream
#      rule and holds past the ttl, while a session that worked and then only waited, however
#      often it says so, is suspended by its ttl as before;
#   4. a payload that is no beat, and a hook with no server to ask, exit 0 and say nothing.
. "$ROOT/test/lib.sh"
command -v jq >/dev/null 2>&1 || skip "jq absent"
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
mkdir -p lib ci docs && echo a > lib/a && echo c > ci/c && echo d > docs/d
git add -A && git commit -qm base
mkdir -p "$T/state"
export XDG_STATE_HOME="$T/state"

# The repository's hook, as the provider runs it, beside an executable it can ask.
mkdir -p .claude/hooks bin
cp "$ROOT/.claude/hooks/majordomus-session-wait" .claude/hooks/
printf '#!/bin/sh\nexec "%s" "$@"\n' "$RB" > bin/majordomus-cli
chmod +x .claude/hooks/majordomus-session-wait bin/majordomus-cli

mkdir -p .ai/repo/mesh
printf 'schema: mesh/v1\nkind: mesh-declaration\nid: majordomus\nenabled: true\n' > .ai/repo/mesh/majordomus.yaml
printf 'multicast:\n  enabled: false\ntrust:\n  policy: tofu\n' >> .ai/repo/mesh/majordomus.yaml
printf 'cooperation:\n  heartbeat_seconds: 1\n  expiry_seconds: 30\n  session_ttl_seconds: 8\n  repository: case-3992\n' \
  >> .ai/repo/mesh/majordomus.yaml
git add -A && git commit -qm "mesh enabled"
( exec "$RB" serve --port 0 --idle 0 --discovery filesystem </dev/null >"$T/serve.log" 2>&1 ) &
SERVER=$!
PIDS="$PIDS $SERVER"
ready() { [ -n "$(jq -r '.url // empty' .ai/local/state/mcp/server.json 2>/dev/null)" ]; }
i=0; until ready; do i=$((i + 1)); [ "$i" -lt 200 ] || { cat "$T/serve.log"; exit 1; }; sleep 0.2; done

# The provider session, opened as the start event opens it, holding an exclusive claim.
expect_exit 0 "$RB" mesh session open --session claude-code-s1 --client claude-code
expect_exit 0 "$RB" mesh claim lib --session claude-code-s1
expect_grep '^written '

hook() {   # hook PAYLOAD: run the hook the way the provider does; it must exit 0 and say nothing
  printf '%s' "$1" | ./.claude/hooks/majordomus-session-wait >"$T/hook.out" 2>&1
  rc=$?
  [ "$rc" = 0 ] || { echo "    the hook exited $rc for $1"; cat "$T/hook.out"; exit 1; }
  [ ! -s "$T/hook.out" ] || { echo "    the hook said something for $1:"; cat "$T/hook.out"; exit 1; }
}
session() {   # session KEY-SUFFIX: the session's node in mesh peers
  "$RB" mesh peers --full --format json 2>/dev/null \
    | jq -c --arg s "/$1" '[.. | objects | select((.key? // "") | type == "string" and endswith($s))
        | select(has("claims_held"))][0]'
}
claims_over() {
  "$RB" mesh state --full --format json 2>/dev/null | jq -c --arg p "$1" '[.state.claims[] | select(.scope == [$p])
    | {session, state: .state.state, detail: (.state.detail // "")}]'
}
marks() {   # how many session beats name a wait on a person
  "$RB" mesh events --newest --limit 1000 --beats --format json 2>/dev/null \
    | jq '[.events[] | select(.body.kind == "session_beat" and ((.body.phase // "") | startswith("blocked:")))] | length'
}

# ---------------------------------------------------------------- 1. a permission dialog marks it at once
hook '{"session_id":"s1","hook_event_name":"Notification","notification_type":"permission_prompt","message":"Claude needs your permission to use Bash"}'
S="$(session claude-code-s1)"
jq -e '.blocked == "permission dialog (Claude needs your permission to use Bash)"
    and (.phase | startswith("blocked: permission dialog"))' <<<"$S" >/dev/null \
  || { echo "    mesh peers does not show the session blocked on the dialog: $S"; exit 1; }
[ "$(marks)" = 1 ] || { echo "    the wait is not a journal event a linked runtime receives ($(marks))"; exit 1; }
"$RB" mesh peers 2>/dev/null | grep -q 'blocked: permission dialog' \
  || { echo "    the text board does not say the session is blocked:"; "$RB" mesh peers; exit 1; }

# ---------------------------------------------------------------- 2. the next tool call clears it; a stop marks it again
hook '{"session_id":"s1","hook_event_name":"PreToolUse","tool_name":"Bash"}'
S="$(session claude-code-s1)"
jq -e '(has("blocked") | not) and (has("phase") | not)' <<<"$S" >/dev/null \
  || { echo "    the tool call did not clear the wait: $S"; exit 1; }
hook '{"session_id":"s1","hook_event_name":"Stop","stop_hook_active":false}'
S="$(session claude-code-s1)"
jq -e '.blocked == "awaiting input"' <<<"$S" >/dev/null \
  || { echo "    the end of a turn does not read as awaiting input: $S"; exit 1; }
[ "$(marks)" = 2 ] || { echo "    expected two waits in the journal, found $(marks)"; exit 1; }

# ---------------------------------------------------------------- 3. information only
# a session that only ever waited: its claim keeps the stream rule and holds past the ttl
expect_exit 0 "$RB" mesh session open --session claude-code-s2 --client claude-code
expect_exit 0 "$RB" mesh claim ci --session claude-code-s2
hook '{"session_id":"s2","hook_event_name":"Notification","notification_type":"idle_prompt","message":"Claude is waiting for your input"}'
# a session that worked once, then only waited, saying so again and again
expect_exit 0 "$RB" mesh session open --session claude-code-s3 --client claude-code
expect_exit 0 "$RB" mesh claim docs --session claude-code-s3
expect_exit 0 "$RB" mesh beat --session claude-code-s3 --phase "editing"
for n in 1 2 3 4 5 6; do
  if [ $((n % 2)) = 0 ]; then why="permission dialog"; else why="awaiting input"; fi
  expect_exit 0 "$RB" mesh beat --session claude-code-s3 --phase "blocked: $why"
  sleep 2
done
C="$(claims_over ci)"
jq -e '.[0].state == "held"' <<<"$C" >/dev/null \
  || { echo "    a wait put a session that never progressed on the progress rule: $C"; exit 1; }
D="$(claims_over docs)"
jq -e '.[0].state == "suspended" and (.[0].detail | test("stopped making progress"))' <<<"$D" >/dev/null \
  || { echo "    twelve seconds of waits kept a claim excluding past its 8 s ttl: $D"; exit 1; }
S="$(session claude-code-s3)"
jq -e '.blocked == "permission dialog" and (.progress_ms | type == "number")' <<<"$S" >/dev/null \
  || { echo "    the waiting session's reason or progress is not shown: $S"; exit 1; }

# ---------------------------------------------------------------- 4. never a failure
hook 'not json'
hook '{"session_id":"s1","hook_event_name":"SessionStart"}'
kill "$SERVER" 2>/dev/null; wait "$SERVER" 2>/dev/null || true
hook '{"session_id":"s1","hook_event_name":"Stop"}'
exit 0
