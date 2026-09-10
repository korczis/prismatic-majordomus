# majordomus-covers: capture session
# majordomus-negative: capture session
# Every provider draws its own boundary — not only the one this repository is developed in.
#
# Case 54 proves the episode boundary is real for Claude Code and case 111 proves that two
# windows of one provider are two episodes. Both were true of exactly one tool: `capture
# session --provider codex` answered `no lifecycle adapter` and exited 12, so Codex and
# Gemini CLI got no episode, no working context, no briefing and no record. The adapters are
# `share/providers.yaml` now, one `hooks` block per provider, and this proves the second and
# third of them all the way down: installed into the file the vendor reads, driven with the
# payload the vendor sends, into an episode of its own.
#
# Every assertion below drives the shim the provider would run, for the reason case 54 gives:
# a lifecycle that only works when a test calls the library proves nothing about a session
# that ended in a crash.
. "$ROOT/test/lib.sh"

# The suite may itself be running inside a provider session — this repository is developed in
# one — and a case that drives three named provider sessions must not silently be a fourth.
unset MAJORDOMUS_PROVIDER_SESSION CLAUDE_CODE_SESSION_ID GEMINI_SESSION_ID

"$MJ" init >/dev/null; "$MJ" update >/dev/null
git add .gitignore >/dev/null 2>&1; git commit -qm "ignore local ai state" >/dev/null 2>&1 || true
PATH="$(dirname "$MJ"):$PATH"; export PATH

must() { local why="$1"; shift; "$@" || { printf '    %s\n' "$why"; exit 1; }; }
OPEN=.ai/local/state/sessions-open
open_count() { find "$OPEN" -maxdepth 1 -name '*.yaml' 2>/dev/null | wc -l | tr -d ' '; }
records()    { find .ai/repo/sessions -maxdepth 1 -name '2*.md' 2>/dev/null | wc -l | tr -d ' '; }
sid_of()     { sed -n 's/^session_id: //p' "$1" | head -n 1; }

# ---------------------------------------------------------------- install
# One command writes what each provider needs, in the file that provider reads, in the
# dialect that provider reads it in. Two of the three are not Claude Code's, and one of them
# is not even JSON.
expect_exit 0 "$MJ" capture install

# Claude Code: unchanged, and case 54 owns the detail.
must "the Claude Code start shim was not written"   [ -x .claude/hooks/majordomus-session-start ]

# Gemini CLI: the same object shape in a file of its own, with its own project-directory
# variable and its own name for the compaction event.
must "the Gemini start shim was not written"        [ -x .gemini/hooks/majordomus-session-start ]
must "the Gemini end shim was not written"          [ -x .gemini/hooks/majordomus-session-end ]
must "the Gemini compaction shim was not written"   [ -x .gemini/hooks/majordomus-session-compact ]
grep -qF 'SessionStart' .gemini/settings.json
grep -qF 'PreCompress' .gemini/settings.json          # its own name for the same moment
grep -qF '${GEMINI_PROJECT_DIR}/.gemini/hooks/majordomus-session-start' .gemini/settings.json
expect_no_grep 'CLAUDE_PROJECT_DIR' .gemini/settings.json
# and it is the object shape that provider reads, not text that happens to name the events
grep -qF '"type": "command"' .gemini/settings.json
grep -qF '"matcher": ""' .gemini/settings.json

# Codex: TOML, an array of tables, and no project-directory variable at all — Codex
# substitutes nothing into a command string and runs it wherever the session was started, so
# the command has to find the repository itself.
must "the Codex start shim was not written"         [ -x .codex/hooks/majordomus-session-start ]
must "the Codex prompt shim was not written"        [ -x .codex/hooks/majordomus-capture ]
grep -qF '[[hooks.SessionStart]]' .codex/config.toml
grep -qF '[[hooks.SessionEnd]]' .codex/config.toml
grep -qF '[[hooks.UserPromptSubmit]]' .codex/config.toml
grep -qF '.codex/hooks/majordomus-session-start' .codex/config.toml
grep -qF 'git rev-parse --show-toplevel' .codex/config.toml
expect_no_grep 'PROJECT_DIR' .codex/config.toml
# the end event is clamped to three seconds by Codex, so the declaration asks for the most
# it will give rather than accepting the one-second default
grep -qF 'timeout = 3' .codex/config.toml

# The shim of every provider resolves the repository from its own location, never from a
# variable the provider happened to export.
for s in .gemini/hooks/majordomus-session-end .codex/hooks/majordomus-session-end; do
  grep -qF 'dirname -- "$0"' "$s" || { echo "    $s does not locate the repository from itself"; exit 1; }
done

# Running it again changes nothing, for every provider and not only the first.
expect_exit 0 "$MJ" capture install
expect_grep 'already'

# ---------------------------------------------------------------- the refusal is per file
# A configuration this tool did not write is not this tool's to rewrite — and that is now
# three files rather than one. The Gemini settings a person already keeps their MCP servers
# in is the ordinary case.
cp .gemini/settings.json "$T/gemini.keep"
printf '{"mcpServers":{"someone-elses":{"command":"x"}}}\n' > .gemini/settings.json
expect_exit 15 "$MJ" capture install
expect_grep 'not this tool'
expect_grep 'SessionStart'
grep -qF 'someone-elses' .gemini/settings.json           # refused means unchanged
expect_no_grep 'majordomus-session-start' .gemini/settings.json
cp "$T/gemini.keep" .gemini/settings.json

# ---------------------------------------------------------------- status, aspect by aspect
# Each provider has two aspects and they are two answers. Gemini has an episode boundary and
# no honest prompt event, which is a finding and reads as one; a reader asking about either
# never gets the other's answer.
expect_exit 0 "$MJ" capture status
expect_grep 'gemini:session +verified'
expect_grep 'codex:session +verified'
expect_grep 'claude-code:session +verified'
expect_grep 'codex +verified'
expect_grep 'gemini +unsupported'
expect_grep 'BeforeAgent carries the expanded text'      # the reason is the declaration's
# a provider with no hook at all says why, and the why is about that tool
expect_grep 'bb +unsupported'
expect_grep 'the agent it runs owns the hooks'
# and one provider's wiring is independent of another's
chmod -x .gemini/hooks/majordomus-session-end
expect_exit 0 "$MJ" capture status
expect_grep 'gemini:session +named'
expect_grep 'codex:session +verified'
expect_grep 'claude-code:session +verified'
chmod +x .gemini/hooks/majordomus-session-end

# ---------------------------------------------------------------- three providers, three episodes
# The payloads are the ones each vendor actually sends. Claude Code and Gemini both name the
# session `session_id` and the start source `source`; Codex adds the fields its own schema
# marks required, and its end event's `reason` is pinned by that schema to `other`.
cc_start()  { printf '{"session_id":"%s","source":"startup"}' "$1" | ./.claude/hooks/majordomus-session-start; }
gm_start()  { printf '{"session_id":"%s","source":"startup","cwd":"%s","hook_event_name":"SessionStart","timestamp":"2026-09-10T00:00:00Z"}' "$1" "$PWD" | ./.gemini/hooks/majordomus-session-start; }
gm_end()    { printf '{"session_id":"%s","reason":"%s","cwd":"%s","hook_event_name":"SessionEnd"}' "$1" "$2" "$PWD" | ./.gemini/hooks/majordomus-session-end; }
cx_start()  { printf '{"session_id":"%s","source":"startup","cwd":"%s","hook_event_name":"SessionStart","model":"m","permission_mode":"default","transcript_path":null}' "$1" "$PWD" | ./.codex/hooks/majordomus-session-start; }
cx_end()    { printf '{"session_id":"%s","reason":"other","cwd":"%s","hook_event_name":"SessionEnd","transcript_path":null}' "$1" "$PWD" | ./.codex/hooks/majordomus-session-end; }

cc_start cc-1 >/dev/null 2>&1
gm_start gm-1 >/dev/null 2>&1
cx_start cx-1 >/dev/null 2>&1
must "three providers did not open three episodes" [ "$(open_count)" = 3 ]
must "the Gemini episode is not keyed by its own provider session" [ -f "$OPEN/gm-1.yaml" ]
must "the Codex episode is not keyed by its own provider session"  [ -f "$OPEN/cx-1.yaml" ]
must "two providers were given one episode id" [ "$(sid_of "$OPEN/gm-1.yaml")" != "$(sid_of "$OPEN/cx-1.yaml")" ]

# The episode carries the provider that opened it, so a record can say whose sitting it was.
expect_grep '^provider: "gemini"$' "$OPEN/gm-1.yaml"
expect_grep '^provider: "codex"$' "$OPEN/cx-1.yaml"
expect_grep '^provider: "claude-code"$' "$OPEN/cc-1.yaml"

# ---------------------------------------------------------------- the briefing, in its envelope
# The document is the same one for every provider. What differs is how the provider takes it
# off standard output, and that is declared: Claude Code adds the text verbatim, Gemini and
# Codex parse one JSON object and read `hookSpecificOutput.additionalContext` out of it.
# Text that is not that object is not a briefing to them, and is dropped without saying so.
say() { printf '    %s\n' "$1"; printf '%s\n' "$out" | sed 's/^/    | /'; exit 1; }
out="$(gm_start gm-1 2>/dev/null)"
[ -n "$out" ] || { echo "    the Gemini start hook wrote no briefing to stdout"; exit 1; }
printf '%s' "$out" | grep -qF '{"hookSpecificOutput":{"hookEventName":"SessionStart","additionalContext":"' \
  || say "the Gemini briefing is not the object that provider reads:"
printf '%s' "$out" | grep -qF 'Majordomus' || say "the Gemini briefing does not name itself:"
# It is one JSON string, so every line break of the document is an escape inside it. A
# writer that dropped them would run the whole briefing together in the one field the
# provider reads, which is the failure nothing downstream could report.
printf '%s' "$out" | grep -qF '\n' || say "the briefing lost its line breaks:"
[ "$(printf '%s\n' "$out" | wc -l | tr -d ' ')" = 1 ] || say "the envelope is not one JSON object on one line:"
case "$out" in *'"}}') ;; *) say "the envelope is not closed:" ;; esac

# ...and Claude Code's is the text itself, unwrapped, exactly as case 54 asserts
out="$(cc_start cc-1 2>/dev/null)"
printf '%s' "$out" | grep -qF 'Majordomus' || say "the Claude Code briefing is missing:"
if printf '%s' "$out" | grep -qF 'hookSpecificOutput'; then
  say "Claude Code's briefing was wrapped in another provider's envelope:"
fi

# a start event fired again is a resume, for these providers too: one provider session, one
# episode
must "a resume opened a fourth episode" [ "$(open_count)" = 3 ]

# ---------------------------------------------------------------- each end closes its own
was="$(records)"
gm_end gm-1 exit >/dev/null 2>"$T/err"
must "the Gemini end event wrote no session record" [ "$(records)" = "$((was + 1))" ]
must "the Gemini end event closed more than its own episode" [ "$(open_count)" = 2 ]
must "the Gemini end event closed the Codex episode"      [ -f "$OPEN/cx-1.yaml" ]
must "the Gemini end event closed the Claude Code episode" [ -f "$OPEN/cc-1.yaml" ]
# `exit` is Gemini's ordinary ending and it is in its own list of deliberate ones
grep -qF 'outcome: closed' "$("$MJ" session latest --path)" \
  || { echo "    a deliberate Gemini ending was not recorded as closed"; exit 1; }

# Codex cannot say how a sitting ended — its end event's reason is pinned to the literal
# `other` — so every Codex episode is interrupted. That is the direction this design chose:
# calling a cut-short episode complete is the worse of the two mistakes.
cx_end cx-1 >/dev/null 2>"$T/err"
must "the Codex end event wrote no session record" [ "$(records)" = "$((was + 2))" ]
must "the Codex end event closed more than its own episode" [ "$(open_count)" = 1 ]
grep -qF 'outcome: interrupted' "$("$MJ" session latest --path)" \
  || { echo "    a Codex ending, which says nothing about itself, was recorded as complete"; exit 1; }

# An end event naming a provider session with no episode here closes nothing at all, for a
# second provider as for the first.
gm_end gm-nothing exit >/dev/null 2>"$T/err"
grep -qF 'nothing to close' "$T/err" || { echo "    an unknown Gemini session did not report absence"; sed 's/^/    | /' "$T/err"; exit 1; }
must "an unknown provider session closed something" [ "$(open_count)" = 1 ]

printf '{"session_id":"cc-1","reason":"clear"}' | ./.claude/hooks/majordomus-session-end >/dev/null 2>&1
must "the last episode was not closed" [ "$(open_count)" = 0 ]

# ---------------------------------------------------------------- a prompt Codex captured
# Codex's UserPromptSubmit is the one prompt event of the three that carries what the person
# typed with a per-prompt identity beside it, so it is the one the second provider gets an
# adapter for. Gemini's does not, which is why it has none.
prompts() { find .ai/local/prompts -maxdepth 1 -name '*.json' 2>/dev/null | wc -l | tr -d ' '; }
was_prompts="$(prompts)"
printf '{"session_id":"cx-9","turn_id":"t-1","prompt":"what Codex was actually asked","cwd":"%s","hook_event_name":"UserPromptSubmit","model":"m","permission_mode":"default","transcript_path":null}' "$PWD" \
  | ./.codex/hooks/majordomus-capture
must "the Codex prompt hook wrote no record" [ "$(prompts)" = "$((was_prompts + 1))" ]
f="$(find .ai/local/prompts -maxdepth 1 -name '*.json' | sort | tail -n 1)"
grep -qF '"provider": "codex"' "$f"
grep -qF '"event": "UserPromptSubmit"' "$f"
grep -qF 'what Codex was actually asked' "$f"
# the per-prompt identity is `turn_id` here and `prompt_id` in Claude Code; the record
# carries whichever the provider sent, under the one name the archive uses
grep -qF '"id": "t-1"' "$f"
# and the same hook delivered twice is one prompt, not two
printf '{"session_id":"cx-9","turn_id":"t-1","prompt":"what Codex was actually asked","cwd":"%s","hook_event_name":"UserPromptSubmit","model":"m","permission_mode":"default","transcript_path":null}' "$PWD" \
  | ./.codex/hooks/majordomus-capture
must "a redelivered Codex prompt became a second record" [ "$(prompts)" = "$((was_prompts + 1))" ]
# nothing was lost on the way
if [ -s .ai/local/prompts/.capture.log ]; then
  echo "    a capture failed:"; sed 's/^/    | /' .ai/local/prompts/.capture.log; exit 1
fi

# ---------------------------------------------------------------- the declaration is the adapter
# Adding a provider is an entry in share/providers.yaml and nothing else. The proof that the
# shell holds no provider's own facts is that no configuration file, event name or payload
# key of a provider appears in it.
for lit in '.claude/settings.json' '.gemini/settings.json' '.codex/config.toml' \
           'SessionStart' 'SessionEnd' 'PreCompact' 'PreCompress' 'UserPromptSubmit' \
           'BeforeAgent' 'CLAUDE_PROJECT_DIR' 'GEMINI_PROJECT_DIR' \
           'prompt_id' 'session_id' 'prompt_source' 'turn_id' 'prompt_input_exit'; do
  # comments are exempt: prose may name the event a paragraph is about, and does
  if grep -v '^[[:space:]]*#' "$ROOT/lib/capture.sh" | grep -qF -- "$lit"; then
    echo "    lib/capture.sh carries '$lit', a fact about one provider; it belongs in share/providers.yaml"
    exit 1
  fi
done
grep -qF 'hooks:' "$ROOT/share/providers.yaml" || { echo "    share/providers.yaml declares no hooks"; exit 1; }

# ...and the other direction: take an event out of the declaration and it stops existing.
# All three providers happen to have all three events, so the only way to prove that the
# events are the declaration's and not the shell's is to publish a distribution with one
# fewer. A share under no working tree is an installed distribution, which is what the
# variable is for.
cp -R "$ROOT/share" "$T/alt-share"
grep -v '^          compact: PreCompress$' "$ROOT/share/providers.yaml" > "$T/alt-share/providers.yaml"
if grep -qE '^ +compact: PreCompress$' "$T/alt-share/providers.yaml"; then
  echo "    the probe distribution still declares Gemini's compaction event"; exit 1
fi
export MAJORDOMUS_SHARE="$T/alt-share"
# the kind is still one of this tool's three...
expect_exit 12 "$MJ" capture session --provider gemini --event compact
expect_grep 'fires no compact event'
expect_grep 'it has start end'
# ...and the one provider that lost it is the only one that lost it
expect_exit 12 "$MJ" capture session --provider codex --event nonsense
expect_grep 'must be one of start end compact'
unset MAJORDOMUS_SHARE
expect_exit 0 "$MJ" capture status
expect_grep 'gemini:session +verified'
exit 0
