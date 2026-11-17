# An intent is proposed by a capability, and its stage stays derived (ADR 0144).
#
# Until this case every intent was written by hand: six capabilities read intents and none
# created one, so nothing that speaks to the registry — an MCP client, an HTTP caller, a
# generated skill — could say what must become true. `intents.propose` is the one writer.
#
# What is held here, through the built executable:
#
#   1  the command line and the MCP tool reach the same capability and write the same shape
#      of record: the statement verbatim, no milestone, no criterion, no invariant;
#   2  a proposal is valid: its stage derives as `declared`, `intent validate` exits 0 and
#      reports the two absences as warnings;
#   3  a half-planned intent — a milestone and no criterion — still fails, exit 10;
#   4  a refusal writes nothing: an identity that exists, an empty statement, an id that is
#      not a slug, a field the input does not have;
#   5  nothing about a proposal's standing is stored: the record carries no stage or status.
#
# Skips itself when there is neither cargo nor MAJORDOMUS_BIN, as every Rust case does.
. "$ROOT/test/lib.sh"
command -v jq >/dev/null 2>&1 || skip "no jq"
RB="$(rust_bin)" || rust_bin_exit $?
[ -x "$RB" ] || { echo "    the build produced no executable at $RB"; exit 1; }
MAJORDOMUS_SHARE="$ROOT/share"; export MAJORDOMUS_SHARE
S="$(mktemp -d "${TMPDIR:-/tmp}/mj2978.XXXXXX")"; trap 'rm -rf "$S"' EXIT
MJ="$ROOT/bin/majordomus"
DIR=.ai/repo/project/intents

# One MCP tool call over the real pipes, as case 133 makes it: the tool's own verdict is
# isError, not the process's status.
tool() { # <tool> <arguments-json> -> structuredContent on stdout, isError in $TOOL_ERR
  local name="$1" args="${2:-{\}}" out="$S/frames"
  {
    printf '{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2025-06-18","capabilities":{},"clientInfo":{"name":"case2978","version":"0"}}}\n'
    printf '{"jsonrpc":"2.0","method":"notifications/initialized"}\n'
    printf '{"jsonrpc":"2.0","id":2,"method":"tools/call","params":{"name":"%s","arguments":%s}}\n' "$name" "$args"
  } | "$RB" mcp 2>"$S/mcp.err" > "$out" || {
    printf '    the server failed on %s:\n' "$name" >&2; sed 's/^/    | /' "$S/mcp.err" >&2; return 1; }
  TOOL_ERR="$(tail -n 1 "$out" | jq -r '.result.isError')"
  TOOL_TEXT="$(tail -n 1 "$out" | jq -r '.result.content[0].text // ""')"
  tail -n 1 "$out" | jq '.result.structuredContent'
}

fail=0
note() { printf '    %s\n' "$1"; fail=1; }

"$MJ" init >/dev/null 2>&1
pj_init
pj_milestone M000 0
git add -A >/dev/null 2>&1
git commit -qm "fixture" >/dev/null 2>&1

# ---------------------------------------------------------------- 1. two surfaces, one shape
"$RB" intent propose --format json \
  --statement 'The "UI": answers within 100 ms # warm' > "$S/cli.json" 2>"$S/cli.err" \
  || { note "the command line refused a proposal:"; sed 's/^/    | /' "$S/cli.err"; }
[ "$(jq -r .id < "$S/cli.json")" = the-ui-answers-within-100-ms-warm ] \
  || note "the command line named the intent $(jq -r .id < "$S/cli.json")"
tool majordomus_intent_propose \
  '{"statement":"Every peer sees a change without replaying it","title":"Peers see changes","id":"peers-see-changes"}' \
  > "$S/mcp.json" || note "the MCP tool failed"
[ "$TOOL_ERR" = false ] || note "the MCP tool refused a proposal: $TOOL_TEXT"
[ "$(jq -r .path < "$S/mcp.json")" = "$DIR/peers-see-changes.yaml" ] \
  || note "the MCP tool wrote $(jq -r .path < "$S/mcp.json")"

for id in the-ui-answers-within-100-ms-warm peers-see-changes; do
  f="$DIR/$id.yaml"
  [ -f "$f" ] || { note "no record at $f"; continue; }
  keys="$(sed -E 's/^([a-z_]+):.*/\1/' "$f" | tr '\n' ' ')"
  [ "$keys" = "id title statement invariants milestones satisfaction created_at updated_at " ] \
    || note "$f carries the keys [$keys]"
  for empty in invariants milestones satisfaction; do
    grep -qx "$empty: \[\]" "$f" || note "$f does not leave $empty empty"
  done
done
# the statement survives its quotes, its colon and its hash, verbatim
got="$(sed -n 's/^statement: //p' "$DIR/the-ui-answers-within-100-ms-warm.yaml" | jq -r .)"
[ "$got" = 'The "UI": answers within 100 ms # warm' ] || note "the statement reads back as: $got"
# nothing is left beside the records
[ "$(find "$DIR" -name '.*.tmp.*' | wc -l | tr -d ' ')" = 0 ] || note "a temporary file was left in $DIR"

# ---------------------------------------------------------------- 2. a proposal is valid
git add -A >/dev/null 2>&1
st=0; "$RB" intent validate --format json > "$S/valid.json" 2>/dev/null || st=$?
[ "$st" = 0 ] || note "intent validate exits $st on a repository that holds only proposals"
[ "$(jq -r .failures < "$S/valid.json")" = 0 ] || note "a proposal is a failure: $(jq -c '[.findings[]|select(.level=="FAIL")|.code]' < "$S/valid.json")"
for code in intent_without_milestone intent_without_criterion; do
  n="$(jq --arg c "$code" '[.findings[]|select(.code==$c and .level=="WARN")]|length' < "$S/valid.json")"
  [ "$n" = 2 ] || note "$code is a warning for $n of the 2 proposals"
done
[ "$("$RB" intent show peers-see-changes --format json 2>/dev/null | jq -r .stage)" = declared ] \
  || note "a proposal's stage is not declared"

# ---------------------------------------------------------------- 3. half a plan still fails
sed 's/^milestones: \[\]$/milestones: [M000]/' "$DIR/peers-see-changes.yaml" > "$S/half.yaml"
cp "$S/half.yaml" "$DIR/peers-see-changes.yaml"
st=0; "$RB" intent validate --format json > "$S/half.json" 2>/dev/null || st=$?
[ "$st" = 10 ] || note "an intent with a milestone and no criterion exits $st, not 10"
[ "$(jq '[.findings[]|select(.code=="intent_without_criterion" and .level=="FAIL")]|length' < "$S/half.json")" = 1 ] \
  || note "the half-planned intent's missing criterion is not a failure"
git checkout -q -- "$DIR/peers-see-changes.yaml" 2>/dev/null || git restore --staged --worktree "$DIR/peers-see-changes.yaml"

# ---------------------------------------------------------------- 4. a refusal writes nothing
before="$(cat "$DIR"/*.yaml | cksum) $(find "$DIR" | wc -l | tr -d ' ')"
refused() { # <label> <arguments-json> <expected substring>
  tool majordomus_intent_propose "$2" > /dev/null 2>&1
  [ "$TOOL_ERR" = true ] || { note "$1: the capability allowed it"; return; }
  case "$TOOL_TEXT" in *"$3"*) ;; *) note "$1: refused, but said '$TOOL_TEXT'" ;; esac
}
refused "an identity that exists" '{"statement":"something else","id":"peers-see-changes"}' "never overwrites"
refused "an empty statement"      '{"statement":"   "}' "needs a statement"
refused "an id that is no slug"   '{"statement":"s","id":"Not A Slug"}' "not an intent identity"
refused "a field it does not have" '{"statement":"s","stage":"satisfied"}' "stage"
st=0; "$RB" intent propose --statement "again" --id peers-see-changes >/dev/null 2>&1 || st=$?
[ "$st" != 0 ] || note "the command line overwrote an existing intent"
[ "$(cat "$DIR"/*.yaml | cksum) $(find "$DIR" | wc -l | tr -d ' ')" = "$before" ] \
  || note "a refused proposal changed the intents directory"

# ---------------------------------------------------------------- 5. no standing is stored
grep -lE '^(stage|status|state|verdict):' "$DIR"/*.yaml > "$S/stored" 2>/dev/null || true
[ ! -s "$S/stored" ] || note "a record stores its own standing: $(tr '\n' ' ' < "$S/stored")"

[ "$fail" = 0 ] || exit 1
echo "    one capability proposes: the command line and MCP write the same unplanned record, it is valid and declared, half a plan still fails, and a refusal writes nothing"
