# majordomus-covers: session capture
# majordomus-negative: session
# A client that dies without sending its end event, and what the next one finds.
#
# `continuity.state` is exercised here through `session status` and the store it reads, and
# is not in the covers header: the header names public commands, and the typed reader is an
# MCP and HTTP capability with no command line of its own.
#
# This is the ordinary way an episode ends. A provider that is killed, crashes, loses its
# connection or is shut down by a spend limit sends no SessionEnd: the open record stays in
# state/sessions-open/ and the episode is open for ever, `session start` with that key is
# refused, and `continuity.state` reports an episode nobody is in. Measured in this
# repository's primary checkout on 2026-09-11: five open episodes, two of which had not
# stamped a ledger line since the previous evening.
#
# What this case holds is the path a real client takes back: it restarts, and its SessionStart
# event arrives for an episode that is still open. Three things have to be true of that, and
# each of them has a way of failing quietly:
#
#   the episode is kept, not duplicated and not replaced. `--if-open keep` is what the start
#   shim passes, and it is the difference between resuming an episode and starting a second
#   one that shadows it — the ledger lines written before the crash belong to the first.
#
#   the close that eventually comes writes exactly one record, and says the episode was cut
#   short. `interrupted` is the only thing about an ended episode that changes what the next
#   worker does, and the reason vocabulary decides it: a reason the provider's adapter does
#   not call clean closes the episode as interrupted. (The reason *string* itself reaches
#   stderr and nothing durable; the record carries the two-valued outcome and no more. That
#   is what exists, and this case asserts that and not more.)
#
#   a live peer is not touched. An episode is stranded or it is somebody's, and nothing here
#   can tell the difference by looking at one file — which is why the deliberate sweep that
#   does decide it (`majordomus recover`, on feature/session-store-recovery) measures age and
#   fails closed, and why 135_session_store_recovery owns that judgement rather than this
#   case. What is asserted here is the weaker and more important property: the ordinary crash
#   path never reaps anything, so a worker in the next window along keeps its episode.
#
# The client is killed with SIGKILL, which it cannot catch and so cannot clean up after —
# the only kill that produces the state this case is about. It is killed by pid, never by
# name: this machine runs the sessions of other checkouts, and a pattern kill would take
# them with it.
. "$ROOT/test/lib.sh"

unset MAJORDOMUS_PROVIDER_SESSION CLAUDE_CODE_SESSION_ID

"$MJ" init >/dev/null; "$MJ" update >/dev/null
sed 's/^  ensure_server_on_start: true /  ensure_server_on_start: false /' .ai/repo/policy.yaml > "$T/pol" && cp "$T/pol" .ai/repo/policy.yaml
git add .gitignore >/dev/null 2>&1; git commit -qm "ignore local ai state" >/dev/null 2>&1 || true
"$MJ" capture install >/dev/null
PATH="$(dirname "$MJ"):$PATH"; export PATH

OPEN=.ai/local/state/sessions-open
PTR=.ai/local/state/session-current.yaml
LEDGER=.ai/local/state/ledger.jsonl
must() { local why="$1"; shift; "$@" || { printf '    %s\n' "$why"; exit 1; }; }
start_event() { printf '{"session_id":"%s","source":"%s"}' "$1" "${2:-startup}" | ./.claude/hooks/majordomus-session-start; }
end_event()   { printf '{"session_id":"%s","reason":"%s"}' "$1" "${2:-clear}" | ./.claude/hooks/majordomus-session-end; }
sid_of()      { sed -n 's/^session_id: //p' "$1" | head -n 1; }
records_for() { grep -rl "^session_id: $1\$" .ai/repo/sessions 2>/dev/null | wc -l | tr -d ' '; }

# ---------------------------------------------------------------- a client that dies
# The worker attaches, works, and is killed. The work is a real command inside the episode,
# so the episode has a ledger line to be resumed against rather than only a file.
start_event dead startup >/dev/null 2>&1
dead="$(sid_of "$OPEN/dead.yaml")"
must "the start event opened no episode" [ -n "$dead" ]
MAJORDOMUS_PROVIDER_SESSION=dead "$MJ" decision add "Recorded before the crash" \
  --why "an episode that is resumed must still own what it wrote before it died" >/dev/null
grep -qF "\"session\":\"$dead\"" "$LEDGER" \
  || { echo "    the work done inside the episode was not attributed to it"; exit 1; }

# A peer in the next window along, open the whole time. Nothing below may disturb it.
start_event live startup >/dev/null 2>&1
live="$(sid_of "$OPEN/live.yaml")"
must "the peer episode did not open" [ -n "$live" ]
must "the two episodes were given one id" [ "$dead" != "$live" ]

# The client itself. A real process, inside the episode, holding it the way a provider does;
# SIGKILL so that no trap, no exit handler and no end event can run.
( MAJORDOMUS_PROVIDER_SESSION=dead exec sleep 300 ) &
client=$!
kill -KILL "$client" 2>/dev/null || true
wait "$client" 2>/dev/null || true
must "the killed client left no open episode behind, so there is nothing to recover" [ -f "$OPEN/dead.yaml" ]
must "the crash closed the episode, which no end event asked for" [ "$(records_for "$dead")" = 0 ]
must "the crash disturbed the peer's episode" [ -f "$OPEN/live.yaml" ]

# ---------------------------------------------------------------- what a reader is told
# Absence of an end event is not absence of an episode. The pointer still resolves and the
# typed read model still reports the episode — which is the honest answer and the reason the
# staleness judgement has to be made by something that measures age rather than by a reader.
expect_exit 0 "$MJ" session status
expect_grep 'Session: +s-'
must "the pointer stopped resolving when the client died" [ -e "$PTR" ]

# ---------------------------------------------------------------- the client comes back
# The provider restarts and its SessionStart arrives for an episode that never closed. It is
# kept: the same id, the same file, no second episode, and nothing lost from the ledger.
start_event dead resume >/dev/null 2>"$T/resume.err"
expect_grep 'kept' "$T/resume.err"
must "the restart opened a second episode for one provider session" \
  [ "$(find "$OPEN" -maxdepth 1 -name '*.yaml' | wc -l | tr -d ' ')" = 2 ]
must "the restart replaced the episode it should have kept" [ "$(sid_of "$OPEN/dead.yaml")" = "$dead" ]
must "the restart disturbed the peer's episode" [ "$(sid_of "$OPEN/live.yaml")" = "$live" ]
grep -qF "\"session\":\"$dead\"" "$LEDGER" \
  || { echo "    the work recorded before the crash lost its episode across the restart"; exit 1; }

# ---------------------------------------------------------------- and it ends, once
# A reason the provider's adapter does not call clean — a crash, a name the table has not
# seen — closes the episode as interrupted, because calling a cut-short episode complete is
# the worse mistake.
end_event dead other >/dev/null 2>"$T/end.err"
must "the recovered episode produced $(records_for "$dead") records, not 1" [ "$(records_for "$dead")" = 1 ]
rec="$(grep -rl "^session_id: $dead\$" .ai/repo/sessions | head -n 1)"
expect_grep '^outcome: interrupted$' "$rec"
expect_grep "\"event\":\"session.closed\".*\"outcome\":\"interrupted\"" "$LEDGER"
must "the close left the episode open" [ ! -f "$OPEN/dead.yaml" ]
must "the close removed no episode file at all" [ "$(find "$OPEN" -maxdepth 1 -name '*.yaml' | wc -l | tr -d ' ')" = 1 ]
# THE ASSERTION THIS CASE EXISTS FOR, BESIDE EXACTLY-ONCE: the live worker in the next
# window is still in its episode. A recovery that ends an episode nobody asked it to end
# costs a worker the boundary of the work it is doing, which is the expensive mistake.
must "closing the crashed episode closed the live peer's as well" [ -f "$OPEN/live.yaml" ]
must "the live peer's episode lost its identity" [ "$(sid_of "$OPEN/live.yaml")" = "$live" ]
must "the live peer's episode was published as closed" [ "$(records_for "$live")" = 0 ]
# and the pointer, freed by the close, is aimed at the one episode still open here
must "the pointer was not aimed back at the episode still open" [ "$(sid_of "$PTR")" = "$live" ]

# ---------------------------------------------------------------- a second end event
# The provider re-delivers. There is nothing left to close and nothing is written; 220 owns
# the concurrent shape of this, and this is the sequential one after a crash.
end_event dead other >/dev/null 2>"$T/end2.err"
expect_grep 'nothing to close' "$T/end2.err"
must "a second end event after recovery wrote a second record" [ "$(records_for "$dead")" = 1 ]

# ---------------------------------------------------------------- nothing lingers
end_event live clear >/dev/null 2>&1
must "closing the last episode left it in the store" [ "$(find "$OPEN" -maxdepth 1 -name '*.yaml' | wc -l | tr -d ' ')" = 0 ]
must "closing the last episode left the pointer behind" [ ! -e "$PTR" ]
must "closing the last episode left a dangling pointer" [ ! -L "$PTR" ]
must "the run left staging files in the tracked store" \
  [ "$(find .ai/repo/sessions -maxdepth 1 -name '.tmp.*' 2>/dev/null | wc -l | tr -d ' ')" = 0 ]
must "the run left a close lock behind" \
  [ "$(find .ai/local/state/locks -maxdepth 1 -type d -name 'session-close.*' 2>/dev/null | wc -l | tr -d ' ')" = 0 ]
