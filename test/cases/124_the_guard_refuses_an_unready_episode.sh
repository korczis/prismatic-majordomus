# majordomus-covers: capture
# The guard refuses an unready episode — and refuses nothing else.
#
# Entering as an agent converges on a ready shared server, and then nothing asks again: a
# server that dies, is killed or goes outdated halfway through an episode changes nothing
# the worker can see. The pre-tool event asks again, before each mutation, and it is the
# only provider event that may answer no.
#
# Every step drives the shim the provider would run, with the payload shape the provider
# sends, in a repository `init` wrote. Both directions are proved with the same weight,
# because the switch is off by default and the off path is therefore the one every session
# in this repository actually runs: it is the path that must never block anything.
. "$ROOT/test/lib.sh"
RB="$(rust_bin)" || rust_bin_exit $?
"$MJ" init >/dev/null; "$MJ" update >/dev/null
mkdir -p lib && echo a > lib/a && git add . && git commit -qm base
expect_exit 0 "$MJ" capture install              # a fresh repository: it writes the config
PATH="$(dirname "$MJ"):$PATH"; export PATH
MAJORDOMUS_SHARE="$ROOT/share"; export MAJORDOMUS_SHARE
trap '"$RB" serve stop --repo "$T" >/dev/null 2>&1 || true' EXIT

GUARD=./.claude/hooks/majordomus-session-guard
cache=.ai/local/state/guard/verdict
lease=.ai/local/state/mcp/server.json

# The payload the provider sends before a tool runs. One function, so that every assertion
# below is about the guard and not about a hand-written JSON object.
payload() { printf '{"session_id":"%s","hook_event_name":"PreToolUse","tool_name":"%s","tool_input":{"file_path":"%s/lib/a"},"cwd":"%s"}' "${2:-cc-1}" "$1" "$T" "$T"; }
guard() { payload "$@" | MAJORDOMUS_BIN="${SPY:-$RB}" $GUARD; }

# ---------------------------------------------------------------- it is wired as data
# The provider table declares the event, its shim and its matcher; the configuration the
# tool writes names all four events. A guard nobody wired is prose with an exit code.
expect_file "$GUARD"
[ -x "$GUARD" ] || { echo "    the guard shim is not executable"; exit 1; }
expect_grep 'capture guard' "$GUARD"
expect_grep 'PreToolUse' .claude/settings.json
expect_grep 'majordomus-session-guard' .claude/settings.json
expect_grep '"matcher": "[A-Za-z|]*MultiEdit' .claude/settings.json
# and `capture status` reconciles it through the lifecycle aspect, not a second mechanism
expect_exit 0 "$MJ" capture status
expect_grep 'claude-code:session'

# ---------------------------------------------------------------- 1. the switch is off
# This is what every session in this repository runs today. Nothing is refused, whatever
# the server is doing — and there is no server here at all.
[ ! -f "$lease" ] || { echo "    a fresh repository has a lease before anything started"; exit 1; }
expect_grep '^  guard_before_mutation: false' .ai/repo/policy.yaml
for tool in Edit Write MultiEdit NotebookEdit Bash Read Grep Task; do
  guard "$tool" > "$T/off.out" 2>&1 || { echo "    the guard refused $tool with the switch off:"; cat "$T/off.out"; exit 1; }
done
expect_file "$cache"
grep -q '^off ' "$cache" || { echo "    the switch being off was not remembered:"; cat "$cache"; exit 1; }

# a payload that will not parse, one that names no tool, one that is empty, and one that is
# not a payload at all: none of them may cost somebody a tool call
for bad in '{"session_id":"cc-1","tool_name":' '' 'null' '{"tool_input":{}}' '[]' '{"tool_name":"Edit"' 'not json at all'; do
  printf '%s' "$bad" | MAJORDOMUS_BIN="$RB" $GUARD >/dev/null 2>&1 \
    || { echo "    a malformed payload blocked a tool call: <$bad>"; exit 1; }
done

# ---------------------------------------------------------------- 2. the switch is on
sed 's/^  guard_before_mutation: false /  guard_before_mutation: true  /' .ai/repo/policy.yaml > "$T/pol" && cp "$T/pol" .ai/repo/policy.yaml
expect_grep '^  guard_before_mutation: true' .ai/repo/policy.yaml
rm -f "$cache"

# --- no episode is open: the start event never ran, so nothing written now belongs to one
"$RB" serve ensure --repo "$T" --idle 60 >/dev/null 2>&1 || true
[ -f "$lease" ] || { echo "    serve ensure started nothing"; cat .ai/local/state/mcp/*.log 2>/dev/null; exit 1; }
guard Edit > "$T/noep.out" 2>&1 && { echo "    an episode that was never opened was not refused:"; cat "$T/noep.out"; exit 1; }
code=$?; [ "$code" = 2 ] || { echo "    expected exit 2 for an unopened episode, got $code"; cat "$T/noep.out"; exit 1; }
grep -q 'no episode is open' "$T/noep.out" || { echo "    the refusal does not name the cause:"; cat "$T/noep.out"; exit 1; }
[ ! -f "$cache" ] || { echo "    a refusal was remembered; it must be decided again every time"; exit 1; }

# --- a ready episode passes: the start event opens one, the server is already serving
printf '{"session_id":"cc-1","hook_event_name":"SessionStart","source":"startup"}' | ./.claude/hooks/majordomus-session-start >/dev/null 2>&1
guard Edit > "$T/ready.out" 2>&1 || { echo "    a ready episode was refused:"; cat "$T/ready.out"; exit 1; }
expect_file "$cache"
grep -q '^pass ' "$cache" || { echo "    a ready episode was not remembered as a pass:"; cat "$cache"; exit 1; }

# --- a tool that does not mutate the repository is never refused, ready or not
rm -f "$cache"
for tool in Read Grep Glob WebFetch Task; do
  guard "$tool" >/dev/null 2>&1 || { echo "    $tool was refused, and it mutates nothing"; exit 1; }
done
[ ! -f "$cache" ] || { echo "    a tool that mutates nothing was decided and remembered"; exit 1; }

# ---------------------------------------------------------------- 3. the warm path
# A remembered pass is answered without asking the executable anything. Proved by counting:
# the executable is replaced by a spy that records every call and delegates, so a second
# guard that probed would leave a second line.
SPY="$T/spy"; export SPY
{ printf '#!/bin/sh\n'; printf 'printf "%%s\\n" "$*" >> "%s/spy.log"\n' "$T"; printf 'exec "%s" "$@"\n' "$RB"; } > "$SPY"
chmod +x "$SPY"
rm -f "$T/spy.log" "$cache"
guard Edit >/dev/null 2>&1 || { echo "    the cold reading refused a ready episode"; exit 1; }
[ -f "$T/spy.log" ] || { echo "    the cold reading never asked the executable anything"; exit 1; }
grep -q 'serve status' "$T/spy.log" || { echo "    the cold reading did not read 'serve status':"; cat "$T/spy.log"; exit 1; }
cold="$(wc -l < "$T/spy.log" | tr -d ' ')"
for _ in 1 2 3 4 5; do guard Edit >/dev/null 2>&1 || { echo "    a warm call refused"; exit 1; }; done
warm="$(wc -l < "$T/spy.log" | tr -d ' ')"
[ "$warm" = "$cold" ] || { echo "    five warm calls asked the executable $((warm - cold)) more time(s); the remembered answer is not being read"; cat "$T/spy.log"; exit 1; }
unset SPY

# ---------------------------------------------------------------- 4. the server goes away
# Killed under a working episode, lease and all. `serve ensure` is asked once to put it
# back, so this is a self-heal and not a refusal — and the assertion says which.
pid="$(sed -n 's/.*"pid"[[:space:]]*:[[:space:]]*\([0-9]*\).*/\1/p' "$lease" | head -n 1)"
[ -n "$pid" ] || { echo "    the lease names no process"; cat "$lease"; exit 1; }
kill -9 "$pid" 2>/dev/null || true
sleep 1
rm -f "$cache"
guard Edit > "$T/killed.out" 2>&1 || { echo "    a killed server was refused rather than healed:"; cat "$T/killed.out"; cat .ai/local/state/mcp/ensure.log 2>/dev/null; exit 1; }
newpid="$(sed -n 's/.*"pid"[[:space:]]*:[[:space:]]*\([0-9]*\).*/\1/p' "$lease" | head -n 1)"
[ -n "$newpid" ] && [ "$newpid" != "$pid" ] || { echo "    the guard exited 0 without a server behind it (pid $pid -> ${newpid:-none})"; exit 1; }
echo "    a server killed under the episode was healed by the guard, not refused (pid $pid -> $newpid)"

# ---------------------------------------------------------------- 5. and when it cannot be healed
# The same state, with an executable that reports it and cannot fix it: that is the one
# thing the guard is allowed to refuse, and it names the standing it refused on.
STUB="$T/stub"
{ printf '#!/bin/sh\n'
  printf 'case "$*" in\n'
  printf '  *"serve status"*) echo "{\\"standing\\": \\"stale\\"}"; exit 0 ;;\n'
  printf '  *"serve ensure"*) exit 0 ;;\n'
  printf 'esac\n'
  printf 'exit 0\n'
} > "$STUB"
chmod +x "$STUB"
rm -f "$cache"
payload Edit | MAJORDOMUS_BIN="$STUB" $GUARD > "$T/stale.out" 2>&1 && { echo "    a stale server that ensure could not fix was not refused:"; cat "$T/stale.out"; exit 1; }
code=$?; [ "$code" = 2 ] || { echo "    expected exit 2 for a stale server, got $code"; cat "$T/stale.out"; exit 1; }
grep -q "stale" "$T/stale.out" || { echo "    the refusal does not name the standing:"; cat "$T/stale.out"; exit 1; }
grep -q 'guard_before_mutation' "$T/stale.out" || { echo "    the refusal does not name the switch:"; cat "$T/stale.out"; exit 1; }
[ ! -f "$cache" ] || { echo "    a refusal was remembered"; exit 1; }

# ---------------------------------------------------------------- 6. what it cannot decide
# An executable that is not there is not a reason to stop somebody editing: it is this tool
# failing to decide, and it says so where the model reads it.
rm -f "$cache"
payload Edit | MAJORDOMUS_BIN="$T/nowhere" $GUARD > "$T/nobin.out" 2>&1 \
  || { echo "    a missing executable blocked a tool call:"; cat "$T/nobin.out"; exit 1; }
grep -q 'not built' "$T/nobin.out" || { echo "    a missing executable was not named:"; cat "$T/nobin.out"; exit 1; }
grep -q 'not refused' "$T/nobin.out" || { echo "    it did not say that nothing was decided:"; cat "$T/nobin.out"; exit 1; }
[ ! -f "$cache" ] || { echo "    an undecidable state was remembered"; exit 1; }

# ---------------------------------------------------------------- 7. off again, at once
# Turning the switch back off takes effect on the next call and not when something expires:
# a remembered answer is invalidated by the policy file being newer than it. So a `pass` is
# established first, and the very next call after the edit must have decided again.
rm -f "$cache"
guard Edit >/dev/null 2>&1 || { echo "    a ready episode was refused before the switch was moved"; exit 1; }
grep -q '^pass ' "$cache" || { echo "    expected a remembered pass to invalidate:"; cat "$cache"; exit 1; }
sed 's/^  guard_before_mutation: true /  guard_before_mutation: false /' .ai/repo/policy.yaml > "$T/pol2" && cp "$T/pol2" .ai/repo/policy.yaml
guard Edit > "$T/off2.out" 2>&1 || { echo "    the guard refused right after the switch went off:"; cat "$T/off2.out"; exit 1; }
grep -q '^off ' "$cache" || { echo "    the remembered pass outlived the policy that made it:"; cat "$cache"; exit 1; }

# and with the switch off nothing is refused even with no server at all
"$RB" serve stop --repo "$T" >/dev/null 2>&1 || true
guard Edit > "$T/off3.out" 2>&1 || { echo "    the guard refused with the switch off and no server at all:"; cat "$T/off3.out"; exit 1; }

# ---------------------------------------------------------------- 8. the gate of the rule
# Everything above is behaviour. This is the half a script can decide: that each piece of
# the wiring, planted broken in a fixture tree, is refused by the gate with the cause named.
# A gate that cannot be shown to fail is decoration.
GATE="$ROOT/scripts/ci/guard-refuses-unready"
[ -x "$GATE" ] || { echo "    scripts/ci/guard-refuses-unready is missing or not executable"; exit 1; }
expect_grep 'id: guard-refuses-unready' "$ROOT/.ai/repo/ci/gates.yaml"
expect_grep 'scripts/ci/guard-refuses-unready' "$ROOT/.ai/repo/ci/gates.yaml"
RULE="$ROOT/.ai/repo/rules/project/the-guard-refuses-an-unready-episode.v1.md"
expect_grep '^id: project\.the-guard-refuses-an-unready-episode' "$RULE"
expect_grep '^class: blocking' "$RULE"
expect_grep 'scripts/ci/guard-refuses-unready' "$RULE"

FX="$T/fixture"
for p in lib/capture.sh .claude/settings.json .claude/hooks/majordomus-session-guard \
         .ai/repo/policy.yaml share/skeleton/policy.yaml share/allow/policy.txt \
         share/schemas/majordomus/policy/policy.v1.schema.json; do
  [ -f "$ROOT/$p" ] || { echo "    the checkout has no $p; the gate reads it"; exit 1; }
  mkdir -p "$FX/$(dirname "$p")"
  cp "$ROOT/$p" "$FX/$p"
done
chmod +x "$FX/.claude/hooks/majordomus-session-guard"
gate() { MJ_ROOT="$FX" "$GATE" --no-suite; }

expect_exit 0 gate
expect_no_grep '^FAIL'

save() { cp "$FX/$1" "$T/pristine"; }
restore() { cp "$T/pristine" "$FX/$1"; [ "$1" = ".claude/hooks/majordomus-session-guard" ] && chmod +x "$FX/$1"; return 0; }

# --- the guard removed from the adapter table: every other gate stays green
save lib/capture.sh
sed "s/^guard 14 15 16 guard refusing'\$/'/" "$T/pristine" > "$FX/lib/capture.sh"
expect_exit 10 gate
expect_grep 'no row for the guard'
restore lib/capture.sh

# --- the matcher narrowed until a mutating tool is outside it
sed "s/Edit|Write|MultiEdit|NotebookEdit|Bash/Edit|Write/" "$T/pristine" > "$FX/lib/capture.sh"
expect_exit 10 gate
expect_grep 'the matcher does not cover Bash'
restore lib/capture.sh

# --- the guard carrying its own list rather than reading the provider's
sed 's/mj_lifecycle_matcher/mj_guard_own_list/g' "$T/pristine" > "$FX/lib/capture.sh"
expect_exit 10 gate
expect_grep 'two lists'
restore lib/capture.sh

# --- the decision no longer reading the switch
sed 's/session\.guard_before_mutation/session.something_else/' "$T/pristine" > "$FX/lib/capture.sh"
expect_exit 10 gate
expect_grep 'declared and unread'
restore lib/capture.sh

# --- the PreToolUse row removed from the wired configuration
save .claude/settings.json
sed 's/PreToolUse/PostToolUse/' "$T/pristine" > "$FX/.claude/settings.json"
expect_exit 10 gate
expect_grep 'declares no PreToolUse hook'
restore .claude/settings.json

# --- the matcher of the wired row narrowed
sed 's/"Edit|Write|MultiEdit|NotebookEdit|Bash"/"Edit"/' "$T/pristine" > "$FX/.claude/settings.json"
expect_exit 10 gate
expect_grep 'covers Bash'
restore .claude/settings.json

# --- the shim no longer delegating to the tool
save .claude/hooks/majordomus-session-guard
sed 's/capture guard/capture prompt/' "$T/pristine" > "$FX/.claude/hooks/majordomus-session-guard"
expect_exit 10 gate
expect_grep "does not dispatch 'capture guard'"
restore .claude/hooks/majordomus-session-guard

# --- the shim handing the process over, so every exit code it can have reaches the provider
sed 's|^"\$mj" --repo "\$root" capture guard|exec "$mj" --repo "$root" capture guard|' "$T/pristine" > "$FX/.claude/hooks/majordomus-session-guard"
expect_exit 10 gate
expect_grep 'execs the tool'
restore .claude/hooks/majordomus-session-guard

# --- the switch dropped from the schema: a key the schema does not know stops the whole
# policy parsing at run time, for everybody
save share/schemas/majordomus/policy/policy.v1.schema.json
sed 's/guard_before_mutation/renamed_key/g' "$T/pristine" > "$FX/share/schemas/majordomus/policy/policy.v1.schema.json"
expect_exit 10 gate
expect_grep 'the policy schema .* does not declare'
restore share/schemas/majordomus/policy/policy.v1.schema.json

# --- and from the skeleton, where it would exist only here
save share/skeleton/policy.yaml
sed 's/guard_before_mutation/renamed_key/g' "$T/pristine" > "$FX/share/skeleton/policy.yaml"
expect_exit 10 gate
expect_grep 'the policy a new repository is written from .* does not declare'
restore share/skeleton/policy.yaml

# --- every restore put back what it took
expect_exit 0 gate
expect_no_grep '^FAIL'
expect_exit 2 env MJ_ROOT="$FX" "$GATE" --nonsense

echo "    the guard refuses an unready episode when it is switched on, refuses nothing when it is not, never refuses what it cannot decide, and its gate refuses each thing the rule forbids"
