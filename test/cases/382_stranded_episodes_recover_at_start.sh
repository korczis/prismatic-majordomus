# majordomus-covers: capture session recover
# majordomus-negative: capture recover
# A stranded episode is closed by the next provider session that starts, not by a person
# who remembers `recover episodes`.
#
# A provider that is killed sends no end event, and its episode stays open in
# state/sessions-open/ for ever. `recover episodes` closes such episodes correctly and ran only
# by hand, so nobody ran it: 21 were stranded in this repository's primary checkout and
# session.recovered had never been written (I1700). The start event now runs it, and this case
# drives the shim the provider runs to prove:
#
#   1. an old stranded episode is closed interrupted, with its record and session.recovered;
#   2. a live episode, and the episode the start event itself belongs to, are left open;
#   3. it happens exactly once: a second start recovers nothing more;
#   4. it is bounded — MJ_RECOVER_LIMIT closes, the rest on the next start;
#   5. it is fail-open — a policy with no threshold recovers nothing, says so in the log, and
#      the session still starts.
. "$ROOT/test/lib.sh"
unset MAJORDOMUS_PROVIDER_SESSION CLAUDE_CODE_SESSION_ID MJ_SESSION_KEY MAJORDOMUS_NOW
RB="$(rust_bin)" || { echo "    the Rust executable could not be built"; exit 1; }
export MAJORDOMUS_BIN="$RB"

"$MJ" init >/dev/null
# no server and no briefing: neither is under test, and a server a fixture starts outlives it
sed -i.bak -e "s/ensure_server_on_start: true/ensure_server_on_start: false/" -e "s/briefing_on_start: true/briefing_on_start: false/" .ai/repo/policy.yaml && rm -f .ai/repo/policy.yaml.bak
"$MJ" update >/dev/null
git add -A >/dev/null 2>&1; git commit -qm "layer" >/dev/null 2>&1 || true
"$MJ" capture install >/dev/null
PATH="$(dirname "$MJ"):$PATH"; export PATH

OPEN=.ai/local/state/sessions-open
LEDGER=.ai/local/state/ledger.jsonl
must() { local why="$1"; shift; "$@" || { printf '    %s\n' "$why"; exit 1; }; }
sid_of() { sed -n 's/^session_id: //p' "$1" | head -n 1; }
recovered() { grep -c '"event":"session.recovered"' "$LEDGER" 2>/dev/null || true; }
start_event() { printf '{"session_id":"%s","source":"%s"}' "$1" "${2:-startup}" | ./.claude/hooks/majordomus-session-start; }
# An episode opened, and then abandoned, long ago: its start event and every line it stamped
# carry a clock two years back, which is what a provider killed then leaves behind. Its own
# start event also runs recovery, at that clock, where every other episode is younger than it
# and nothing is a candidate; the start that follows at today's clock is the one that finds it.
strand() { MAJORDOMUS_NOW=2024-01-01T00:00:00Z start_event "$1" >/dev/null 2>&1; must "the fixture episode $1 did not open" [ -f "$OPEN/$1.yaml" ]; }

start_event live-b >/dev/null 2>&1
must "the live episode did not open" [ -f "$OPEN/live-b.yaml" ]
strand old-a
old_a="$(sid_of "$OPEN/old-a.yaml")"
must "the fixture already recovered something" [ "$(recovered)" = 0 ]

# ---------------------------------------------------------------- 1 and 2. one start
expect_exit 0 start_event new-c
must "the start event's own episode did not open" [ -f "$OPEN/new-c.yaml" ]
must "the stranded episode is still open after a start event" [ ! -f "$OPEN/old-a.yaml" ]
must "the live episode was closed" [ -f "$OPEN/live-b.yaml" ]
must "expected one session.recovered, found $(recovered)" [ "$(recovered)" = 1 ]
tail_rec="$(grep '"event":"session.recovered"' "$LEDGER" | tail -n 1)"
printf '%s' "$tail_rec" | jq -e --arg s "$old_a" '.session_id == $s' >/dev/null \
  || { echo "    session.recovered names another episode:"; printf '    | %s\n' "$tail_rec"; exit 1; }
rec="$(grep -rl "^session_id: $old_a\$" .ai/repo/sessions 2>/dev/null | head -n 1 || true)"
must "no record was written for the recovered episode" [ -n "$rec" ]
expect_grep '^outcome: interrupted$' "$rec"
# the start event's stdout is the briefing; recovery's report went to its log, not there
expect_grep "stranded: .*close with outcome interrupted" .ai/local/state/recover.log
start_event new-c resume > "$T/briefing" 2>/dev/null
expect_no_grep "stranded:" "$T/briefing"

# The episode the start event belongs to is never a candidate, even when its own record looks
# abandoned: a resume of an old episode is a worker coming back into it.
strand old-self
expect_exit 0 start_event old-self resume
must "the start event recovered its own episode" [ -f "$OPEN/old-self.yaml" ]
expect_grep "live: this process is inside it" .ai/local/state/recover.log

# ---------------------------------------------------------------- 3. exactly once
n="$(recovered)"
expect_exit 0 start_event new-d
must "a second start recovered again ($n, then $(recovered))" [ "$(recovered)" = "$n" ]
must "the live episode was closed by a later start" [ -f "$OPEN/live-b.yaml" ]
count="$(grep -rl "^session_id: $old_a\$" .ai/repo/sessions | wc -l | tr -d ' ')"
must "the recovered episode has $count records, not one" [ "$count" = 1 ]

# ---------------------------------------------------------------- 4. bounded
strand old-e; strand old-f; strand old-g
n="$(recovered)"
MJ_RECOVER_LIMIT=2 expect_exit 0 start_event new-h
must "the limit of 2 closed $(( $(recovered) - n ))" [ "$(( $(recovered) - n ))" = 2 ]
expect_grep "deferred: 2 closed already" .ai/local/state/recover.log
MJ_RECOVER_LIMIT=2 expect_exit 0 start_event new-h resume
must "the deferred episode was not closed by the next start" [ "$(( $(recovered) - n ))" = 3 ]
for k in old-e old-f old-g; do must "$k is still open" [ ! -f "$OPEN/$k.yaml" ]; done

# ---------------------------------------------------------------- 5. fail-open
strand old-i
cp .ai/repo/policy.yaml "$T/policy.yaml"
grep -v 'stranded_after:' "$T/policy.yaml" > .ai/repo/policy.yaml
if grep -q 'stranded_after:' .ai/repo/policy.yaml; then echo "    the mutation left the threshold in the policy"; exit 1; fi
expect_exit 0 start_event new-j
must "the session did not start when recovery could not run" [ -f "$OPEN/new-j.yaml" ]
must "recovery ran without a threshold" [ -f "$OPEN/old-i.yaml" ]
expect_grep "stranded episodes were not recovered .*recover episodes" .ai/local/session-contexts/.session-context.log
cp "$T/policy.yaml" .ai/repo/policy.yaml
