# majordomus-timeout: 300
# A session joins the mesh at start (I2734). The provider's start event opens the session on
# the mesh of the checkout's shared server under `<provider>-<provider session>`, with its
# branch and head, and with the task only while one is active; the briefing names that id so
# the worker can claim under it; a resume says it again; the end event closes it, so its
# claims end everywhere at once. Without a server the start event still opens the episode
# and says nothing about a mesh session.
. "$ROOT/test/lib.sh"
RB="$(rust_bin)" || rust_bin_exit $?
export MAJORDOMUS_SHARE="$ROOT/share" MAJORDOMUS_BIN="$RB"

PIDS=""
trap 'for p in $PIDS; do kill "$p" 2>/dev/null || true; done' EXIT

R="$T/repo"
mkdir -p "$R"
(
  cd "$R" || exit
  git init -q .
  git config user.email t@example.com
  git config user.name t
  "$MJ" init >/dev/null
  "$MJ" update >/dev/null
  mkdir -p .ai/repo/mesh
  printf 'schema: mesh/v1\nkind: mesh-declaration\nid: majordomus\nenabled: true\n' > .ai/repo/mesh/majordomus.yaml
  printf 'multicast:\n  enabled: false\ntrust:\n  policy: tofu\n' >> .ai/repo/mesh/majordomus.yaml
  printf 'cooperation:\n  heartbeat_seconds: 1\n  expiry_seconds: 30\n  repository: case-3066\n' >> .ai/repo/mesh/majordomus.yaml
  git add -A && git commit -qm base
)
mkdir -p "$T/state" "$T/bin"
printf '#!/bin/sh\necho "gh: not here in this case" >&2\nexit 4\n' > "$T/bin/gh"
chmod +x "$T/bin/gh"
export PATH="$T/bin:$PATH" XDG_STATE_HOME="$T/state"
( cd "$R" && exec "$RB" serve --port 0 --idle 0 --discovery filesystem </dev/null >"$T/serve.log" 2>&1 ) &
PIDS="$PIDS $!"
ready() { [ -n "$(jq -r '.url // empty' "$R/.ai/local/state/mcp/server.json" 2>/dev/null)" ]; }
i=0; until ready; do i=$((i + 1)); [ "$i" -lt 200 ] || { cat "$T/serve.log"; exit 1; }; sleep 0.2; done
cd "$R" || exit

# The session `id` as the mesh's full state holds it, as one compact JSON object.
session() {
  "$RB" mesh state --full --format json 2>/dev/null \
    | jq -c --arg id "$1" '[.. | objects | select((.key? // "") | type == "string" and endswith("/" + $id))][0] // empty'
}
# The case runs under `bash -eu`: a capture that fails would end it without a word (master
# c1a430dcfe, 2026-10-16), so a failing one says which event it was and what it printed.
start() {
  printf '{"session_id":"%s","source":"%s"}' "$1" "$2" \
    | "$MJ" capture session --provider claude-code --event start 2>"$T/err" \
    || { echo "    the start event ($2) of $1 exited $?" >&2; cat "$T/err" >&2; return 1; }
}

# ---------------------------------------------------------------- 1. the start event joins
BRIEF="$(start cc-3066 startup)"
grep -q '^Mesh: active' <<<"$BRIEF" || { echo "    the briefing has no active mesh line"; echo "$BRIEF" | head -20; cat "$T/err"; exit 1; }
grep -q '^Mesh session: claude-code-cc-3066 — ' <<<"$BRIEF" \
  || { echo "    the briefing does not name the session's mesh id"; echo "$BRIEF" | head -20; exit 1; }
S="$(session claude-code-cc-3066)"
[ -n "$S" ] || { echo "    the start event opened no session on the mesh"; "$RB" mesh state --full --format json | head -60; exit 1; }
jq -e --arg head "$(git rev-parse HEAD)" --arg branch "$(git symbolic-ref --short HEAD)" \
  '.info.client == "claude-code" and .info.branch == $branch and .info.head == $head' <<<"$S" >/dev/null \
  || { echo "    the mesh session does not carry its client, branch and head: $S"; exit 1; }
jq -e '.info.task == null' <<<"$S" >/dev/null || { echo "    a session with no active task names one: $S"; exit 1; }

# ---------------------------------------------------------------- 2. a resume says it again, with the active task
"$MJ" start "case 3066 work" --scope lib >"$T/out" 2>&1 || { echo "    the task did not start"; cat "$T/out"; exit 1; }
TASK="$(sed -n 's/^id: *//p' .ai/local/state/current.yaml 2>/dev/null | head -n 1)"
[ -n "$TASK" ] || TASK="$("$MJ" status --json 2>/dev/null | jq -r '.task.id // empty')"
start cc-3066 resume >/dev/null
S="$(session claude-code-cc-3066)"
jq -e --arg t "$TASK" '.info.task == $t and .info.intent == "case 3066 work"' <<<"$S" >/dev/null \
  || { echo "    the resumed session does not carry the active task $TASK: $S"; exit 1; }

# ---------------------------------------------------------------- 3. the end event leaves
printf '{"session_id":"cc-3066","reason":"prompt_input_exit"}' \
  | "$MJ" capture session --provider claude-code --event end >/dev/null 2>"$T/err" \
  || { echo "    the end event exited $?"; cat "$T/err"; exit 1; }
S="$(session claude-code-cc-3066)"
jq -e '.state == "closed"' <<<"$S" >/dev/null \
  || { echo "    the end event did not close the mesh session: $S"; exit 1; }

# ---------------------------------------------------------------- 4. no server, no mesh session, still an episode
kill $PIDS 2>/dev/null || { echo "    the server had exited before the case stopped it"; cat "$T/serve.log"; exit 1; }
wait 2>/dev/null || true
PIDS=""
rm -f .ai/local/state/mcp/server.json
BRIEF="$(MAJORDOMUS_BIN=/nonexistent start cc-3067 startup)"
grep -q '^Mesh session:' <<<"$BRIEF" && { echo "    a start with no server named a mesh session"; exit 1; }
grep -q 'cc-3067' "$T/err" || grep -rq 'cc-3067' .ai/local/state 2>/dev/null \
  || { echo "    the start event without a server opened no episode"; cat "$T/err"; exit 1; }
exit 0
