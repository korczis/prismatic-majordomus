# majordomus-covers: capture
# `capture install` wires the edit guard into Claude Code's PreToolUse, the lifecycle check
# reports a guard that is unwired or hand-edited, and doctor's claim-coverage report names the
# sessions that edited without a claim (I3322).
#
# Until I3322 nothing wrote a PreToolUse entry: the shim existed, case 3069 piped payloads into
# it by hand, the policy asked every edit for a claim, and Claude Code never called it. This case
# reads the configuration `capture install` generates, not the shim: the entry, its matcher, the
# command and its timeout; then each way the wiring can stop reaching the guard, as `capture
# status` words it; then this repository's own .claude/settings.json against the generated
# entry; then the report doctor builds from the advisory records the guard writes. It needs no
# executable of this tree: the judgement is case 3069's, and the end-to-end path is case 4522's.
. "$ROOT/test/lib.sh"
"$MJ" init >/dev/null; "$MJ" update >/dev/null
# the start event's server is case 108's, not this one's
sed 's/^  ensure_server_on_start: true /  ensure_server_on_start: false /' .ai/repo/policy.yaml > "$T/pol" && cp "$T/pol" .ai/repo/policy.yaml
mkdir -p lib && echo a > lib/a && git add . && git commit -qm base
PATH="$(dirname "$MJ"):$PATH"; export PATH

cfg=.claude/settings.json
shim=.claude/hooks/majordomus-edit-guard
matcher='Edit|Write|MultiEdit|NotebookEdit'
session_state() { "$MJ" capture status | awk '$1 == "claude-code:session"'; }

# ---------------------------------------------------------------- 1. no guard here, no entry
# A checkout that does not carry the guard's shim has nothing to wire: the entry would name a
# file that does not exist (I3300 is what carries it to such a checkout).
rm -f "$cfg"
expect_exit 0 "$MJ" capture install
expect_no_grep 'PreToolUse' "$cfg"
[ "$(session_state | awk '{ print $2 }')" = verified ] || { echo "    without a guard the lifecycle is not verified:"; session_state; exit 1; }

# ---------------------------------------------------------------- 2. the guard carried: the entry is written
mkdir -p .claude/hooks && cp "$ROOT/$shim" "$shim" && chmod +x "$shim"
rm -f "$cfg"
expect_exit 0 "$MJ" capture install
expect_grep 'written with the UserPromptSubmit, SessionStart, SessionEnd, PreCompact, PreToolUse hook\(s\)'
# the entry as Claude Code reads it: under hooks.PreToolUse, a group with the matcher, a
# command handler running the shim through the project-directory placeholder, with a timeout
grep -A8 -F '"PreToolUse": [' "$cfg" > "$T/entry"
grep -qF "\"matcher\": \"$matcher\"" "$T/entry" || { echo "    the PreToolUse entry does not match the four tools that write a file:"; cat "$cfg"; exit 1; }
grep -qF "\"command\": \"\${CLAUDE_PROJECT_DIR}/$shim\"" "$T/entry" || { echo "    the PreToolUse entry does not run the guard's shim:"; cat "$cfg"; exit 1; }
grep -qE '"timeout": 30$' "$T/entry" || { echo "    the PreToolUse handler carries no 30 s timeout:"; cat "$cfg"; exit 1; }
if command -v jq >/dev/null 2>&1; then
  jq -e --arg m "$matcher" --arg c "\${CLAUDE_PROJECT_DIR}/$shim" \
    '.hooks.PreToolUse == [{matcher: $m, hooks: [{type: "command", command: $c, timeout: 30}]}]' "$cfg" >/dev/null \
    || { echo "    jq does not read the PreToolUse entry as generated:"; jq .hooks.PreToolUse "$cfg"; exit 1; }
  # the episode hooks are untouched and match every occurrence
  jq -e '[.hooks.SessionStart, .hooks.SessionEnd, .hooks.PreCompact, .hooks.UserPromptSubmit] | all(.[0].matcher == "")' "$cfg" >/dev/null \
    || { echo "    an episode hook gained a matcher"; exit 1; }
fi
cp "$cfg" "$T/current"
state="$(session_state)"
printf '%s\n' "$state" | grep -qE '^claude-code:session +verified .*majordomus-edit-guard' \
  || { echo "    the lifecycle check does not count the wired guard:"; printf '%s\n' "$state"; exit 1; }
expect_exit 0 "$MJ" capture install
expect_grep 'already names the hooks'
cmp -s "$cfg" "$T/current" || { echo "    a second install changed the configuration"; exit 1; }

# ---------------------------------------------------------------- 3. what this tool wrote before the guard is brought up to date
awk 'BEGIN { skip = 0 }
  /^    "PreToolUse": \[$/ { skip = 1 }
  skip { if ($0 ~ /^    \]$/) skip = 0; next }
  { lines[++n] = $0 }
  END { for (i = 1; i <= n; i++) { l = lines[i]; if (i < n && lines[i + 1] ~ /^  }$/ && l ~ /^    \],$/) l = "    ]"; print l } }' \
  "$T/current" > "$cfg"
expect_no_grep 'PreToolUse' "$cfg"
if command -v jq >/dev/null 2>&1; then jq -e . "$cfg" >/dev/null || { echo "    the fixture is not JSON:"; cat "$cfg"; exit 1; }; fi
[ "$(session_state | awk '{ print $2 }')" = unconfigured ] || { echo "    an unwired guard is not reported:"; session_state; exit 1; }
session_state | grep -qF 'declares no PreToolUse hook, so no edit reaches .claude/hooks/majordomus-edit-guard' \
  || { echo "    the unwired guard is not named:"; session_state; exit 1; }
expect_exit 0 "$MJ" capture install
expect_grep 'rewritten: the edit guard is wired \(PreToolUse Edit\|Write\|MultiEdit\|NotebookEdit\)'
cmp -s "$cfg" "$T/current" || { echo "    the upgraded configuration is not the current one"; diff "$T/current" "$cfg"; exit 1; }

# ---------------------------------------------------------------- 4. a hand-edited guard is reported, and left as it is
sed 's/"matcher": "Edit|Write|MultiEdit|NotebookEdit"/"matcher": "Edit|Write"/' "$T/current" > "$cfg"
cp "$cfg" "$T/narrowed"
state="$(session_state)"
printf '%s\n' "$state" | grep -qE '^claude-code:session +named ' || { echo "    a narrowed matcher reads as wired:"; printf '%s\n' "$state"; exit 1; }
printf '%s\n' "$state" | grep -qF 'matches "Edit|Write" rather than "Edit|Write|MultiEdit|NotebookEdit"' \
  || { echo "    the narrowed matcher is not named:"; printf '%s\n' "$state"; exit 1; }
printf '%s\n' "$state" | grep -qF 'hand-edited' || { echo "    the finding does not say it was edited by hand"; exit 1; }
expect_exit 0 "$MJ" capture install
cmp -s "$cfg" "$T/narrowed" || { echo "    a configuration this tool did not write was rewritten"; exit 1; }
# a guard the provider cannot run
cp "$T/current" "$cfg"; chmod -x "$shim"
session_state | grep -qF "$shim is not executable" || { echo "    a shim the provider cannot run is not reported:"; session_state; exit 1; }
chmod +x "$shim"
# somebody's own configuration without the entry is named the entry to add, matcher included
if command -v jq >/dev/null 2>&1; then
  jq '. + {permissions: {allow: []}} | del(.hooks.PreToolUse)' "$T/current" > "$cfg"
  cp "$cfg" "$T/foreign"
  expect_exit 15 "$MJ" capture install
  printf '%s\n' "$LAST_OUT" | grep -qF "\"PreToolUse\": [ { \"matcher\": \"$matcher\", \"hooks\": [ { \"type\": \"command\", \"command\": \"\${CLAUDE_PROJECT_DIR}/$shim\"" \
    || { echo "    the refusal does not name the entry to add, matcher included:"; printf '%s\n' "$LAST_OUT"; exit 1; }
  cmp -s "$cfg" "$T/foreign" || { echo "    somebody's configuration was rewritten"; exit 1; }
fi
cp "$T/current" "$cfg"

# ---------------------------------------------------------------- 5. this repository's own configuration carries the entry as generated
if command -v jq >/dev/null 2>&1; then
  [ "$(jq -c .hooks.PreToolUse "$ROOT/$cfg")" = "$(jq -c .hooks.PreToolUse "$T/current")" ] \
    || { echo "    $ROOT/$cfg does not carry the PreToolUse entry capture install writes:"; jq .hooks.PreToolUse "$ROOT/$cfg"; exit 1; }
else
  grep -qF "\"matcher\": \"$matcher\"" "$ROOT/$cfg" && grep -qF "\${CLAUDE_PROJECT_DIR}/$shim" "$ROOT/$cfg" \
    || { echo "    $ROOT/$cfg does not wire the guard"; exit 1; }
fi
grep -qE '^  edit_guard_mode: advisory$' "$ROOT/.ai/repo/policy.yaml" \
  || { echo "    this repository's policy does not declare the guard advisory (refusal is I3323)"; exit 1; }

# ---------------------------------------------------------------- 6. the claim-coverage report
sed -i.bak 's/^  edits_require_claim: false/  edits_require_claim: true/' .ai/repo/policy.yaml && rm -f .ai/repo/policy.yaml.bak
grep -qE '^  edit_guard_mode: advisory( |$)' .ai/repo/policy.yaml || { echo "    the skeleton does not declare the guard advisory"; exit 1; }
export MAJORDOMUS_NOW=2026-11-01T12:00:00Z
"$MJ" doctor > "$T/doctor.out" 2>&1 || true
grep -qE '^OK +guard +claim coverage — no session edited a file without a claim in the last 12h \(session.edit_guard_mode: advisory\)' "$T/doctor.out" \
  || { echo "    with no record, the report does not say nobody edited unclaimed:"; grep ' guard ' "$T/doctor.out"; exit 1; }
# the lines the advisory guard writes: two by one session, one by another, one too old to be
# somebody editing now (stranded_after is 12h), one that is not a record
mkdir -p .ai/local/state/guard
printf '%s\t%s\t%s\n' \
  2026-11-01T09:00:00Z claude-code-aa lib/a \
  2026-11-01T11:30:00Z claude-code-aa lib/b \
  2026-11-01T10:00:00Z claude-code-bb README.md \
  2026-10-30T10:00:00Z claude-code-cc docs/old.md > .ai/local/state/guard/unclaimed.log
printf 'not a record\n' >> .ai/local/state/guard/unclaimed.log
"$MJ" doctor > "$T/doctor.out" 2>&1 || true
grep ' guard ' "$T/doctor.out" > "$T/guard.out" || true
grep -qE '^WARN +guard +claude-code-aa — 2 unclaimed edit\(s\) in the last 12h, the newest at 2026-11-01T11:30:00Z: lib/b in this checkout +\[reproduce: majordomus mesh claim lib/b --session claude-code-aa --advisory\]' "$T/guard.out" \
  || { echo "    the session with two unclaimed edits is not reported as such:"; cat "$T/guard.out"; exit 1; }
grep -qE '^WARN +guard +claude-code-bb — 1 unclaimed edit\(s\)' "$T/guard.out" || { echo "    the second session is not reported:"; cat "$T/guard.out"; exit 1; }
expect_no_grep 'claude-code-cc' "$T/guard.out"
[ "$(wc -l < "$T/guard.out" | tr -d ' ')" = 2 ] || { echo "    the report is not one line per live session:"; cat "$T/guard.out"; exit 1; }
# a report and never a failure: the guard lets these edits through by design
expect_no_grep '^FAIL +guard' "$T/doctor.out"
# a linked worktree's records are read too: a session edits files of other checkouts
git worktree add -q "$T/linked" -b linked 2>/dev/null
mkdir -p "$T/linked/.ai/local/state/guard"
printf '2026-11-01T11:00:00Z\tclaude-code-dd\tlib/a\n' > "$T/linked/.ai/local/state/guard/unclaimed.log"
"$MJ" doctor > "$T/doctor.out" 2>&1 || true
grep -qE "^WARN +guard +claude-code-dd — 1 unclaimed edit\(s\) in the last 12h, the newest at 2026-11-01T11:00:00Z: lib/a in $(cd "$T/linked" && pwd -P)" "$T/doctor.out" \
  || { echo "    a linked worktree's unclaimed edit is not reported:"; grep ' guard ' "$T/doctor.out"; exit 1; }
# the switch off: the guard judges nothing, and the report says so rather than listing
sed -i.bak 's/^  edits_require_claim: true/  edits_require_claim: false/' .ai/repo/policy.yaml && rm -f .ai/repo/policy.yaml.bak
"$MJ" doctor > "$T/doctor.out" 2>&1 || true
grep -qE '^INFO +guard +claim coverage — session.edits_require_claim is off' "$T/doctor.out" \
  || { echo "    with the switch off the report does not say so:"; grep ' guard ' "$T/doctor.out"; exit 1; }

echo "    capture install wires PreToolUse $matcher to the guard; unwired, hand-edited and unrunnable guards are reported; doctor names the live sessions editing unclaimed"
