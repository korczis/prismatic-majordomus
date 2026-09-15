# majordomus-covers: history session
# majordomus-negative: history
# The ledger has one writer, and it takes the lock.
#
# lib/common.sh's mj_ledger_append composed its line in the shell and appended it with a bare
# `>>`, while the Rust executable appended to the same file under an exclusive lock: two
# writers of one record, one of them unlocked (I1700). The shell now hands the event to
# `majordomus ledger append`, and this case proves what that is for:
#
#   1. shell and Rust appends running at the same time all land, each line whole;
#   2. a shell append names the episode the process resolves to — the provider session it
#      runs inside, over the pointer — exactly as the shell's own resolution always did;
#   3. with no executable the append is a named refusal (exit 12) that writes nothing, and
#      an executable that predates `ledger append` is named as that, never silence.
#
# Case 133 holds the executable's line to the shell's envelope byte for byte; case 282 holds
# the envelope-key refusal. Neither is repeated here.
. "$ROOT/test/lib.sh"
unset MAJORDOMUS_PROVIDER_SESSION CLAUDE_CODE_SESSION_ID MJ_SESSION_KEY
RB="$(rust_bin)" || { echo "    the Rust executable could not be built"; exit 1; }
export MAJORDOMUS_BIN="$RB"

"$MJ" init >/dev/null
git add -A >/dev/null && git commit -qm "layer" >/dev/null
LEDGER=.ai/local/state/ledger.jsonl
OPEN=.ai/local/state/sessions-open
must() { local why="$1"; shift; "$@" || { printf '    %s\n' "$why"; exit 1; }; }
lines() { [ -f "$LEDGER" ] && awk 'END { print NR + 0 }' "$LEDGER" || echo 0; }

# A shell append, from the library the tool runs: the function under test and nothing around
# it. `question.opened` requires task_id and question.
probe="$T/append.sh"
cat > "$probe" <<PROBE
MJ_BIN_DIR="$ROOT/bin"; MJ_LIB_DIR="$ROOT/lib"; export MJ_BIN_DIR MJ_LIB_DIR
. "$ROOT/lib/common.sh"
mj_require_installed
mj_ledger_append question.opened "\"task_id\":\"\$1\",\"question\":\"\$2\""
PROBE

# ---------------------------------------------------------------- 1. interleaved, whole
# Forty appends from each program, all started before any is waited for, each carrying a
# value far past the size one write() is atomic for, so an unlocked writer could tear a line
# across another. The shell's go through mj_ledger_append; the Rust ones call the executable
# directly with a payload of their own.
big="$(head -c 70000 /dev/zero | tr '\0' 'x')"
before="$(lines)"
pids=""
for i in $(seq 1 40); do
  bash "$probe" "sh-$i" "$big" 2>>"$T/sh.err" & pids="$pids $!"
  printf '{"task_id":"rs-%s","question":"%s"}' "$i" "$big" \
    | "$RB" ledger append question.opened --root "$PWD" --share "$ROOT/share" 2>>"$T/rs.err" & pids="$pids $!"
done
failed=0
for p in $pids; do wait "$p" || failed=$((failed + 1)); done
[ "$failed" = 0 ] || { echo "    $failed append(s) failed:"; sed 's/^/    | /' "$T/sh.err" "$T/rs.err" | head -20; exit 1; }
after="$(lines)"
must "expected 80 new lines, the ledger grew by $((after - before))" [ "$((after - before))" = 80 ]
bad="$(tail -n 80 "$LEDGER" | while IFS= read -r l; do printf '%s\n' "$l" | jq -e . >/dev/null 2>&1 || echo x; done | wc -l | tr -d ' ')"
must "$bad of the 80 lines are not one JSON document each: a line was torn" [ "$bad" = 0 ]
for who in sh rs; do
  n="$(tail -n 80 "$LEDGER" | jq -r .task_id | grep -c "^$who-")"
  must "expected 40 lines from $who, found $n" [ "$n" = 40 ]
  full="$(tail -n 80 "$LEDGER" | jq -r "select(.task_id | startswith(\"$who-\")) | .question | length" | sort -u)"
  must "a $who line lost part of its payload (lengths: $(printf '%s' "$full" | tr '\n' ' '))" [ "$full" = 70000 ]
done
# every line carries the envelope in the order both writers compose it
orders="$(tail -n 80 "$LEDGER" | jq -r 'keys_unsorted[0:5] | join(",")' | sort -u)"
must "an envelope was composed in another order: $orders" [ "$orders" = "ts,event,head,branch,by" ]
expect_exit 0 "$MJ" history --validate

# ---------------------------------------------------------------- 2. the resolved episode
# Two episodes open here, each keyed by a provider session; the pointer names the one opened
# last. A process inside the first provider session is that episode's worker, and its append
# must say so rather than take the pointer's guess.
"$MJ" session start --provider claude-code --provider-session win-a >/dev/null
"$MJ" session start --provider claude-code --provider-session win-b >/dev/null
a="$(sed -n 's/^session_id: //p' "$OPEN/win-a.yaml")"
hand="$(sed -n 's/^session_id: //p' .ai/local/state/session-current.yaml)"
must "the provider session's episode did not open" [ -n "$a" ]
must "the fixture did not open two distinct episodes ($a, $hand)" [ "$a" != "$hand" ]

MAJORDOMUS_PROVIDER_SESSION=win-a bash "$probe" attributed "whose is this"
got="$(tail -n 1 "$LEDGER" | jq -r .session)"
must "the shell append named episode '$got', not its provider session's episode '$a'" [ "$got" = "$a" ]
bash "$probe" pointer "whose is this"
got="$(tail -n 1 "$LEDGER" | jq -r .session)"
must "with no provider session the append named '$got', not the pointer's '$hand'" [ "$got" = "$hand" ]
# the hook's key is strict: a key naming no open episode names none, never the pointer
MJ_SESSION_KEY=gone bash "$probe" strict "whose is this"
got="$(tail -n 1 "$LEDGER" | jq -r '.session // "none"')"
must "a hook key naming no episode fell back to '$got'" [ "$got" = none ]

# ---------------------------------------------------------------- 3. no executable
before="$(lines)"
MAJORDOMUS_BIN="$T/no-such-executable" expect_exit 12 bash "$probe" lost "nothing"
expect_grep "was not recorded"
expect_grep "is not an executable"
expect_grep "cargo build"
must "a refused append wrote a line" [ "$(lines)" = "$before" ]

# An executable from before `ledger append` existed: clap's usage exit is the missing
# capability, and it is named as such rather than reported as a generic failure.
printf '#!/bin/sh\necho "error: unrecognized subcommand" >&2\nexit 2\n' > "$T/old-majordomus"
chmod +x "$T/old-majordomus"
MAJORDOMUS_BIN="$T/old-majordomus" expect_exit 12 bash "$probe" old "nothing"
expect_grep "predates .ledger append."
must "an append through an old executable wrote a line" [ "$(lines)" = "$before" ]

# A refusal from the writer itself carries its own code through: an undeclared event is 13.
expect_exit 13 "$RB" ledger append question.openned --root "$PWD" --share "$ROOT/share" --payload '{"task_id":"x","question":"q"}'
expect_grep "unregistered event 'question.openned'"
must "a refused event wrote a line" [ "$(lines)" = "$before" ]
