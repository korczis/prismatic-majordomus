# majordomus-covers: history plan
# claim: a ledger line names the process that wrote it, its pid, the directory it ran in and
# the mesh session it works for, in both writers and in one declared order (I4561)
#
# On 2026-11-07 a line in another repository's ledger recorded `projections.updated` with no
# session and nothing else about its writer; two sessions spent an hour excluding suspects by
# timing and never found it. The envelope now carries `pid`, `cwd` and, when the process
# names one, `mesh_session` (MAJORDOMUS_OWNER_SESSION, else claude-code-$CLAUDE_CODE_SESSION_ID,
# the rule `mesh who` reads an owner by). Four parts:
#   1. the shell (mj_ledger_append) and the executable (ledger::append) each write pid and
#      the physical directory they ran in, and no mesh_session when none is named;
#   2. with a session named, both lines carry it;
#   3. both compose the envelope in the order MJ_LEDGER_ENVELOPE_KEYS declares;
#   4. a payload naming pid, cwd or mesh_session is refused, and `history` prints the writer.
. "$ROOT/test/lib.sh"
command -v jq >/dev/null 2>&1 || skip "no jq"
RB="$(rust_bin)" || rust_bin_exit $?
[ -x "$RB" ] || { echo "    the build produced no executable at $RB"; exit 1; }
MAJORDOMUS_SHARE="$ROOT/share"; export MAJORDOMUS_SHARE

fail=0
note() { printf '    %s\n' "$1"; fail=1; }

"$MJ" init >/dev/null 2>&1
pj_init
pj_milestone M000 0
for i in I0001 I0002 I0003 I0004; do pj_issue "$i" M000; done
mkdir -p sub/dir
git add -A >/dev/null 2>&1
git commit -qm fixture >/dev/null 2>&1
REPO="$(pwd -P)"
L="$REPO/.ai/local/state/ledger.jsonl"

capability() { # <issue> [env...] — the executable's plan transition, run from sub/dir
  local issue="$1"; shift
  ( cd sub/dir && printf '%s\n' \
      '{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2025-06-18","capabilities":{},"clientInfo":{"name":"case4570","version":"0"}}}' \
      '{"jsonrpc":"2.0","method":"notifications/initialized"}' \
      "{\"jsonrpc\":\"2.0\",\"id\":2,\"method\":\"tools/call\",\"params\":{\"name\":\"majordomus_plan_transition\",\"arguments\":{\"issue\":\"$issue\",\"transition\":\"start\"}}}" \
    | env "$@" "$RB" mcp --repo "$REPO" >/dev/null 2>&1 )
}
last() { grep "\"issue\":\"$1\"" "$L" | tail -n 1; }

# ---------------------------------------------------------------- 1. no session named
( cd sub/dir && "$MJ" plan start I0001 >/dev/null 2>&1 ) || note "the shell refused to start I0001"
capability I0002 || note "the capability failed to start I0002"
for i in I0001 I0002; do
  line="$(last "$i")"
  [ -n "$line" ] || { note "$i: no ledger line"; continue; }
  pid="$(printf '%s' "$line" | jq -r '.pid // empty')"
  case "$pid" in ''|*[!0-9]*) note "$i: the line names no pid: $line" ;; esac
  [ "$(printf '%s' "$line" | jq -r '.cwd // empty')" = "$REPO/sub/dir" ] \
    || note "$i: cwd is not the directory the writer ran in ($REPO/sub/dir): $line"
  [ "$(printf '%s' "$line" | jq -r 'has("mesh_session")')" = false ] \
    || note "$i: a mesh_session with none named: $line"
done

# ---------------------------------------------------------------- 2. a session named
( cd sub/dir && CLAUDE_CODE_SESSION_ID=s-4570 "$MJ" plan start I0003 >/dev/null 2>&1 ) \
  || note "the shell refused to start I0003"
capability I0004 MAJORDOMUS_OWNER_SESSION=owner-4570 CLAUDE_CODE_SESSION_ID=s-4570 \
  || note "the capability failed to start I0004"
[ "$(last I0003 | jq -r '.mesh_session // empty')" = claude-code-s-4570 ] \
  || note "the shell did not name the Claude Code session: $(last I0003)"
[ "$(last I0004 | jq -r '.mesh_session // empty')" = owner-4570 ] \
  || note "the executable did not prefer the stated owner: $(last I0004)"

# ---------------------------------------------------------------- 3. one declared order
declared="$(sed -n "s/^MJ_LEDGER_ENVELOPE_KEYS='\(.*\)'$/\1/p" "$ROOT/lib/common.sh")"
[ -n "$declared" ] || note "lib/common.sh declares no MJ_LEDGER_ENVELOPE_KEYS"
for i in I0001 I0002 I0003 I0004; do
  keys="$(last "$i" | jq -r 'keys_unsorted | join(" ")')"
  want=""; for k in $declared; do case " $keys " in *" $k "*) want="$want $k" ;; esac; done
  [ "$keys" = "${want# } issue" ] || note "$i composes [$keys], the declared order is [${want# } issue]"
done

# ---------------------------------------------------------------- 4. refused, and printed
probe="$T/probe.sh"
for key in pid cwd mesh_session; do
  cat > "$probe" <<PROBE
MJ_BIN_DIR="$ROOT/bin"; MJ_LIB_DIR="$ROOT/lib"; export MJ_BIN_DIR MJ_LIB_DIR
. "$ROOT/lib/common.sh"
mj_require_installed
mj_ledger_append question.opened '"task_id":"x","question":"q","$key":"forged"'
PROBE
  got=0; out="$(bash "$probe" 2>&1)" || got=$?
  [ "$got" = 13 ] || note "a payload field named $key was accepted (exit $got, expected 13)"
  case "$out" in *"'$key'"*"envelope key"*) ;; *) note "the refusal of $key did not name it: $out" ;; esac
done
grep -q '"forged"' "$L" && note "a refused payload reached the ledger"
hist="$("$MJ" history --event plan_start 2>&1)"
printf '%s\n' "$hist" | grep -q "pid=[0-9][0-9]* mesh=owner-4570" \
  || note "history does not print the writer: $hist"

exit "$fail"
