# majordomus-timeout: 300
# An unclaimed edit is reported, recorded and let through by the command the generated
# .claude/settings.json names, and doctor names the session that made it (I3322).
#
# Case 4521 reads the generated configuration and case 3069 drives the judgement with the guard
# refusing. This one drives what Claude Code drives: the PreToolUse entry `capture install`
# wrote, its command resolved the way the provider resolves it (the project-directory
# placeholder substituted textually), a payload of the provider's shape on stdin, against a
# repository on the mesh whose policy asks every edit for a claim with the guard advisory, as
# this repository's policy declares it. Held here:
#
#   1  an unclaimed Edit exits 0, the stderr names the path, the remedy and the advisory mode,
#      and one line — time, session, path — lands in .ai/local/state/guard/unclaimed.log;
#   2  an edit the session's live claim covers exits 0 and records nothing;
#   3  a tool that names no file (Bash) is not judged and records nothing;
#   4  doctor's claim-coverage report names the session that edited unclaimed;
#   5  with MJ_CLAUDE_E2E=1 and a signed-in `claude` on PATH, Claude Code itself writes a file
#      through the configuration and the guard records that edit, under the client's own
#      session id. That run needs a client and is not part of CI, as in case 2980.
#
# Skips itself when there is neither cargo nor MAJORDOMUS_BIN, as every Rust case does.
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
sed -i.bak -e 's/^  edits_require_claim: false/  edits_require_claim: true/' \
  -e 's/^  ensure_server_on_start: true /  ensure_server_on_start: false /' .ai/repo/policy.yaml && rm -f .ai/repo/policy.yaml.bak
grep -q '^  edits_require_claim: true' .ai/repo/policy.yaml || { echo "    the fixture's policy did not turn the guard on"; exit 1; }
grep -qE '^  edit_guard_mode: advisory( |$)' .ai/repo/policy.yaml || { echo "    the skeleton does not declare the guard advisory"; exit 1; }
mkdir -p .ai/repo/mesh lib docs
printf 'schema: mesh/v1\nkind: mesh-declaration\nid: majordomus\nenabled: true\n' > .ai/repo/mesh/majordomus.yaml
printf 'multicast:\n  enabled: false\ntailscale:\n  enabled: false\ntrust:\n  policy: tofu\n' >> .ai/repo/mesh/majordomus.yaml
printf 'cooperation:\n  heartbeat_seconds: 1\n  expiry_seconds: 30\n  repository: case-4522\n' >> .ai/repo/mesh/majordomus.yaml
# The guard's shim, which this checkout carries as this repository does, and the launcher it
# asks, stood in by this tree's executable: the shim never builds and never resolves anything
# but $root/bin/majordomus-cli.
mkdir -p .claude/hooks bin
cp "$ROOT/.claude/hooks/majordomus-edit-guard" .claude/hooks/ && chmod +x .claude/hooks/majordomus-edit-guard
printf '#!/bin/sh\nMAJORDOMUS_SHARE=%s exec %s "$@"\n' "$ROOT/share" "$RB" > bin/majordomus-cli && chmod +x bin/majordomus-cli
expect_exit 0 "$MJ" capture install
echo a > lib/a && echo d > docs/d && git add -A && git commit -qm base
mkdir -p "$T/state" "$T/bin"
printf '#!/bin/sh\necho "gh: not here in this case" >&2\nexit 4\n' > "$T/bin/gh"
chmod +x "$T/bin/gh"
export PATH="$T/bin:$PATH" XDG_STATE_HOME="$T/state"
( exec "$RB" serve --port 0 --idle 0 --discovery filesystem </dev/null >"$T/serve.log" 2>&1 ) &
PIDS="$PIDS $!"
ready() { [ -n "$(jq -r '.url // empty' .ai/local/state/mcp/server.json 2>/dev/null)" ]; }
i=0; until ready; do i=$((i + 1)); [ "$i" -lt 200 ] || { cat "$T/serve.log"; exit 1; }; sleep 0.2; done

# The command the generated configuration names for an Edit, resolved as the provider does it.
cmd="$(jq -r '.hooks.PreToolUse[] | select(.matcher | split("|") | index("Edit")) | .hooks[0].command' .claude/settings.json)"
[ -n "$cmd" ] || { echo "    the generated configuration names no PreToolUse command for Edit"; cat .claude/settings.json; exit 1; }
cmd="$(printf '%s' "$cmd" | sed "s|\${CLAUDE_PROJECT_DIR}|$R|")"
[ -x "$cmd" ] || { echo "    the generated command $cmd is not an executable"; exit 1; }
LOG=.ai/local/state/guard/unclaimed.log

# provider <session> <tool> <path> -> the exit code; stderr in $T/err
provider() {
  printf '{"session_id":"%s","tool_name":"%s","tool_input":{"file_path":"%s"}}' "$1" "$2" "$3" \
    | (cd "$R" && CLAUDE_PROJECT_DIR="$R" "$cmd") 2>"$T/err"
}

# ---------------------------------------------------------------- 1. unclaimed: let through, said, recorded
rc=0; provider e2e1 Edit "$R/lib/a" || rc=$?
[ "$rc" = 0 ] || { echo "    an advisory guard refused the edit: exit $rc"; cat "$T/err"; exit 1; }
grep -q 'majordomus guard (advisory): no claim covers lib/a' "$T/err" || { echo "    the unclaimed edit was not said:"; cat "$T/err"; exit 1; }
grep -q -- '--session claude-code-e2e1' "$T/err" || { echo "    the remedy was not said:"; cat "$T/err"; exit 1; }
grep -q 'edit_guard_mode: refuse' "$T/err" || { echo "    the stderr does not say what would have refused it"; cat "$T/err"; exit 1; }
expect_file "$LOG"
awk -F '\t' 'NF == 3 && $1 ~ /^[0-9]{4}-[0-9]{2}-[0-9]{2}T[0-9:]{8}Z$/ && $2 == "claude-code-e2e1" && $3 == "lib/a"' "$LOG" | grep -q . \
  || { echo "    the record is not time, session, path:"; cat "$LOG"; exit 1; }
rc=0; provider e2e1 Write "$R/docs/new.md" || rc=$?
[ "$rc" = 0 ] || { echo "    a Write of a new file was refused: exit $rc"; cat "$T/err"; exit 1; }
[ "$(wc -l < "$LOG" | tr -d ' ')" = 2 ] || { echo "    one line per unclaimed edit was not kept:"; cat "$LOG"; exit 1; }

# ---------------------------------------------------------------- 2. claimed: nothing to say, nothing recorded
expect_exit 0 "$RB" mesh claim lib --session claude-code-e2e2 --advisory --intent "case 4522"
rc=0; provider e2e2 Edit "$R/lib/a" || rc=$?
[ "$rc" = 0 ] || { echo "    a claimed edit was refused: exit $rc"; cat "$T/err"; exit 1; }
[ ! -s "$T/err" ] || { echo "    a claimed edit was reported:"; cat "$T/err"; exit 1; }
expect_no_grep 'claude-code-e2e2' "$LOG"

# ---------------------------------------------------------------- 3. a tool that names no file is not judged
printf '{"session_id":"e2e3","tool_name":"Bash","tool_input":{"command":"echo x > lib/a"}}' \
  | (cd "$R" && "$cmd") 2>"$T/err" || { echo "    a Bash call was refused"; exit 1; }
expect_no_grep 'claude-code-e2e3' "$LOG"

# ---------------------------------------------------------------- 4. the claim-coverage report names the session
"$MJ" doctor > "$T/doctor.out" 2>&1 || true
grep -qE '^WARN +guard +claude-code-e2e1 — 2 unclaimed edit\(s\) in the last 12h, the newest at [0-9TZ:-]+: docs/new.md in this checkout' "$T/doctor.out" \
  || { echo "    doctor does not name the session that edited unclaimed:"; grep ' guard ' "$T/doctor.out"; exit 1; }
expect_no_grep 'claude-code-e2e2' "$T/doctor.out"

# ---------------------------------------------------------------- 5. Claude Code itself (opt-in)
if [ "${MJ_CLAUDE_E2E:-0}" = 1 ]; then
  command -v claude >/dev/null 2>&1 || { echo "    MJ_CLAUDE_E2E=1 and there is no claude on PATH"; exit 1; }
  before="$(wc -l < "$LOG" | tr -d ' ')"
  # its project and no MCP server: the guard is a hook, and the hook is what is measured
  timeout 300 claude -p 'Create the file notes/advisory.md containing the single line: advisory' \
    --strict-mcp-config --allowedTools Write --permission-mode acceptEdits >"$T/claude.out" 2>&1 \
    || { echo "    the client failed: $(tail -3 "$T/claude.out" | tr '\n' ' ')"; exit 1; }
  [ -f notes/advisory.md ] || { echo "    the client wrote no notes/advisory.md, so the advisory guard stopped it:"; tail -5 "$T/claude.out"; exit 1; }
  [ "$(wc -l < "$LOG" | tr -d ' ')" -gt "$before" ] || { echo "    Claude Code's Write did not reach the guard: no line was recorded"; exit 1; }
  tail -n 1 "$LOG" | awk -F '\t' '$2 ~ /^claude-code-./ && $3 == "notes/advisory.md"' | grep -q . \
    || { echo "    the client's edit is not recorded under its session:"; tail -n 1 "$LOG"; exit 1; }
fi

echo "    the generated PreToolUse command reports and records an unclaimed edit, lets it through, and doctor names the session"
