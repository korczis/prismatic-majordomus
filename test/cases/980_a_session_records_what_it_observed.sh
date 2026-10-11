# majordomus-covers: knowledge
# majordomus-negative: knowledge
# claim: a-session-records-what-it-observed
# A worker records what it met in one command, and only an observation of the right shape is
# recorded (ADR 0118).
#
# `knowledge observe` writes one `observation.recorded` line with the kind, the subject, the
# statement and each evidence reference, inside a task or outside one, while an episode is open
# (without one it is refused with exit 10, because nothing would ever derive it). A kind outside the
# five, a subject that is neither a repository path nor a typed identifier, an evidence
# reference that names nothing typed, a statement that spans lines or opens like a turn of a
# conversation: each is refused with exit 2 and leaves the ledger as it was.
. "$ROOT/test/lib.sh"

unset MAJORDOMUS_PROVIDER_SESSION CLAUDE_CODE_SESSION_ID

"$MJ" init >/dev/null; "$MJ" update >/dev/null
git add .gitignore >/dev/null 2>&1; git commit -qm "ignore local ai state" >/dev/null 2>&1 || true

LEDGER=.ai/local/state/ledger.jsonl
count() { grep -c '"event":"observation.recorded"' "$LEDGER" 2>/dev/null || true; }

# ---------------------------------------------------------------- no episode, no observation
rc=0; "$MJ" knowledge observe --kind friction --subject lib "nobody would derive this" >/dev/null 2>&1 || rc=$?
[ "$rc" = 10 ] || { echo "    an observation with no open episode exited $rc, expected 10"; exit 1; }
[ "$(count)" = 0 ] || { echo "    an observation with no open episode was written"; exit 1; }
"$MJ" session start >/dev/null

# ---------------------------------------------------------------- outside a task
"$MJ" knowledge observe --kind friction --subject scripts/site-build \
  --evidence commit:bf9b27caeb --evidence issue:I2091 \
  "a moved head composes a stale rustdoc surface" >/dev/null \
  || { echo "    a well-formed observation outside a task was refused"; exit 1; }
[ "$(count)" = 1 ] || { echo "    expected one observation line, found $(count)"; exit 1; }
line="$(grep '"event":"observation.recorded"' "$LEDGER" | tail -n 1)"
for want in '"kind":"friction"' '"subject":"scripts/site-build"' \
  '"statement":"a moved head composes a stale rustdoc surface"' \
  '"evidence":["commit:bf9b27caeb","issue:I2091"]' '"task_id":"none"'; do
  printf '%s' "$line" | grep -qF "$want" || { echo "    the line lacks $want: $line"; exit 1; }
done

# ---------------------------------------------------------------- inside a task, typed subjects
"$MJ" start "observe under a task" --scope lib >/dev/null
TASK="$(sed -n 's/^id: //p' .ai/local/state/current.yaml | head -n 1)"
for subject in capability:intents.binding rule:project.derived-once command:commit gate:intent-check .githooks/pre-push; do
  "$MJ" knowledge observe --kind repetition --subject "$subject" "it happened again" >/dev/null \
    || { echo "    the subject $subject was refused"; exit 1; }
done
[ "$(count)" = 6 ] || { echo "    expected six observation lines, found $(count)"; exit 1; }
grep '"event":"observation.recorded"' "$LEDGER" | tail -n 1 | grep -qF "\"task_id\":\"$TASK\"" \
  || { echo "    an observation inside a task does not name the task"; exit 1; }
grep '"event":"observation.recorded"' "$LEDGER" | tail -n 1 | grep -qF '"evidence":[]' \
  || { echo "    an observation without evidence does not carry an empty list"; exit 1; }

# ---------------------------------------------------------------- refusals write nothing
refused() { # <why> <args...>
  local why="$1"; shift
  local rc=0
  "$MJ" knowledge observe "$@" >/dev/null 2>&1 || rc=$?
  [ "$rc" = 2 ] || { echo "    $why: exit $rc, expected 2"; exit 1; }
  [ "$(count)" = 6 ] || { echo "    $why: a refused observation was written"; exit 1; }
}
refused "an unknown kind"          --kind annoyance --subject lib "x"
refused "no kind"                  --subject lib "x"
refused "no subject"               --kind defect "x"
refused "an absolute path"         --kind defect --subject /etc/passwd "x"
refused "a path that climbs"       --kind defect --subject ../outside "x"
refused "an untyped identifier"    --kind defect --subject url:http://x "x"
refused "an empty identifier"      --kind defect --subject capability: "x"
refused "evidence naming nothing"  --kind defect --subject lib --evidence "see chat" "x"
refused "no statement"             --kind defect --subject lib
refused "two statements"           --kind defect --subject lib "x" "y"
refused "a multi-line statement"   --kind defect --subject lib "$(printf 'one\ntwo')"
refused "a conversation"           --kind defect --subject lib "assistant: I did it"
exit 0
