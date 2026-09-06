# majordomus-covers: capture
# majordomus-negative: capture doctor
# Prompt capture: the surface, the five states a provider can be in, what the shim actually
# writes — a record and the rendering of it — and the failures doctor must go red on.
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
# the record is a pretty-printed object: one member per line, in the declared order
[ "$(sed -n '1p' "$rec")" = '{' ] || { echo "    a record does not open with {"; head -n 2 "$rec" | sed 's/^/    | /'; exit 1; }
sed -n '2p' "$rec" | grep -qxF '  "schema": "majordomus.prompt/v1",' \
  || { echo "    a record does not name its schema on the second line"; sed -n '2p' "$rec" | sed 's/^/    | /'; exit 1; }
[ "$(tail -n 1 "$rec")" = '}' ] || { echo "    a record does not close with }"; exit 1; }
sed -n '3p' "$rec" | grep -q '^  "started_at": ' || { echo "    a record does not carry started_at"; sed -n '3p' "$rec" | sed 's/^/    | /'; exit 1; }
# and it is still valid JSON, which one line of it never proved
python3 -c 'import json,sys; json.load(open(sys.argv[1]))' "$rec" \
  || { echo "    a record is not valid JSON"; sed 's/^/    | /' "$rec"; exit 1; }
# nothing the hook cannot observe is present as null: the turn had not run
expect_no_grep '"(finished_at|duration_ms|model|effort|tokens|output)"' "$rec"
# the text is the raw span from the payload: an embedded quote and an escape survive byte
# for byte, because nothing decodes and re-encodes them
grep -qxF '  "text": "he said \"no\"\nand left"' "$rec" || { echo "    the prompt text did not survive"; sed 's/^/    | /' "$rec"; exit 1; }
grep -qF '"schema": "majordomus.prompt/v1"' "$rec"
# the model's half of the exchange has no field to arrive in
expect_no_grep '"(response|completion|transcript)":' "$rec"

# ---------------------------------------------------------------- both formats, one stem
# The record is for the machine and the rendering is for the person, and neither is optional:
# a capture that wrote one of them wrote half an archive. The rendering is asserted against
# the same payload as the record above, so the decode is proved and not assumed — the raw
# span keeps \" and \n, and the body must show a quote and a line break.
md="${rec%.json}.md"
[ -f "$md" ] || { echo "    the record has no Markdown rendering beside it"; ls -1 .ai/local/prompts; exit 1; }
[ "$(find .ai/local/prompts -maxdepth 1 -name '*.md' | wc -l | tr -d ' ')" = 1 ] \
  || { echo "    one prompt did not write exactly one rendering"; ls -1 .ai/local/prompts; exit 1; }
head -n 1 "$md" | grep -qx -- '---' || { echo "    the rendering does not open with front matter"; head -n 3 "$md" | sed 's/^/    | /'; exit 1; }
grep -qF "schema: 'majordomus.prompt/v1'" "$md"
grep -qF "record: '$(basename "$rec")'" "$md" || { echo "    the rendering does not name the record it was built from"; sed 's/^/    | /' "$md"; exit 1; }
grep -qF "provider: 'claude-code'" "$md"
# the same fields again as a table, because front matter is what a tool reads and a table
# is what a person reads
grep -qF '| **Provider** | `claude-code` · `UserPromptSubmit` |' "$md" \
  || { echo "    the rendering has no readable metadata table"; sed 's/^/    | /' "$md"; exit 1; }
grep -qxF '# Prompt — 2026' "$md" >/dev/null 2>&1 || grep -q '^# Prompt — ' "$md" \
  || { echo "    the rendering has no heading"; sed 's/^/    | /' "$md"; exit 1; }

# the prompt comes last, under its own heading, fenced, and decoded: the escape became a
# real line break and the quote is a quote
grep -qx '## PROMPT' "$md" || { echo "    the prompt is not under ## PROMPT"; sed 's/^/    | /' "$md"; exit 1; }
sed -n '/^## PROMPT$/,$p' "$md" | sed '1,3d' | sed '$d' > "$T/body"   # between the fences
grep -qxF 'he said "no"' "$T/body" || { echo "    the prompt was not decoded into the block"; sed 's/^/    | /' "$T/body"; exit 1; }
grep -qxF 'and left' "$T/body" || { echo "    the escaped newline did not become a line break"; sed 's/^/    | /' "$T/body"; exit 1; }
# and the rendering carries the person's half only, exactly as the record does
expect_no_grep '"(response|completion|transcript)":' "$md"

# a prompt that quotes a fence must not end the block early: the fence is sized to what it
# has to hold, which is the only way to quote a prompt verbatim without touching a byte
fencedpayload='{"prompt":"here is a block:\n````\n```\ninside\n```\n````\ndone","prompt_id":"pf"}'
printf '%s' "$fencedpayload" | ./.claude/hooks/majordomus-capture
fenced="$(find .ai/local/prompts -maxdepth 1 -name '*-here-is-a-block*.md')"
[ -n "$fenced" ] || { echo "    the fenced prompt was not captured"; ls -1 .ai/local/prompts; exit 1; }
grep -qx '`````' "$fenced" || { echo "    the fence was not grown past the prompt's own"; sed 's/^/    | /' "$fenced"; exit 1; }
[ "$(grep -cx '`````' "$fenced")" = 2 ] || { echo "    the block does not open and close exactly once"; exit 1; }
grep -qxF 'inside' "$fenced" || { echo "    the nested block did not survive"; exit 1; }
rm -f "$fenced" "${fenced%.md}.json"

# a rendering is rebuilt from its record, and only from its record: render writes no record,
# leaves a rendering that is already there alone, and is the repair the doctrine names
rm -f "$md"
expect_exit 0 "$MJ" capture render
expect_grep '1 rendering'
[ -f "$md" ] || { echo "    capture render did not rebuild the missing rendering"; exit 1; }
[ "$(records)" = 1 ] || { echo "    capture render wrote a record"; exit 1; }
expect_exit 0 "$MJ" capture render
expect_grep '0 rendering'
expect_exit 2 "$MJ" capture render --nonsense
expect_grep 'unknown option'

# idempotent on the provider's own prompt identity: a hook delivered twice writes once
printf '%s' "$payload" | ./.claude/hooks/majordomus-capture
[ "$(records)" = 1 ] || { echo "    a repeated prompt_id wrote a second record"; ls -1 .ai/local/prompts; exit 1; }
# a different prompt is a different record, and the slug follows the new text
second='{"session_id":"s1","prompt_id":"p2","cwd":"/w","hook_event_name":"UserPromptSubmit","prompt_source":"user","prompt_text":"why is the archive empty"}'
printf '%s' "$second" | ./.claude/hooks/majordomus-capture
[ "$(records)" = 2 ] || { echo "    a new prompt_id did not write a record"; exit 1; }
[ -n "$(find .ai/local/prompts -maxdepth 1 -name '*-why-is-the-archive-empty.json')" ] \
  || { echo "    the slug does not follow the prompt text"; ls -1 .ai/local/prompts; exit 1; }
[ -n "$(find .ai/local/prompts -maxdepth 1 -name '*-why-is-the-archive-empty.md')" ] \
  || { echo "    the second prompt wrote no rendering"; ls -1 .ai/local/prompts; exit 1; }

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
# written the way an older version wrote them: one line, and `ts` before the field had a
# sibling called finished_at. They must reformat rather than be reported forever.
for d in 20010101000000 20010102000000 20010103000000; do
  printf '{"schema":"majordomus.prompt/v1","ts":"%s","provider":"claude-code","event":"UserPromptSubmit","id":"%s","session":null,"source":null,"cwd":null,"repository":"r","branch":"b","head":"h","text":"an older prompt"}\n' \
    "$d" "$d" > ".ai/local/prompts/$d-old.json"
done
before="$(records)"
printf '%s' "$second" | sed 's/"p2"/"p3"/' | ./.claude/hooks/majordomus-capture
[ "$(records)" = "$((before + 1))" ] || { echo "    capturing changed the number of earlier records"; ls -1 .ai/local/prompts; exit 1; }
for d in 20010101000000 20010102000000 20010103000000; do
  [ -f ".ai/local/prompts/$d-old.json" ] || { echo "    $d-old.json was deleted"; exit 1; }
done
expect_no_grep '^prompts:' .ai/repo/policy.yaml   # there is no cap on prompts to configure

# ---------------------------------------------------------------- the pair is enforced
# The archive above now holds records planted by hand, which have no rendering. That is the
# state the doctrine is for, so it is asserted here before it is repaired.
expect_exit 0 "$MJ" capture render
expect_grep '3 rendering'
# reformatted in place, and the old field name carried to the new one rather than lost
grep -qxF '  "started_at": "20010101000000",' .ai/local/prompts/20010101000000-old.json \
  || { echo "    a record written on one line under the old name was not reformatted"; sed 's/^/    | /' .ai/local/prompts/20010101000000-old.json; exit 1; }
expect_no_grep '"ts":' .ai/local/prompts/20010101000000-old.json

# ---------------------------------------------------------------- the schema is a path
# The identifier in a record is the location of the file that describes it, so an archive
# is self-describing without a registry that could fall out of step with it.
# both halves, each in the language that fits it: the record is JSON and gets JSON Schema,
# the document is a shape and gets protobuf
for ext in schema.json proto; do
  [ -f "$ROOT/share/schemas/majordomus/prompt/v1.$ext" ] \
    || { echo "    majordomus.prompt/v1 names no v1.$ext at the path it derives"; exit 1; }
done
grep -qF '| **Schema** | `majordomus.prompt/v1` · `share/schemas/majordomus/prompt/v1.{schema.json,proto}` |' "$md" \
  || { echo "    the rendering does not name the files describing its schema"; grep -i schema "$md" | sed 's/^/    | /'; exit 1; }
"$MJ" doctor > "$T/doctor.out" 2>&1 || true
grep -qF 'at the path the identifier derives' "$T/doctor.out" \
  || { echo "    doctor does not check that the schema resolves"; exit 1; }

# a record carrying what the hook could not observe renders those rows, and only then
full=.ai/local/prompts/20010105000000-with-a-finished-turn.json
printf '{\n  "schema": "majordomus.prompt/v1",\n  "started_at": "2026-01-05T10:00:00Z",\n  "provider": "claude-code",\n  "event": "UserPromptSubmit",\n  "id": "pf1",\n  "session": null,\n  "source": null,\n  "cwd": null,\n  "repository": "r",\n  "branch": "b",\n  "head": "h",\n  "text": "a finished turn",\n  "finished_at": "2026-01-05T10:03:20Z",\n  "duration_ms": 200000,\n  "model": "claude-opus-5",\n  "effort": "high"\n}\n' > "$full"
expect_exit 0 "$MJ" capture render
grep -qF '| **Finished** | `2026-01-05T10:03:20Z` |' "${full%.json}.md" || { echo "    an observed finish did not render"; exit 1; }
grep -qF '| **Duration** | `3m 20s` |' "${full%.json}.md" || { echo "    a duration did not render as one"; grep -i duration "${full%.json}.md" | sed 's/^/    | /'; exit 1; }
grep -qF '| **Model** | `claude-opus-5` · `high` |' "${full%.json}.md" || { echo "    the model and effort did not render"; exit 1; }
# ...while the record the hook wrote carries none of them, and shows none
expect_no_grep '\*\*Finished\*\*' "$md"
expect_no_grep '\*\*Duration\*\*' "$md"
expect_no_grep '\*\*Model\*\*' "$md"
rm -f "$full" "${full%.json}.md"

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

# a record with no rendering is half an archive, and the repair is named where it is found
rm -f .ai/local/prompts/*-he-said-no*.md
expect_exit 10 "$MJ" doctor
expect_grep 'no Markdown rendering'
expect_grep 'majordomus capture render'
expect_exit 0 "$MJ" capture render
"$MJ" doctor > "$T/doctor.out" 2>&1 || true
grep -qF 'every prompt is present as both a record and a rendering' "$T/doctor.out" \
  || { echo "    doctor does not report a complete archive as complete"; grep -i capture "$T/doctor.out" | sed 's/^/    | /'; exit 1; }

# the opposite half is the unrepairable one: nothing can rebuild a record from a rendering,
# so it is reported for a person to remove and render must not pretend to fix it
: > .ai/local/prompts/20010104000000-orphan.md
expect_exit 10 "$MJ" doctor
expect_grep 'no record and cannot be rebuilt'
expect_exit 0 "$MJ" capture render
[ -f .ai/local/prompts/20010104000000-orphan.md ] || { echo "    render deleted an orphan rendering"; exit 1; }
[ ! -f .ai/local/prompts/20010104000000-orphan.json ] || { echo "    render invented a record from a rendering"; exit 1; }
rm -f .ai/local/prompts/20010104000000-orphan.md

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
printf '{\n  "schema": "majordomus.prompt/v1",\n  "started_at": "t",\n  "provider": "claude-code",\n  "event": "e",\n  "id": "p8",\n  "session": null,\n  "source": null,\n  "cwd": null,\n  "repository": "r",\n  "branch": "b",\n  "head": "h",\n  "response": "what the model said"\n}\n' \
  > .ai/local/prompts/20260101000000-planted.json
expect_exit 10 "$MJ" doctor
expect_grep "model's half"
