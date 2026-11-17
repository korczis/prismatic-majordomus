# A capability is invoked by name from Claude Code, through a skill generated from its
# declaration (ADR 0146).
#
# `/intent <sentence>` is `intents.propose` because the capability says `.skill("intent",
# "statement")`, and for no other reason: no file names the pair, and the skill Claude Code
# reads is written by `majordomus generate editors` from the registry.
#
# What is held here, through the built executable, in a repository that is not this one:
#
#   1  a fresh repository gets `.claude/skills/intent/SKILL.md` from `generate editors`, with
#      the front matter Claude Code requires, the banner after it, the capability's id, its MCP
#      tool and its command line, and the typed text placed exactly once;
#   2  a writer is a person's to invoke: `disable-model-invocation: true`;
#   3  generation is idempotent, and `generate --check` fails on a tampered skill;
#   4  a hand-written skill at the projected path is refused, never overwritten;
#   5  `editors.status` reports the claude-code adapter and the file's derivation;
#   6  the registry carries the alias on the capability, so every projection reads one fact.
#
# The run with the installed client is not repeated here: it needs a signed-in Claude Code.
# It is recorded in ADR 0146, and `MJ_CLAUDE_E2E=1` runs it where a client is available.
#
# Skips itself when there is neither cargo nor MAJORDOMUS_BIN, as every Rust case does.
. "$ROOT/test/lib.sh"
command -v jq >/dev/null 2>&1 || skip "no jq"
RB="$(rust_bin)" || rust_bin_exit $?
[ -x "$RB" ] || { echo "    the build produced no executable at $RB"; exit 1; }
MAJORDOMUS_SHARE="$ROOT/share"; export MAJORDOMUS_SHARE
S="$(mktemp -d "${TMPDIR:-/tmp}/mj2980.XXXXXX")"; trap 'rm -rf "$S"' EXIT
MJ="$ROOT/bin/majordomus"
SKILL=.claude/skills/intent/SKILL.md

fail=0
note() { printf '    %s\n' "$1"; fail=1; }

"$MJ" init >/dev/null 2>&1
git add -A >/dev/null 2>&1
git commit -qm "fixture" >/dev/null 2>&1

# ---------------------------------------------------------------- 1. the generated skill
"$RB" generate editors >"$S/gen.out" 2>"$S/gen.err" || { note "generate editors failed:"; sed 's/^/    | /' "$S/gen.err" | tail -5; }
[ -f "$SKILL" ] || { echo "    generate editors wrote no $SKILL"; exit 1; }
[ "$(sed -n 1p "$SKILL")" = "---" ] || note "the skill does not open with its front matter"
[ "$(sed -n 2p "$SKILL")" = "name: intent" ] || note "the skill's name is not its alias: $(sed -n 2p "$SKILL")"
grep -q '^description: "' "$SKILL" || note "the skill has no description"
grep -qx 'argument-hint: "<statement>"' "$SKILL" || note "the skill does not hint the field the text fills"
end="$(awk 'NR > 1 && $0 == "---" { print NR; exit }' "$SKILL")"
[ -n "$end" ] && sed -n "$((end + 1))p" "$SKILL" | grep -q '^<!-- GENERATED FILE' \
  || note "the banner is not the first line after the front matter"
grep -q 'capability `intents.propose`' "$SKILL" || note "the skill does not name its capability"
grep -q 'the tool `majordomus_intent_propose`' "$SKILL" || note "the skill does not name the MCP tool"
grep -q '`majordomus intent propose`' "$SKILL" || note "the skill does not name the command line"
[ "$(grep -c 'ARGUMENTS' "$SKILL")" = 1 ] || note "the typed text is placed $(grep -c 'ARGUMENTS' "$SKILL") time(s), not once"

# ---------------------------------------------------------------- 2. a writer is a person's
sed -n "1,${end}p" "$SKILL" | grep -qx 'disable-model-invocation: true' \
  || note "a capability that writes the repository may be invoked by the model"
grep -q 'do not try again by another way' "$SKILL" || note "the skill does not forbid retrying a writer another way"

# ---------------------------------------------------------------- 3. idempotent, and checked
before="$(cksum < "$SKILL")"
"$RB" generate editors >/dev/null 2>&1 || note "a second generate failed"
[ "$(cksum < "$SKILL")" = "$before" ] || note "a second generate changed the skill"
cp "$SKILL" "$S/skill.orig"
printf '\nCall it twice.\n' >> "$SKILL"
st=0; "$RB" generate editors --check >"$S/check.out" 2>&1 || st=$?
[ "$st" = 10 ] || note "generate --check exits $st on a tampered skill, not 10"
grep -q "$SKILL" "$S/check.out" || note "generate --check does not name the stale skill"
"$RB" generate editors >/dev/null 2>&1 || note "generate did not restore a tampered skill of its own"
cmp -s "$SKILL" "$S/skill.orig" || note "the tampered skill was not rewritten to what the registry says"

# ---------------------------------------------------------------- 4. a person's file is theirs
printf -- '---\nname: intent\ndescription: "mine"\n---\n\nMy own command.\n' > "$SKILL"
st=0; "$RB" generate editors >"$S/own.out" 2>&1 || st=$?
[ "$st" != 0 ] || note "generate overwrote a hand-written skill"
grep -q 'My own command.' "$SKILL" || note "the hand-written skill was replaced"
cp "$S/skill.orig" "$SKILL"

# ---------------------------------------------------------------- 5. the projection explains itself
{ printf '{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2025-06-18","capabilities":{},"clientInfo":{"name":"case2980","version":"0"}}}\n'
  printf '{"jsonrpc":"2.0","method":"notifications/initialized"}\n'
  printf '{"jsonrpc":"2.0","id":2,"method":"tools/call","params":{"name":"majordomus_editors","arguments":{}}}\n'
} | "$RB" mcp 2>"$S/mcp.err" | tail -n 1 | jq '.result.structuredContent' > "$S/status.json" \
  || note "editors.status could not be asked over MCP"
jq -e '[.adapters[] | select(.id == "claude-code")] | length == 1' "$S/status.json" >/dev/null \
  || note "editors.status reports no claude-code adapter"
jq -e --arg p "$SKILL" '[.artifacts[] | select(.path == $p and (.adapters | index("claude-code")) and (.sources | index("capability:intents.propose")) and .state == "current")] | length == 1' "$S/status.json" >/dev/null \
  || note "editors.status does not derive $SKILL from intents.propose for claude-code, current"

# ---------------------------------------------------------------- 6. one fact, on the descriptor
"$RB" capabilities describe intents.propose --format json >"$S/cap.json" 2>/dev/null \
  || note "capabilities describe intents.propose failed"
jq -e '.. | objects | select(has("skill")) | .skill | (.alias == "intent" and .argument == "statement")' "$S/cap.json" >/dev/null \
  || note "the descriptor of intents.propose does not carry its skill"

# ---------------------------------------------------------------- the installed client, on request
if [ "${MJ_CLAUDE_E2E:-0}" = 1 ]; then
  command -v claude >/dev/null 2>&1 || { echo "    MJ_CLAUDE_E2E=1 and there is no claude on PATH"; exit 1; }
  mkdir "$S/bin"
  printf '#!/bin/sh\nMAJORDOMUS_SHARE=%s exec %s "$@"\n' "$ROOT/share" "$RB" > "$S/bin/majordomus"; chmod +x "$S/bin/majordomus"
  printf '{"mcpServers":{"majordomus":{"command":"%s","args":["mcp","--standalone","--repo","%s"]}}}\n' "$S/bin/majordomus" "$PWD" > "$S/mcp.json"
  # its own server and no other: without --strict-mcp-config the client may reach a server of
  # another repository under the same name, and write there
  PATH="$S/bin:$PATH" timeout 300 claude -p '/intent Every page answers within 100 ms when warm' \
    --mcp-config "$S/mcp.json" --strict-mcp-config \
    --allowedTools mcp__majordomus__majordomus_intent_propose >"$S/claude.out" 2>&1 \
    || note "the client failed: $(tail -3 "$S/claude.out" | tr '\n' ' ')"
  f=.ai/repo/project/intents/every-page-answers-within-100-ms-when-warm.yaml
  [ -f "$f" ] || note "the client's /intent wrote no record at $f"
  [ "$(sed -n 's/^statement: //p' "$f" 2>/dev/null | jq -r .)" = "Every page answers within 100 ms when warm" ] \
    || note "the statement the client passed is not the text that was typed"
fi

[ "$fail" = 0 ] || exit 1
echo "    /intent is generated from intents.propose: a skill a person invokes, written once, checked, never over a hand-written file, and explained by editors.status"
