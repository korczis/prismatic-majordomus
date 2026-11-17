# majordomus-covers: capture
# A generated hook declares how long the provider lets it run, and the session end closes its
# episode inside that budget (I3060).
#
# Claude Code cancels a command hook at its timeout and discards what it did not finish, and
# without a per-hook timeout every SessionEnd hook shares 1.5 seconds. The end event does a
# closing checkpoint, a continuation and the close, which took 3 to 8 seconds on a loaded
# adopter, so the episode was cut short before it closed. This case proves the three halves
# of the fix by driving what the provider would drive: the configuration `capture install`
# writes, the upgrade of one it wrote before, and the end shim under a budget.
. "$ROOT/test/lib.sh"
"$MJ" init >/dev/null; "$MJ" update >/dev/null
# the start event's server is case 108's, not this one's
sed 's/^  ensure_server_on_start: true /  ensure_server_on_start: false /' .ai/repo/policy.yaml > "$T/pol" && cp "$T/pol" .ai/repo/policy.yaml
mkdir -p lib && echo a > lib/a && git add . && git commit -qm base
PATH="$(dirname "$MJ"):$PATH"; export PATH

cfg=.claude/settings.json
# The timeout the configuration gives the handler running <shim>: the line after its command.
timeout_of() { grep -A1 -F "\${CLAUDE_PROJECT_DIR}/.claude/hooks/$1\"" "$cfg" | sed -n 's/^ *"timeout": \([0-9]*\)$/\1/p'; }

# ---------------------------------------------------------------- a fresh configuration
rm -f "$cfg"
expect_exit 0 "$MJ" capture install
expect_grep 'written with the'
[ "$(timeout_of majordomus-capture)" = 30 ] || { echo "    UserPromptSubmit: timeout '$(timeout_of majordomus-capture)', want 30"; cat "$cfg"; exit 1; }
[ "$(timeout_of majordomus-session-start)" = 30 ] || { echo "    SessionStart: timeout '$(timeout_of majordomus-session-start)', want 30"; exit 1; }
[ "$(timeout_of majordomus-session-end)" = 60 ] || { echo "    SessionEnd: timeout '$(timeout_of majordomus-session-end)', want 60"; exit 1; }
[ "$(timeout_of majordomus-session-compact)" = 60 ] || { echo "    PreCompact: timeout '$(timeout_of majordomus-session-compact)', want 60"; exit 1; }
# Claude Code raises the SessionEnd budget to a per-hook timeout only up to 60 seconds; a longer
# one would be a budget this tool believes it has and the provider does not give.
[ "$(timeout_of majordomus-session-end)" -le 60 ]
# still JSON, where a reader is installed to say so
if command -v jq >/dev/null 2>&1; then
  jq -e '.hooks.SessionEnd[0].hooks[0].timeout == 60' "$cfg" >/dev/null || { echo "    jq does not read the SessionEnd timeout"; exit 1; }
fi
cp "$cfg" "$T/current"
expect_exit 0 "$MJ" capture install
expect_grep 'already names the hooks'
cmp -s "$cfg" "$T/current" || { echo "    a second install changed the configuration"; exit 1; }

# ---------------------------------------------------------------- the upgrade
# Exactly what this tool wrote before it declared timeouts is this tool's, and is brought up
# to date by the command that wrote it.
sed -E -e '/^ *"timeout": [0-9]+$/d' -e 's/^( *"command": ".*"),$/\1/' "$T/current" > "$cfg"
expect_no_grep 'timeout' "$cfg"
expect_exit 0 "$MJ" capture install
expect_grep 'rewritten: every hook now carries its timeout'
cmp -s "$cfg" "$T/current" || { echo "    the upgraded configuration is not the current one"; diff "$T/current" "$cfg"; exit 1; }

# One byte more and it is somebody's: left exactly as it is, and what it lacks is named.
sed -E -e '/^ *"timeout": [0-9]+$/d' -e 's/^( *"command": ".*"),$/\1/' "$T/current" \
  | awk 'NR == 1 { print; print "  \"permissions\": { \"allow\": [] },"; next } { print }' > "$cfg"
cp "$cfg" "$T/foreign"
expect_exit 0 "$MJ" capture install
cmp -s "$cfg" "$T/foreign" || { echo "    a configuration this tool did not write was rewritten"; exit 1; }
if command -v jq >/dev/null 2>&1; then
  expect_grep 'SessionEnd handler running .*majordomus-session-end carries no timeout; add "timeout": 60'
  expect_grep '4 hook\(s\) carry no timeout'
else
  expect_grep 'not checked: jq is not installed'
fi
cp "$T/current" "$cfg"

# ---------------------------------------------------------------- the end fits its budget
records() { find .ai/repo/sessions -maxdepth 1 -name '2*.md' 2>/dev/null | wc -l | tr -d ' '; }
was="$(records)"
printf '{"session_id":"cc-budget-1","source":"startup"}' | ./.claude/hooks/majordomus-session-start >/dev/null 2>&1
expect_exit 0 "$MJ" session status
expect_grep 'Session: +s-'
begin=$SECONDS
printf '{"session_id":"cc-budget-1","reason":"prompt_input_exit"}' | ./.claude/hooks/majordomus-session-end 2>"$T/err"
took=$((SECONDS - begin))
[ "$took" -lt "$(timeout_of majordomus-session-end)" ] || { echo "    the end hook took ${took}s, over its declared timeout"; exit 1; }
grep -qF 'episode closed (closed' "$T/err" || { echo "    the end hook did not close the episode"; cat "$T/err"; exit 1; }
grep -qF 'continuation written' "$T/err" || { echo "    with the whole budget the continuation was not written"; cat "$T/err"; exit 1; }
[ "$(records)" = "$((was + 1))" ] || { echo "    the end event wrote no session record"; exit 1; }

# ---------------------------------------------------------------- a budget already spent
# When the optional steps would leave the close too little of the budget, they are skipped and
# said to be skipped, and the episode still closes.
was="$(records)"
printf '{"session_id":"cc-budget-2","source":"startup"}' | ./.claude/hooks/majordomus-session-start >/dev/null 2>&1
printf '{"session_id":"cc-budget-2","reason":"prompt_input_exit"}' \
  | MJ_CAPTURE_END_BUDGET_MS=1 ./.claude/hooks/majordomus-session-end 2>"$T/err"
grep -qF 'episode closed (closed' "$T/err" || { echo "    a spent budget lost the close"; cat "$T/err"; exit 1; }
expect_no_grep 'continuation written' "$T/err"
[ "$(records)" = "$((was + 1))" ] || { echo "    a spent budget wrote no session record"; exit 1; }
grep -rqF 'the closing checkpoint was skipped' .ai/local || { echo "    the skipped checkpoint left no typed failure"; exit 1; }
grep -rqF 'the continuation was skipped' .ai/local || { echo "    the skipped continuation left no typed failure"; exit 1; }
expect_exit 0 "$MJ" session status
expect_grep 'No open session'
