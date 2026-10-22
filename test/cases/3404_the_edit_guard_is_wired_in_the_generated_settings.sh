# majordomus-timeout: 300
# The edit guard is wired in the configuration `capture install` generates (I3322). Before it,
# no PreToolUse entry was written anywhere, so Claude Code never called the guard case 3069
# exercises, although a policy could turn edits_require_claim on and docs/MESH.md said it was
# wired. Held here on a fixture repository of this case's own, with a server and an enabled mesh:
#
#   wired       a configuration capture install writes carries the PreToolUse entry, matching
#               the tools that write a file, and names the guard shim it writes beside it
#   status      capture status counts the guard: no "does not wire the edit guard"
#   advisory    with session.edit_guard: advisory, an unclaimed edit through the shim exits 0
#               with the reason and the remedy as Claude Code's additionalContext and no
#               permission decision, is noted, and doctor's claim coverage names its session
#   refuse      with session.edit_guard: refuse, the same edit exits 2
#   drift       a shim edited by hand, and a configuration without the entry, are reported
#   theirs      a configuration capture install did not write is not rewritten: the missing
#               entry is named with its matcher, and the file is left as it was
#   client      opt-in, as case 2980 is: with MJ_CLAUDE_E2E=1 and a signed-in `claude` on PATH,
#               a real Claude Code session writes a file nobody claimed through the generated
#               configuration, the edit goes ahead, and the guard noted it under that
#               session's id; the client's version is printed, the version this was run against
. "$ROOT/test/lib.sh"
command -v jq >/dev/null 2>&1 || skip "jq absent"
RB="$(rust_bin)" || rust_bin_exit $?
export MAJORDOMUS_SHARE="$ROOT/share" MAJORDOMUS_BIN="$RB"

PIDS=""
trap 'for p in $PIDS; do kill "$p" 2>/dev/null || true; done' EXIT

CFG=.claude/settings.json
SHIM=.claude/hooks/majordomus-edit-guard
R="$T/repo"
mkdir -p "$R"
cd "$R" || exit 1
git init -q .
git config user.email t@example.com
git config user.name t
"$MJ" init >/dev/null
"$MJ" update >/dev/null
sed -i.bak 's/^  edits_require_claim: false.*/  edits_require_claim: true/; s/^  edit_guard: refuse.*/  edit_guard: advisory/' \
  .ai/repo/policy.yaml && rm -f .ai/repo/policy.yaml.bak
grep -q '^  edit_guard: advisory' .ai/repo/policy.yaml || { echo "    the fixture's policy declares no advisory guard"; exit 1; }
mkdir -p .ai/repo/mesh docs
printf 'schema: mesh/v1\nkind: mesh-declaration\nid: majordomus\nenabled: true\n' > .ai/repo/mesh/majordomus.yaml
printf 'multicast:\n  enabled: false\ntrust:\n  policy: tofu\n' >> .ai/repo/mesh/majordomus.yaml
printf 'cooperation:\n  heartbeat_seconds: 1\n  expiry_seconds: 30\n  repository: case-3404\n' >> .ai/repo/mesh/majordomus.yaml
echo d > docs/d && git add -A && git commit -qm base

# ---------------------------------------------------------------- wired
rm -f "$CFG"
"$MJ" capture install --provider claude-code >"$T/install.out" 2>&1 || { cat "$T/install.out"; exit 1; }
[ -f "$CFG" ] || { echo "    capture install wrote no $CFG"; exit 1; }
matcher="$(jq -r '.hooks.PreToolUse[0].matcher // empty' "$CFG")"
[ "$matcher" = 'Edit|Write|MultiEdit|NotebookEdit' ] || { echo "    the PreToolUse matcher is '$matcher'"; cat "$CFG"; exit 1; }
cmd="$(jq -r '.hooks.PreToolUse[0].hooks[0].command // empty' "$CFG")"
[ "$cmd" = "\${CLAUDE_PROJECT_DIR}/$SHIM" ] || { echo "    the PreToolUse entry runs '$cmd'"; exit 1; }
[ -x "$SHIM" ] || { echo "    $SHIM was not written executable"; exit 1; }
grep -qF 'Written by `majordomus capture install`' "$SHIM" || { echo "    $SHIM does not carry the install's marker"; exit 1; }
echo "    wired: the generated configuration calls $SHIM before Edit, Write, MultiEdit and NotebookEdit"

# ---------------------------------------------------------------- status
"$MJ" capture status >"$T/status.out" 2>&1 || true
if grep -q 'does not wire the edit guard' "$T/status.out"; then
  echo "    capture status says the guard is unwired after install"; cat "$T/status.out"; exit 1
fi
echo "    status: capture status counts the guard as wired"

# ---------------------------------------------------------------- advisory
mkdir -p "$T/bin" "$T/state" "$T/elsewhere"
ln -s "$RB" "$T/bin/majordomus-cli"
printf '#!/bin/sh\necho "gh: not here in this case" >&2\nexit 4\n' > "$T/bin/gh"
chmod +x "$T/bin/gh"
export PATH="$T/bin:$PATH" XDG_STATE_HOME="$T/state"
( exec "$RB" serve --port 0 --idle 0 --discovery filesystem </dev/null >"$T/serve.log" 2>&1 ) &
PIDS="$PIDS $!"
ready() { [ -n "$(jq -r '.url // empty' .ai/local/state/mcp/server.json 2>/dev/null)" ]; }
i=0; until ready; do i=$((i + 1)); [ "$i" -lt 200 ] || { cat "$T/serve.log"; exit 1; }; sleep 0.2; done

# shim <session> <path> -> its exit code; stdout in $T/out, stderr in $T/err
shim() {
  printf '{"session_id":"%s","tool_name":"Edit","tool_input":{"file_path":"%s"}}' "$1" "$2" \
    | (cd "$T/elsewhere" && "$R/$SHIM") >"$T/out" 2>"$T/err"
}
rc=0; shim cc1 "$R/docs/d" || rc=$?
[ "$rc" = 0 ] || { echo "    an advisory guard refused (exit $rc)"; cat "$T/err"; exit 1; }
jq -e '.hookSpecificOutput.hookEventName == "PreToolUse"' "$T/out" >/dev/null \
  || { echo "    the advisory answer is not a PreToolUse answer"; cat "$T/out"; exit 1; }
jq -e '.hookSpecificOutput | has("permissionDecision") | not' "$T/out" >/dev/null \
  || { echo "    the advisory answer makes a permission decision"; cat "$T/out"; exit 1; }
note="$(jq -r '.hookSpecificOutput.additionalContext' "$T/out")"
case "$note" in *'no claim covers docs/d'*'--session claude-code-cc1'*) ;; *) echo "    the note lacks the path or the remedy: $note"; exit 1 ;; esac
[ -s "$T/err" ] && { echo "    the advisory guard wrote to stderr, which the model never sees on exit 0"; cat "$T/err"; exit 1; }
jq -e 'select(.session == "claude-code-cc1" and .path == "docs/d")' .ai/local/state/guard/unclaimed.jsonl >/dev/null \
  || { echo "    the unclaimed edit was not noted"; exit 1; }
"$MJ" doctor >"$T/doctor.out" 2>&1 || true
grep -q 'claude-code-cc1 .*edit(s) no claim covered' "$T/doctor.out" \
  || { echo "    doctor's claim coverage does not name the session"; grep -i guard "$T/doctor.out"; exit 1; }
echo "    advisory: the edit goes ahead with the reason as additionalContext, is noted, and doctor names claude-code-cc1"

# ---------------------------------------------------------------- refuse
sed -i.bak 's/^  edit_guard: advisory/  edit_guard: refuse/' .ai/repo/policy.yaml && rm -f .ai/repo/policy.yaml.bak
rc=0; shim cc1 "$R/docs/d" || rc=$?
[ "$rc" = 2 ] || { echo "    a refusing guard let the edit through (exit $rc)"; cat "$T/out" "$T/err"; exit 1; }
grep -q 'no claim covers docs/d' "$T/err" || { echo "    the refusal does not name the path"; cat "$T/err"; exit 1; }
[ -s "$T/out" ] && { echo "    a refusal also answered on stdout"; cat "$T/out"; exit 1; }
echo "    refuse: the same edit exits 2 with the reason on stderr"

# ---------------------------------------------------------------- drift
cp "$SHIM" "$T/shim.keep"
printf '# a hand edit\n' >> "$SHIM"
"$MJ" capture status >"$T/status.out" 2>&1 || true
grep -q 'edited by hand' "$T/status.out" || { echo "    a hand-edited guard shim is not reported"; cat "$T/status.out"; exit 1; }
cp "$T/shim.keep" "$SHIM"
cp "$CFG" "$T/cfg.keep"
jq 'del(.hooks.PreToolUse)' "$T/cfg.keep" > "$CFG"
"$MJ" capture status >"$T/status.out" 2>&1 || true
grep -q 'does not wire the edit guard' "$T/status.out" || { echo "    a configuration without the guard is not reported"; cat "$T/status.out"; exit 1; }
echo "    drift: a hand-edited shim and a configuration without the entry are both reported"

# ---------------------------------------------------------------- theirs
before="$(cat "$CFG")"
rc=0; "$MJ" capture install --provider claude-code >"$T/install.out" 2>&1 || rc=$?
[ "$rc" != 0 ] || { echo "    capture install accepted a configuration that lacks the guard"; exit 1; }
grep -q '"PreToolUse": \[ { "matcher": "Edit|Write|MultiEdit|NotebookEdit"' "$T/install.out" \
  || { echo "    the missing entry is not named with its matcher"; cat "$T/install.out"; exit 1; }
[ "$(cat "$CFG")" = "$before" ] || { echo "    a configuration this tool did not write was rewritten"; exit 1; }
echo "    theirs: an existing configuration is named its missing entry and left as it was"

# ---------------------------------------------------------------- client (opt-in)
if [ "${MJ_CLAUDE_E2E:-0}" = 1 ]; then
  command -v claude >/dev/null 2>&1 || { echo "    MJ_CLAUDE_E2E=1 and there is no claude on PATH"; exit 1; }
  cp "$T/cfg.keep" "$CFG"
  sed -i.bak 's/^  edit_guard: refuse/  edit_guard: advisory/' .ai/repo/policy.yaml && rm -f .ai/repo/policy.yaml.bak
  rm -f .ai/local/state/guard/unclaimed.jsonl
  echo "    client: $(claude --version 2>/dev/null | head -1)"
  timeout 300 claude -p 'Create the file docs/e2e.md containing the single word ok. Do nothing else.' \
    --allowedTools Write,Edit --output-format json >"$T/claude.out" 2>&1 \
    || { echo "    the client failed: $(tail -3 "$T/claude.out" | tr '\n' ' ')"; exit 1; }
  [ "$(cat docs/e2e.md 2>/dev/null)" = ok ] || { echo "    the advisory guard did not let the client's edit go ahead"; exit 1; }
  sid="$(jq -r '.session_id // empty' "$T/claude.out")"
  jq -e --arg s "claude-code-$sid" 'select(.session == $s and .path == "docs/e2e.md")' \
    .ai/local/state/guard/unclaimed.jsonl >/dev/null \
    || { echo "    the client's edit was not noted by the guard under claude-code-$sid"; exit 1; }
  echo "    client: a real session's unclaimed write went ahead and was noted under claude-code-$sid"
fi
exit 0
