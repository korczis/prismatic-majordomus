# majordomus-timeout: 300
# An edit no claim covers is refused (I2735). With session.edits_require_claim on, the guard
# refusing (session.edit_guard_mode: refuse, I3322) and the mesh enabled, `majordomus guard
# hook` — what the provider's pre-tool hook runs — exits 2 for a write no claim covers, naming
# the path and the remedy; a live mesh claim of the writing
# session, or the active task's scope, lets it through, and another session's claim does not.
# Local state, a path outside the checkout, the policy switched off and a server that does not
# answer are let through. The checkout judged is the one the file belongs to, wherever the hook
# runs, and the tracked shim passes on the refusal and nothing else.
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
sed -i.bak 's/^  edits_require_claim: false/  edits_require_claim: true/' .ai/repo/policy.yaml && rm -f .ai/repo/policy.yaml.bak
grep -q '^  edits_require_claim: true' .ai/repo/policy.yaml || { echo "    the fixture's policy did not turn the guard on"; exit 1; }
# refusing: the skeleton declares the guard advisory (I3322), which reports an unclaimed edit and
# lets it through; what advisory does is case 4522's, and this case proves the refusal
sed -i.bak 's/^  edit_guard_mode: advisory/  edit_guard_mode: refuse/' .ai/repo/policy.yaml && rm -f .ai/repo/policy.yaml.bak
grep -q '^  edit_guard_mode: refuse' .ai/repo/policy.yaml || { echo "    the fixture's policy did not make the guard refuse"; exit 1; }
mkdir -p .ai/repo/mesh lib docs
printf 'schema: mesh/v1\nkind: mesh-declaration\nid: majordomus\nenabled: true\n' > .ai/repo/mesh/majordomus.yaml
printf 'multicast:\n  enabled: false\ntailscale:\n  enabled: false\ntrust:\n  policy: tofu\n' >> .ai/repo/mesh/majordomus.yaml
printf 'cooperation:\n  heartbeat_seconds: 1\n  expiry_seconds: 30\n  repository: case-3069\n' >> .ai/repo/mesh/majordomus.yaml
echo a > lib/a && echo d > docs/d && git add -A && git commit -qm base
mkdir -p "$T/state" "$T/bin" "$T/elsewhere"
printf '#!/bin/sh\necho "gh: not here in this case" >&2\nexit 4\n' > "$T/bin/gh"
chmod +x "$T/bin/gh"
export PATH="$T/bin:$PATH" XDG_STATE_HOME="$T/state"
( exec "$RB" serve --port 0 --idle 0 --discovery filesystem </dev/null >"$T/serve.log" 2>&1 ) &
PIDS="$PIDS $!"
ready() { [ -n "$(jq -r '.url // empty' .ai/local/state/mcp/server.json 2>/dev/null)" ]; }
i=0; until ready; do i=$((i + 1)); [ "$i" -lt 200 ] || { cat "$T/serve.log"; exit 1; }; sleep 0.2; done

# guard <session> <path> -> the exit code; stderr in $T/err. Run from elsewhere on purpose:
# the checkout judged is the one the file belongs to, not the hook's working directory.
guard() {
  printf '{"session_id":"%s","tool_name":"Edit","tool_input":{"file_path":"%s"}}' "$1" "$2" \
    | (cd "$T/elsewhere" && "$RB" guard hook --provider claude-code) 2>"$T/err"
}
expect_code() { # <want> <session> <path> <what>
  local got=0; guard "$2" "$3" || got=$?
  [ "$got" = "$1" ] || { echo "    $4: exit $got, wanted $1"; cat "$T/err"; exit 1; }
}

# ---------------------------------------------------------------- 1. nothing claims it: refused, with the remedy
expect_code 2 cc1 "$R/lib/a" "an unclaimed edit"
grep -q 'no claim covers lib/a' "$T/err" || { echo "    the refusal does not name the path"; cat "$T/err"; exit 1; }
grep -q 'majordomus start' "$T/err" && grep -q -- '--session claude-code-cc1' "$T/err" \
  || { echo "    the refusal does not name the remedy"; cat "$T/err"; exit 1; }
expect_code 2 cc1 "$R/lib/new-file.sh" "a new file nobody claimed"

# ---------------------------------------------------------------- 2. the session's own live claim covers it, another session's does not
expect_exit 0 "$RB" mesh claim lib --session claude-code-cc1 --advisory --intent "case 3069"
expect_code 0 cc1 "$R/lib/a" "an edit the session's claim covers"
expect_code 2 cc2 "$R/lib/a" "an edit only another session's claim covers"

# ---------------------------------------------------------------- 3. the active task's scope covers it, for any session
expect_exit 0 "$MJ" start "Docs work" --scope docs
expect_code 0 cc2 "$R/docs/d" "an edit the active task's scope covers"
expect_code 2 cc2 "$R/README.md" "an edit outside the task's scope"

# ---------------------------------------------------------------- 4. what is never refused
expect_code 0 cc2 "$R/.ai/local/state/x.yaml" "the checkout's local state"
expect_code 0 cc2 "$T/elsewhere/x.txt" "a path outside the checkout"
printf '{"session_id":"cc2","tool_name":"Bash","tool_input":{"command":"ls"}}' \
  | "$RB" guard hook --provider claude-code || { echo "    a tool that writes no file was refused"; exit 1; }

# ---------------------------------------------------------------- 5. the tracked shim passes on the refusal, and only that
SHIM="$ROOT/.claude/hooks/majordomus-edit-guard"
[ -x "$SHIM" ] || { echo "    $SHIM is not an executable shim"; exit 1; }
rc=0; printf '{"session_id":"cc2","tool_input":{"file_path":"%s"}}' "$R/README.md" | "$SHIM" 2>"$T/err" || rc=$?
[ "$rc" = 2 ] || { echo "    the shim did not pass on the refusal: exit $rc"; cat "$T/err"; exit 1; }
rc=0; printf '{"session_id":"cc2","tool_input":{"file_path":"%s"}}' "$R/docs/d" | "$SHIM" 2>"$T/err" || rc=$?
[ "$rc" = 0 ] || { echo "    the shim refused an allowed edit: exit $rc"; cat "$T/err"; exit 1; }

# ---------------------------------------------------------------- 6. no server: undecided, let through
kill $PIDS 2>/dev/null; wait 2>/dev/null; PIDS=""
rm -f .ai/local/state/mcp/server.json
expect_code 0 cc2 "$R/README.md" "an edit when no server answers"

# ---------------------------------------------------------------- 7. the switch off: nothing is judged
sed -i.bak 's/^  edits_require_claim: true/  edits_require_claim: false/' .ai/repo/policy.yaml && rm -f .ai/repo/policy.yaml.bak
expect_code 0 cc2 "$R/README.md" "an edit with the switch off"
exit 0
