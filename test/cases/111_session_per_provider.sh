# majordomus-covers: capture session
# majordomus-negative: capture session
# One execution episode per provider session, not one per checkout.
#
# The episode used to be a singleton. `session start --if-open keep` returned the already-open
# episode whatever `--provider-session` said, so a second window of the same provider on one
# checkout was folded into the first: its ledger lines were stamped with the other episode's
# id, and either window's end event closed the episode for both. Measured on 2026-09-09 in
# this repository: seven concurrent sessions, one record between them.
#
# Every assertion below drives the shim the provider would run, with the payload it would
# send, for the reason case 54 gives: a lifecycle that only works when a test calls the
# library proves nothing about the window somebody actually closed.
. "$ROOT/test/lib.sh"

# The suite itself may be running inside a provider session — this repository is developed
# in one — and a case that drives two named provider sessions must not silently be a third.
unset MAJORDOMUS_PROVIDER_SESSION CLAUDE_CODE_SESSION_ID

"$MJ" init >/dev/null; "$MJ" update >/dev/null
git add .gitignore >/dev/null 2>&1; git commit -qm "ignore local ai state" >/dev/null 2>&1 || true
"$MJ" capture install >/dev/null
PATH="$(dirname "$MJ"):$PATH"; export PATH

OPEN=.ai/local/state/sessions-open
PTR=.ai/local/state/session-current.yaml
# Every assertion carries its own message. A bare `[ ... ]` under `bash -eu` ends the case
# having said nothing at all, which is a failure nobody can read.
must() { local why="$1"; shift; "$@" || { printf '    %s\n' "$why"; exit 1; }; }
open_count() { find "$OPEN" -maxdepth 1 -name '*.yaml' 2>/dev/null | wc -l | tr -d ' '; }
records()    { find .ai/repo/sessions -maxdepth 1 -name '2*.md' 2>/dev/null | wc -l | tr -d ' '; }
sid_of()     { sed -n 's/^session_id: //p' "$1" | head -n 1; }
start_event() { printf '{"session_id":"%s","source":"%s"}' "$1" "${2:-startup}" | ./.claude/hooks/majordomus-session-start; }
end_event()   { printf '{"session_id":"%s","reason":"%s"}' "$1" "${2:-clear}" | ./.claude/hooks/majordomus-session-end; }

# ---------------------------------------------------------------- the same session twice
# A resume and a compaction fire the start event again. That is the case `--if-open keep`
# exists for and it has to keep working: one provider session, one episode.
start_event win-a startup >/dev/null 2>&1
must "the start event opened no episode" [ "$(open_count)" = 1 ]
must "the episode is not keyed by the provider session that opened it" [ -f "$OPEN/win-a.yaml" ]
a="$(sid_of "$OPEN/win-a.yaml")"
start_event win-a resume >/dev/null 2>"$T/err"
grep -qF 'kept' "$T/err" || { echo "    a resume did not keep the episode it belongs to"; sed 's/^/    | /' "$T/err"; exit 1; }
must "a resume opened a second episode" [ "$(open_count)" = 1 ]
must "a resume replaced the episode it should have kept" [ "$(sid_of "$OPEN/win-a.yaml")" = "$a" ]

# ---------------------------------------------------------------- a different session
# THE REGRESSION. A different provider session is a different worker in the same checkout,
# and it gets an episode of its own rather than being folded into the one already open.
start_event win-b startup >/dev/null 2>&1
must "a different provider session was folded into the open episode" [ "$(open_count)" = 2 ]
b="$(sid_of "$OPEN/win-b.yaml")"
must "two provider sessions were given one episode id" [ "$a" != "$b" ]

# The episode carries the identity it is keyed by. Until now these two fields reached the
# working context and stopped there, so `continuity.state` declared an `OpenSession.provider`
# no writer could ever produce and it was permanently empty.
expect_grep '^provider: "claude-code"$' "$OPEN/win-a.yaml"
expect_grep '^provider_session: "win-a"$' "$OPEN/win-a.yaml"

# session-current.yaml is the pointer to the episode of this checkout, and it is a link into
# the store rather than a second copy of the record.
must "session-current.yaml is not a link into the store" [ -L "$PTR" ]
must "the pointer does not name the episode opened last here" [ "$(sid_of "$PTR")" = "$b" ]
expect_exit 0 "$MJ" session status
expect_grep "Session: +$b"
expect_grep "Provider: +claude-code \(session win-b\)"
expect_grep "Also open: +$a"

# ---------------------------------------------------------------- attribution
# The ledger line is stamped with the episode of the worker that wrote it. This is what the
# singleton got wrong: one window's records were filed under the other window's id.
MAJORDOMUS_PROVIDER_SESSION=win-a "$MJ" decision add "Attributed to the worker that decided" \
  --why "a ledger line stamped with another episode's id is a record filed under the wrong worker" >/dev/null
tail -n 1 .ai/local/state/ledger.jsonl | grep -qF "\"session\":\"$a\"" \
  || { echo "    the ledger line was stamped with the wrong episode:"; tail -n 1 .ai/local/state/ledger.jsonl | sed 's/^/    | /'; exit 1; }

# A worker whose provider session has no episode here falls through to the pointer rather
# than losing its attribution: sessions opened by hand still stamp the commands of the
# worker sitting in that checkout.
MAJORDOMUS_PROVIDER_SESSION=win-nothing "$MJ" decision add "Falls through to the pointer" \
  --why "an unopened provider session must not cost a worker its attribution" >/dev/null
tail -n 1 .ai/local/state/ledger.jsonl | grep -qF "\"session\":\"$b\"" \
  || { echo "    an unknown provider session did not fall through to the pointer"; tail -n 1 .ai/local/state/ledger.jsonl | sed 's/^/    | /'; exit 1; }

# ---------------------------------------------------------------- the end event
# It closes its own episode and no other. Before this, the second window to be shut ended
# the first window's episode, and the first window went on writing into an episode that had
# already been closed and published.
was="$(records)"
end_event win-a clear >/dev/null 2>"$T/err"
must "the end event wrote no session record" [ "$(records)" = "$((was + 1))" ]
must "the end event closed more than its own episode" [ "$(open_count)" = 1 ]
must "the end event closed the other worker's episode" [ -f "$OPEN/win-b.yaml" ]
must "the end event moved a pointer that was not aimed at it" [ "$(sid_of "$PTR")" = "$b" ]
grep -qF "session_id: $a" "$("$MJ" session latest --path)" \
  || { echo "    the record written names an episode other than the one that ended"; exit 1; }
expect_exit 0 "$MJ" session status
expect_grep "Session: +$b"
expect_no_grep 'Also open:'

# An end event naming a provider session with no episode here closes nothing at all. It is
# the normal case — the episode was closed by hand, or the window never opened one.
end_event win-z clear >/dev/null 2>"$T/err"
grep -qF 'nothing to close' "$T/err" || { echo "    an end event for an unknown session did not report absence"; sed 's/^/    | /' "$T/err"; exit 1; }
must "an end event for an unknown provider session closed something" [ "$(open_count)" = 1 ]
must "an end event for an unknown provider session wrote a record" [ "$(records)" = "$((was + 1))" ]

# The last episode closing leaves nothing behind: no store file, and no pointer aiming at one.
end_event win-b clear >/dev/null 2>&1
must "closing the last episode left it in the store" [ "$(open_count)" = 0 ]
must "closing the last episode left the pointer behind" [ ! -e "$PTR" ]
must "closing the last episode left a dangling pointer" [ ! -L "$PTR" ]

# ---------------------------------------------------------------- no provider at all
# A hand-opened episode still works, and it is the one episode no provider session owns.
# An episode nobody named is keyed `hand`, and there is at most one of those: a provider
# that sends no session identity is indistinguishable from a person at a terminal, and
# inventing a distinction there would multiply episodes nobody can close.
expect_exit 0 "$MJ" session start --owner tester
must "a hand-opened episode is not the hand-opened episode" [ -f "$OPEN/hand.yaml" ]
expect_no_grep '^provider:' "$OPEN/hand.yaml"
h="$(sid_of "$OPEN/hand.yaml")"
expect_exit 15 "$MJ" session start
expect_grep 'is open here since'
expect_exit 0 "$MJ" session status
expect_grep "Session: +$h"
expect_no_grep 'Provider:'

# A provider session opening one beside a person's does not disturb it...
start_event win-c startup >/dev/null 2>&1
must "the provider's episode replaced the person's" [ "$(open_count)" = 2 ]
must "the pointer was not aimed at the episode just opened" [ "$(sid_of "$PTR")" != "$h" ]
# ...and when the provider's closes, the pointer is aimed back at the one episode still
# open, because a person whose episode outlives a provider's must not be told there is none.
end_event win-c clear >/dev/null 2>&1
must "the provider's end event closed the person's episode" [ "$(open_count)" = 1 ]
must "the pointer was not aimed back at the episode still open" [ "$(sid_of "$PTR")" = "$h" ]
expect_exit 0 "$MJ" session status
expect_grep "Session: +$h"

# --- and the person can still close their own, by hand, the way they opened it
expect_exit 0 "$MJ" session close
must "the hand-opened episode survived its own close" [ "$(open_count)" = 0 ]
expect_exit 0 "$MJ" session status
expect_grep 'No open session'
