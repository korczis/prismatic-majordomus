# majordomus-covers: capture session
# majordomus-negative: capture session doctor
# The episode boundary drawn by the provider: the shims, the three events, what each does
# twice, and what exactly one of them may write to stdout.
#
# The point of this case is that the boundary is real without anybody typing a command. Every
# assertion below drives the shim the provider would run, with the payload the provider would
# send, because a lifecycle that only works when a test calls the library proves nothing about
# a session that ended in a crash.
. "$ROOT/test/lib.sh"
"$MJ" init >/dev/null; "$MJ" update >/dev/null
# this case is about the boundary; the server the start event would ensure is case 108's
sed 's/^  ensure_server_on_start: true /  ensure_server_on_start: false /' .ai/repo/policy.yaml > "$T/pol" && cp "$T/pol" .ai/repo/policy.yaml
mkdir -p lib && echo a > lib/a && git add . && git commit -qm base
# how a repository that is not the tool's own checkout reaches majordomus
PATH="$(dirname "$MJ"):$PATH"; export PATH

# ---------------------------------------------------------------- surface
expect_exit 2 "$MJ" capture
expect_grep 'majordomus capture session --provider'
# 12 and not 2, for the reason the prompt path never exits 2: this runs inside a provider
# hook, and there the wrong exit code costs the person their session.
expect_exit 12 "$MJ" capture session --provider claude-code
expect_grep 'must be one of start end compact'
expect_exit 12 "$MJ" capture session --event start
expect_grep 'provider is required'
# A provider with no lifecycle event of its own is refused, and the refusal carries the
# reason the declaration gives rather than a sentence this file made up about somebody
# else's tool. `bb` is an orchestrator: the agent it runs owns the hooks and the episode.
expect_exit 12 "$MJ" capture session --provider bb --event start
expect_grep 'no lifecycle adapter'
expect_grep 'the agent it runs owns the hooks'
# ...and so is an event this tool has no kind for. An event kind a provider does have and a
# second provider does not is case 119's, which publishes a distribution missing one.
expect_exit 12 "$MJ" capture session --provider gemini --event nonsense
expect_grep 'must be one of start end compact'

# ---------------------------------------------------------------- install
expect_exit 0 "$MJ" capture install
[ -x .claude/hooks/majordomus-session-start ]
[ -x .claude/hooks/majordomus-session-end ]
grep -qF 'SessionStart' .claude/settings.json
grep -qF 'SessionEnd' .claude/settings.json
grep -qF '${CLAUDE_PROJECT_DIR}/.claude/hooks/majordomus-session-start' .claude/settings.json
# the shim resolves the repository from its own location, as the prompt shim does
grep -qF 'dirname -- "$0"' .claude/hooks/majordomus-session-end
expect_no_grep 'CLAUDE_PROJECT_DIR' .claude/hooks/majordomus-session-end
expect_exit 0 "$MJ" capture install
expect_grep 'already'

# A configuration written before this tool knew about the lifecycle events is the same case
# as one somebody else wrote: what is missing is named, one event at a time, and nothing is
# rewritten.
cp .claude/settings.json "$T/settings.keep"
printf '{"hooks":{"UserPromptSubmit":[{"matcher":"","hooks":[{"type":"command","command":"${CLAUDE_PROJECT_DIR}/.claude/hooks/majordomus-capture"}]}]}}\n' > .claude/settings.json
expect_exit 15 "$MJ" capture install
expect_grep 'not this tool'
expect_grep 'SessionStart'
expect_grep 'SessionEnd'
expect_no_grep 'UserPromptSubmit.*matcher'      # the event already named is not asked for again
grep -qF 'majordomus-capture' .claude/settings.json    # refused means unchanged
expect_no_grep 'SessionStart' .claude/settings.json
cp "$T/settings.keep" .claude/settings.json

# ---------------------------------------------------------------- the state is run, not read
expect_exit 0 "$MJ" capture status
expect_grep 'claude-code:session +verified'
chmod -x .claude/hooks/majordomus-session-end
expect_exit 0 "$MJ" capture status
expect_grep 'claude-code:session +named'
expect_grep 'not executable'
chmod +x .claude/hooks/majordomus-session-end
expect_exit 0 "$MJ" capture status
expect_grep 'claude-code:session +verified'
# and the two aspects of one provider are two answers: the prompt row is unchanged by any
# of this, so a reader asking whether prompts are captured never gets the other answer
expect_grep 'claude-code +verified'

# the verifier must not close what it is checking: it drove the end shim above, twice
"$MJ" session start >/dev/null
expect_exit 0 "$MJ" capture status
expect_exit 0 "$MJ" session status
expect_grep 'Session:'
"$MJ" session close >/dev/null

# ---------------------------------------------------------------- what the hooks do
contexts() { find .ai/local/session-contexts -maxdepth 1 -name '*.md' 2>/dev/null | wc -l | tr -d ' '; }
records()  { find .ai/repo/sessions -maxdepth 1 -name '2*.md' 2>/dev/null | wc -l | tr -d ' '; }
was_records="$(records)"; was_contexts="$(contexts)"

# A start event opens an episode nobody typed a command for, and freezes the context it was
# given into the store the layer has always named and never filled.
printf '{"session_id":"cc-1","source":"startup"}' | ./.claude/hooks/majordomus-session-start
expect_exit 0 "$MJ" session status
expect_grep 'Session: +s-'
[ "$(contexts)" = "$((was_contexts + 1))" ] || { echo "    the start event did not freeze a working context"; ls -1 .ai/local/session-contexts; exit 1; }
# the document of this episode, found the way a person finds it rather than by globbing
ctx="$("$MJ" session context)"
expect_file "$ctx"
grep -qF 'opened_by: hook' "$ctx"
grep -qF 'provider: claude-code' "$ctx"
# the provider's own session identity is what ties this episode to the prompt archive, whose
# records carry the same string
grep -qF 'provider_session: "cc-1"' "$ctx"
# and what the builder resolved at the open is in it, frozen
grep -qF '## Context at open' "$ctx"
grep -qF '## GIT' "$ctx"

# The start event's stdout is the briefing, because this provider adds it to the context it
# is about to build. That is the one route by which a continuation record reaches a worker
# without the worker remembering to ask, and a record nothing loads is a record nobody reads.
out="$(printf '{"session_id":"cc-1","source":"resume"}' | ./.claude/hooks/majordomus-session-start 2>"$T/err")"
[ -n "$out" ] || { echo "    the start hook wrote no briefing to stdout"; exit 1; }
[ -s "$T/err" ] || { echo "    the start hook said nothing on stderr"; exit 1; }
printf '%s\n' "$out" | grep -qF 'Majordomus' || { echo "    the briefing does not name itself"; exit 1; }
# It is bounded by the policy, not by what happens to be lying around.
budget="$(awk '/^  briefing_budget_lines:/ { print $2 }' .ai/repo/policy.yaml)"
lines="$(printf '%s\n' "$out" | wc -l | tr -d ' ')"
[ "$lines" -le "$((budget + 1))" ] || { echo "    the briefing is $lines lines, budget $budget"; exit 1; }
# ...and a resume is not a second episode: the event fires again, the open one is kept
[ "$(contexts)" = "$((was_contexts + 1))" ] || { echo "    a resume opened a second episode"; ls -1 .ai/local/session-contexts; exit 1; }
grep -qF 'kept' "$T/err"

# The repository that wants the older silence gets it, and the switch is the policy's.
cp .ai/repo/policy.yaml "$T/policy.keep"
sed 's/^  briefing_on_start: true$/  briefing_on_start: false/' "$T/policy.keep" > .ai/repo/policy.yaml
out="$(printf '{"session_id":"cc-1","source":"resume"}' | ./.claude/hooks/majordomus-session-start 2>/dev/null)"
[ -z "$out" ] || { echo "    briefing_on_start: false still wrote to stdout: $out"; exit 1; }
cp "$T/policy.keep" .ai/repo/policy.yaml

# The end event closes it into the shared record, and the outcome is read from the event
# rather than assumed.
out="$(printf '{"session_id":"cc-1","reason":"prompt_input_exit"}' | ./.claude/hooks/majordomus-session-end 2>"$T/err")"
[ -z "$out" ] || { echo "    the end hook wrote to stdout: $out"; exit 1; }
grep -qF 'closed' "$T/err"
[ "$(records)" = "$((was_records + 1))" ] || { echo "    the end event wrote no session record"; exit 1; }
expect_exit 0 "$MJ" session status
expect_grep 'No open session'
# the working context learned how the episode ended, in the same document
grep -qF '## Close' "$ctx"
grep -qF -- '- outcome: closed' "$ctx"

# An end event with nothing open is the normal case, not a failure: the episode may have been
# closed by hand, or never opened at all.
out="$(printf '{"session_id":"cc-1","reason":"clear"}' | ./.claude/hooks/majordomus-session-end 2>"$T/err")"
[ "$(records)" = "$((was_records + 1))" ] || { echo "    an end event with nothing open wrote a record"; exit 1; }
grep -qF 'nothing to close' "$T/err"

# A reason the adapter does not list is an episode cut short, because calling one complete is
# the worse of the two mistakes.
printf '{"session_id":"cc-2","source":"startup"}' | ./.claude/hooks/majordomus-session-start 2>/dev/null
printf '{"session_id":"cc-2","reason":"other"}' | ./.claude/hooks/majordomus-session-end 2>/dev/null
grep -qF 'outcome: interrupted' "$("$MJ" session latest --path)" \
  || { echo "    an unlisted end reason did not close the episode as interrupted"; exit 1; }

# An event whose payload says nothing still marks the boundary: refusing to open an episode
# over a detail no field of the record depends on would lose the episode.
printf '{}' | ./.claude/hooks/majordomus-session-start 2>/dev/null
expect_exit 0 "$MJ" session status
expect_grep 'Session: +s-'
"$MJ" session close >/dev/null
# and an unreadable one is logged rather than raised, because the person's session is not
# this command's to interrupt
printf 'not json at all' | ./.claude/hooks/majordomus-session-start 2>/dev/null
expect_exit 0 "$MJ" session status
grep -qF 'not understood' .ai/local/session-contexts/.session-context.log \
  || { echo "    an unreadable payload left no trace in the log"; ls -1a .ai/local/session-contexts; exit 1; }
"$MJ" session close >/dev/null
rm -f .ai/local/session-contexts/.session-context.log

# ---------------------------------------------------------------- doctor
# The repository declares the enforcement, so doctor is now answerable for it.
awk '{ print }
     /^enforcement:$/ { print "  - name: session-lifecycle"; print "    path: bin/majordomus"; print "    args: [capture, session]"; print "    wired_by: provider-hook:claude-code:session" }' \
  .ai/repo/policy.yaml > "$T/policy.yaml" && cp "$T/policy.yaml" .ai/repo/policy.yaml
grep -qF 'provider-hook:claude-code:session' .ai/repo/policy.yaml || { echo "    the enforcement probe did not take"; exit 1; }
"$MJ" update >/dev/null
mkdir -p .git/hooks
printf '#!/usr/bin/env bash\nmajordomus doctor\n' > .git/hooks/pre-commit
printf '#!/usr/bin/env bash\nmajordomus finish --check\n' > .git/hooks/pre-push
chmod +x .git/hooks/pre-commit .git/hooks/pre-push

"$MJ" doctor > "$T/doctor.out" 2>&1 || true
grep -qF 'session-lifecycle' "$T/doctor.out" || { echo "    doctor never reached the declared enforcement"; sed 's/^/    | /' "$T/doctor.out"; exit 1; }
grep -qE 'session-lifecycle.*(verified|synthetic)' "$T/doctor.out" || { echo "    doctor did not verify the wiring"; grep -F session-lifecycle "$T/doctor.out" | sed 's/^/    | /'; exit 1; }

# a hook the provider can no longer run is a failure of the repository, not a warning
chmod -x .claude/hooks/majordomus-session-start
expect_exit 10 "$MJ" doctor
expect_grep 'session-lifecycle'
chmod +x .claude/hooks/majordomus-session-start

# an aspect nobody declared is named rather than passed over
sed 's/provider-hook:claude-code:session/provider-hook:claude-code:nonsense/' .ai/repo/policy.yaml > "$T/p2" && cp "$T/p2" .ai/repo/policy.yaml
expect_exit 10 "$MJ" doctor
expect_grep 'unknown provider-hook aspect'

# ---------------------------------------------------------------- the other two events
# Everything above ran inside one episode and left it closed, so this opens its own: a
# compaction and an end are only observable against an episode that is running, and
# borrowing the one above would have left the assertions after it reading state this block
# had already changed.
checkpoints() { find .ai/local/state/checkpoints -maxdepth 1 -name '*.md' 2>/dev/null | wc -l | tr -d ' '; }
handovers()   { find .ai/local/state/handovers   -maxdepth 1 -name '*.md' 2>/dev/null | wc -l | tr -d ' '; }

"$MJ" start "compaction probe" --scope lib >/dev/null
printf '{"session_id":"cc-9","source":"startup"}' | ./.claude/hooks/majordomus-session-start >/dev/null 2>&1

# A compaction is not the end of an episode. It records what the conversation is about to
# stop holding, writes nothing to stdout, and leaves the episode open.
was_checkpoints="$(checkpoints)"
out="$(printf '{"session_id":"cc-9"}' | ./.claude/hooks/majordomus-session-compact 2>"$T/err")"
[ -z "$out" ] || { echo "    the compact hook wrote to stdout: $out"; exit 1; }
[ "$(checkpoints)" = "$((was_checkpoints + 1))" ] || { echo "    the compaction recorded no checkpoint"; sed 's/^/    | /' "$T/err"; exit 1; }
expect_exit 0 "$MJ" session status
expect_grep 'Session: +s-'          # still open: a compaction is not an ending
# what it wrote is a checkpoint, derived rather than typed, and within the policy's cap
c="$(find .ai/local/state/checkpoints -maxdepth 1 -name '*.md' | LC_ALL=C sort | tail -n 1)"
grep -qF 'Derived, not authored' "$c"
cap="$(awk '/^  max_body_lines:/ { print $2 }' .ai/repo/policy.yaml)"
body="$(awk 'c>=2{print} /^---$/{c++}' "$c" | wc -l | tr -d ' ')"
[ "$body" -le "$cap" ] || { echo "    the derived checkpoint is $body lines, cap $cap"; exit 1; }

# An episode that ends with its task still active leaves a continuation record, not only the
# envelope. The two answer different questions, and until now the next worker got one of them.
was_handovers="$(handovers)"
printf '{"session_id":"cc-9","reason":"clear"}' | ./.claude/hooks/majordomus-session-end >/dev/null 2>"$T/err"
[ "$(handovers)" = "$((was_handovers + 1))" ] || { echo "    the end event wrote no continuation record"; sed 's/^/    | /' "$T/err"; exit 1; }
grep -qF 'continuation written' "$T/err"
h="$(find .ai/local/state/handovers -maxdepth 1 -name '*.md' | LC_ALL=C sort | tail -n 1)"
grep -qF '# Objective' "$h"
grep -qF '# Current State' "$h"
grep -qF '# Next Action' "$h"
grep -qF 'no model wrote it' "$h"

# The next episode is handed that record, with the label that says how far to trust it. This
# is the whole point: nobody typed a command anywhere in this block.
"$MJ" start "resume probe" --scope lib >/dev/null
out="$(printf '{"session_id":"cc-10","source":"startup"}' | ./.claude/hooks/majordomus-session-start 2>/dev/null)"
printf '%s\n' "$out" | grep -qF 'Handover' || { echo "    the next episode was not handed the record:"; printf '%s\n' "$out" | sed 's/^/    | /'; exit 1; }
printf '%s\n' "$out" | grep -qE 'exact|advanced|diverged' || { echo "    the record arrived without a divergence label"; exit 1; }
printf '%s\n' "$out" | grep -qF 'Next Action' || { echo "    the section a resuming worker acts on did not travel"; exit 1; }

# The section arrives with its shape: blank lines inside it are what separate one paragraph
# from the next, and a lifter that drops them runs the whole thing together in the one part
# of the record the next worker is told to act on.
cat > "$T/two-paragraphs.md" <<'MD'
# Objective

Prove the section lifter keeps a paragraph break.

# Current State

Two paragraphs below.

# Next Action

The first thing to do.

The second thing to do.
MD
"$MJ" handover --no-task < "$T/two-paragraphs.md" >/dev/null
out="$(printf '{"session_id":"cc-11","source":"startup"}' | ./.claude/hooks/majordomus-session-start 2>/dev/null)"
printf '%s\n' "$out" | grep -qF 'The first thing to do.' || { echo "    the quoted section lost its first paragraph"; exit 1; }
printf '%s\n' "$out" | awk '/^The first thing to do\.$/ { got = 1; next } got && !NF { blank = 1 } got && /^The second thing to do\.$/ { exit blank ? 0 : 1 } END { if (!got) exit 1 }' \
  || { echo "    the paragraph break inside the quoted section was dropped"; printf '%s\n' "$out" | sed 's/^/    | /'; exit 1; }
