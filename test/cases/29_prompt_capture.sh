# majordomus-covers: capture
# majordomus-negative: capture doctor
# Prompt capture: the surface, the four states a provider can be in, what the shim actually
# writes, and the failures doctor must go red on.
#
# The point of this case is that "wired" is decided by running the hook. Every state
# assertion below follows a real mutation of the thing the state is about — the shim's
# executable bit, the configuration's contents, a record's fields — because a verifier that
# survives a broken hook proves nothing, and a hook is exactly the kind of thing that
# breaks silently.
. "$ROOT/test/lib.sh"
"$MJ" init >/dev/null; "$MJ" update >/dev/null
mkdir -p lib && echo a > lib/a && git add . && git commit -qm base
# how a repository that is not the tool's own checkout reaches majordomus
PATH="$(dirname "$MJ"):$PATH"; export PATH

# ---------------------------------------------------------------- surface
expect_exit 2 "$MJ" capture
expect_grep 'usage: majordomus capture prompt'
expect_exit 2 "$MJ" capture nonsense
expect_grep 'unknown subcommand'

expect_exit 0 "$MJ" capture status
expect_grep 'claude-code +unconfigured'

# A provider with no adapter is named as such, never assumed to be silent. The exit code is
# 12 and not 2: this command runs inside a provider hook, where 2 rejects the person's own
# prompt, so it is never the answer however wrong the invocation is.
expect_exit 12 "$MJ" capture prompt --provider codex
expect_grep 'no adapter'
expect_grep 'claude-code'
expect_exit 12 "$MJ" capture prompt
expect_grep 'provider is required'

# ---------------------------------------------------------------- install
expect_exit 0 "$MJ" capture install
[ -x .claude/hooks/majordomus-capture ]
grep -qF 'UserPromptSubmit' .claude/settings.json
grep -qF '"matcher": ""' .claude/settings.json
grep -qF '${CLAUDE_PROJECT_DIR}/.claude/hooks/majordomus-capture' .claude/settings.json
# the shim resolves the repository from its own location, not from the environment: the
# provider substitutes its project directory textually and exports nothing
grep -qF 'dirname -- "$0"' .claude/hooks/majordomus-capture
expect_no_grep 'CLAUDE_PROJECT_DIR' .claude/hooks/majordomus-capture

# installing twice changes nothing
expect_exit 0 "$MJ" capture install
expect_grep 'already'

# and a configuration this tool did not write is not this tool's to rewrite
cp .claude/settings.json "$T/settings.keep"
printf '{"hooks":{"UserPromptSubmit":[]}}\n' > .claude/settings.json
expect_exit 15 "$MJ" capture install
expect_grep 'not this tool'
expect_grep 'matcher'
grep -qF '"UserPromptSubmit":[]' .claude/settings.json      # refused means unchanged
cp "$T/settings.keep" .claude/settings.json

# ---------------------------------------------------------------- the state is run, not read
expect_exit 0 "$MJ" capture status
expect_grep 'claude-code +verified'
chmod -x .claude/hooks/majordomus-capture
expect_exit 0 "$MJ" capture status
expect_grep 'claude-code +named'
expect_grep 'not executable'
chmod +x .claude/hooks/majordomus-capture
expect_exit 0 "$MJ" capture status
expect_grep 'claude-code +verified'

# ---------------------------------------------------------------- what the hook writes
records() { find .ai/local/prompts -maxdepth 1 -name '*.json' | wc -l | tr -d ' '; }
payload='{"session_id":"s1","prompt_id":"p1","cwd":"/w","hook_event_name":"UserPromptSubmit","prompt_source":"user","prompt_text":"he said \"no\"\nand left"}'
printf '%s' "$payload" | ./.claude/hooks/majordomus-capture
[ "$(records)" = 1 ] || { echo "    expected one record file"; ls -1 .ai/local/prompts; exit 1; }

# one prompt is one file, named for the second it arrived in and then for its own opening,
# so a directory listing reads as what was asked and when
rec="$(find .ai/local/prompts -maxdepth 1 -name '*.json')"
case "$(basename "$rec")" in
  [0-9][0-9][0-9][0-9][0-9][0-9][0-9][0-9][0-9][0-9][0-9][0-9][0-9][0-9]-he-said-no*.json) ;;
  *) echo "    the record is not named <timestamp>-<slug>.json: $(basename "$rec")"; exit 1 ;;
esac
[ "$(grep -c . "$rec")" = 1 ] || { echo "    a record is more than one line"; exit 1; }
# the text is the raw span from the payload: an embedded quote and an escape survive byte
# for byte, because nothing decodes and re-encodes them
grep -qF ',"text":"he said \"no\"\nand left"}' "$rec" || { echo "    the prompt text did not survive"; sed 's/^/    | /' "$rec"; exit 1; }
grep -qF '"schema":"majordomus.prompt/v1"' "$rec"
# the model's half of the exchange has no field to arrive in
expect_no_grep '"(response|completion|transcript)":' "$rec"

# idempotent on the provider's own prompt identity: a hook delivered twice writes once
printf '%s' "$payload" | ./.claude/hooks/majordomus-capture
[ "$(records)" = 1 ] || { echo "    a repeated prompt_id wrote a second record"; ls -1 .ai/local/prompts; exit 1; }
# a different prompt is a different record, and the slug follows the new text
second='{"session_id":"s1","prompt_id":"p2","cwd":"/w","hook_event_name":"UserPromptSubmit","prompt_source":"user","prompt_text":"why is the archive empty"}'
printf '%s' "$second" | ./.claude/hooks/majordomus-capture
[ "$(records)" = 2 ] || { echo "    a new prompt_id did not write a record"; exit 1; }
[ -n "$(find .ai/local/prompts -maxdepth 1 -name '*-why-is-the-archive-empty.json')" ] \
  || { echo "    the slug does not follow the prompt text"; ls -1 .ai/local/prompts; exit 1; }

# ---------------------------------------------------------------- it never costs a prompt
printf 'not json at all' | ./.claude/hooks/majordomus-capture
[ "$(records)" = 2 ] || { echo "    an unreadable payload wrote a record"; exit 1; }
grep -qF 'not understood' .ai/local/prompts/.capture.log
printf '{"prompt_id":"p9","surprise":"a field nobody declared"}' | ./.claude/hooks/majordomus-capture
[ "$(records)" = 2 ] || { echo "    a payload with no prompt text wrote a record"; exit 1; }
grep -qF 'carries none of' .ai/local/prompts/.capture.log
# a diagnostic that does not say what it saw is a dead end: the next field name has to be
# readable off the log rather than guessed
grep -qF 'surprise' .ai/local/prompts/.capture.log || { echo "    the log does not name the keys the payload did carry"; cat .ai/local/prompts/.capture.log; exit 1; }

# a provider that renames the field it sends the prompt in does not stop capture: the
# adapter names candidates, and the older name is still one of them
printf '{"prompt":"an older field name","prompt_id":"p10"}' | ./.claude/hooks/majordomus-capture
[ "$(records)" = 3 ] || { echo "    the fallback payload key did not capture"; ls -1 .ai/local/prompts; exit 1; }
[ -n "$(find .ai/local/prompts -maxdepth 1 -name '*-an-older-field-name.json')" ] \
  || { echo "    the fallback capture is not named for its text"; exit 1; }

# ---------------------------------------------------------------- not everything is a prompt
# The event is not what its name suggests: the provider also fires it for messages it
# injects into the turn. Those are not the person's prompts, and a skip is normal operation,
# so it leaves no record and — unlike a failure — no log line either.
logsize() { wc -c < .ai/local/prompts/.capture.log 2>/dev/null | tr -d ' ' || echo 0; }
logwas="$(logsize)"
was="$(records)"
printf '{"prompt":"<task-notification>\n<status>completed</status>\n</task-notification>","prompt_id":"n1"}' | ./.claude/hooks/majordomus-capture
[ "$(records)" = "$was" ] || { echo "    an injected task notification was captured as a prompt"; exit 1; }
printf '{"prompt":"injected by something","prompt_id":"n2","prompt_source":"hook"}' | ./.claude/hooks/majordomus-capture
[ "$(records)" = "$was" ] || { echo "    a payload whose origin is not a person was captured"; exit 1; }
[ "$(logsize)" = "$logwas" ] || { echo "    a skip was logged as a failure"; tail -n 2 .ai/local/prompts/.capture.log; exit 1; }
# ...and a payload that says nothing about its origin is still captured, because losing a
# real prompt is the worse of the two mistakes
printf '{"prompt":"no origin declared","prompt_id":"n3"}' | ./.claude/hooks/majordomus-capture
[ "$(records)" = "$((was + 1))" ] || { echo "    a payload with no declared origin was dropped"; exit 1; }

# ---------------------------------------------------------------- nothing is ever deleted
# The ledger and the handovers rotate under a policy cap; a prompt does not, because no
# other file can reconstruct one. Capturing must therefore leave every earlier record where
# it was, and there must be no cap to reach.
for d in 20010101000000 20010102000000 20010103000000; do : > ".ai/local/prompts/$d-old.json"; done
before="$(records)"
printf '%s' "$second" | sed 's/"p2"/"p3"/' | ./.claude/hooks/majordomus-capture
[ "$(records)" = "$((before + 1))" ] || { echo "    capturing changed the number of earlier records"; ls -1 .ai/local/prompts; exit 1; }
for d in 20010101000000 20010102000000 20010103000000; do
  [ -f ".ai/local/prompts/$d-old.json" ] || { echo "    $d-old.json was deleted"; exit 1; }
done
expect_no_grep '^prompts:' .ai/repo/policy.yaml   # there is no cap on prompts to configure

# ---------------------------------------------------------------- doctor
# The repository declares the enforcement, so doctor is now answerable for it. The other two
# enforcements the skeleton declares must be in place first, or their absence masks this one.
awk '{ print }
     /^enforcement:$/ { print "  - name: prompt-capture"; print "    path: bin/majordomus"; print "    args: [capture, prompt]"; print "    wired_by: provider-hook:claude-code" }' \
  .ai/repo/policy.yaml > "$T/policy.yaml" && cp "$T/policy.yaml" .ai/repo/policy.yaml
grep -qF 'provider-hook:claude-code' .ai/repo/policy.yaml || { echo "    the enforcement probe did not take"; exit 1; }
"$MJ" update >/dev/null
mkdir -p .git/hooks
printf '#!/usr/bin/env bash\nmajordomus doctor\n' > .git/hooks/pre-commit
printf '#!/usr/bin/env bash\nmajordomus finish --check\n' > .git/hooks/pre-push
chmod +x .git/hooks/pre-commit .git/hooks/pre-push

# A capture that failed is the guarantee broken, and the log is the only trace it leaves:
# the hook still ran, and the self test still passes because it sends a payload of the
# tool's own making. The failures provoked above must therefore make doctor red.
expect_exit 10 "$MJ" doctor
expect_grep 'capture\(s\) failed'
expect_grep 'prompts were lost'
rm -f .ai/local/prompts/.capture.log

"$MJ" doctor > "$T/doctor.out" 2>&1 || true
grep -qF 'prompt-capture' "$T/doctor.out" || { echo "    doctor never reached the declared enforcement"; sed 's/^/    | /' "$T/doctor.out"; exit 1; }
grep -qE 'prompt-capture.*(verified|synthetic)' "$T/doctor.out" || { echo "    doctor did not verify the wiring"; grep -F prompt-capture "$T/doctor.out" | sed 's/^/    | /'; exit 1; }

# a hook the provider can no longer run is a failure of the repository, not a warning
chmod -x .claude/hooks/majordomus-capture
expect_exit 10 "$MJ" doctor
expect_grep 'prompt-capture'
chmod +x .claude/hooks/majordomus-capture

# a captured prompt that reached the index is a failure: the archive never leaves the machine
one="$(find .ai/local/prompts -maxdepth 1 -name '*.json' | head -n 1)"
git add -f "$one"
expect_exit 10 "$MJ" doctor
expect_grep 'tracked by git'
git rm --cached -q "$one"

# and a record carrying the model's half of the exchange is a failure of the same doctrine
printf '{"schema":"majordomus.prompt/v1","ts":"t","provider":"claude-code","event":"e","id":"p8","session":null,"source":null,"cwd":null,"repository":"r","branch":"b","head":"h","response":"what the model said"}\n' \
  > .ai/local/prompts/20260101000000-planted.json
expect_exit 10 "$MJ" doctor
expect_grep "model's half"
