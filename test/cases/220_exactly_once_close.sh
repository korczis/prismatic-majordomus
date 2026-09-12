# majordomus-covers: session capture
# majordomus-negative: session
# One episode, one canonical record — however many times, and however concurrently, the
# thing that closes it is asked to close it.
#
# The record of a closed episode is immutable by contract and is the object every other
# clone reads (ADR 0014). "Immutable" is worth nothing if the store can hold two of them for
# one episode, because then the question "what did episode X do" has two answers and nothing
# says which is the record. This repository has been in that state: episode
# s-20260909152316-024f carries four records — one at 21:41:36 and three the next morning,
# thirteen and fifteen seconds apart — because a provider's SessionEnd event fires more than
# once and `mj_publish_record` gives every file a unique name, so a second close wrote a
# second *file* rather than colliding with the first.
#
# The close path grew a guard for that: it greps the store for the episode's id and, finding
# a record, keeps it and writes none. This case holds that guard to the four shapes the
# incident actually has, and the fourth is the one the guard did not survive:
#
#   1. four repeated closes, one after another
#   2. a close after a crash — the record was published and the process died before the open
#      record was torn down, so the next close finds both
#   3. a close of an episode that is already closed
#   4. TWO CLOSES AT ONCE. The guard was a `grep` followed by a publish, which is a check and
#      a write with a window between them. Measured against master on 2026-09-11: two
#      concurrent end events for one episode produced two canonical records in two of three
#      trials. The fix is a per-episode lock around the check and the write; this is the
#      assertion that fails without it.
#
# Every assertion drives the shim the provider would run, with the payload it would send, for
# the reason cases 54 and 111 give: a lifecycle that only works when a test calls the library
# proves nothing about the window somebody actually closed.
. "$ROOT/test/lib.sh"

# The suite may itself be running inside a provider session — this repository is developed in
# one — and a case that drives named provider sessions must not silently be another.
unset MAJORDOMUS_PROVIDER_SESSION CLAUDE_CODE_SESSION_ID

"$MJ" init >/dev/null; "$MJ" update >/dev/null
# This case is about the close, not about what a start event ensures beside it: the server
# the start event would bring up needs the Rust executable built, and a suite running where
# cargo is busy or absent would fail here for a reason that has nothing to do with sessions.
# Case 108 owns that half.
sed 's/^  ensure_server_on_start: true /  ensure_server_on_start: false /' .ai/repo/policy.yaml > "$T/pol" && cp "$T/pol" .ai/repo/policy.yaml
git add .gitignore >/dev/null 2>&1; git commit -qm "ignore local ai state" >/dev/null 2>&1 || true
"$MJ" capture install >/dev/null
# The shims resolve the executable from their own path and then from PATH. Without this they
# find whatever `majordomus` is installed on the machine running the suite, which is a
# different tool than the one under test — and every assertion below would be about it.
PATH="$(dirname "$MJ"):$PATH"; export PATH

OPEN=.ai/local/state/sessions-open
STORE=.ai/repo/sessions
# A bare `[ ... ]` under the runner's `set -e` ends a case having printed nothing at all.
must() { local why="$1"; shift; "$@" || { printf '    %s\n' "$why"; exit 1; }; }
start_event() { printf '{"session_id":"%s","source":"%s"}' "$1" "${2:-startup}" | ./.claude/hooks/majordomus-session-start; }
end_event()   { printf '{"session_id":"%s","reason":"%s"}' "$1" "${2:-clear}" | ./.claude/hooks/majordomus-session-end; }
sid_of()      { sed -n 's/^session_id: //p' "$1" | head -n 1; }
# Records claiming one episode. `-l` and not a count of lines: a record names its own id
# once, but nothing in the contract says it may not name it twice, and this must count files.
# The store's own README.md is a context document and never matches.
records_for()  { grep -rl "^session_id: $1\$" "$STORE" 2>/dev/null | wc -l | tr -d ' '; }
# Everything in the store that is not a record: mj_publish_record stages into `.tmp.XXXXXX`
# there before it hard-links the final name, so a crash between the two leaves one behind.
strays()       { find "$STORE" -maxdepth 1 -name '.tmp.*' 2>/dev/null | wc -l | tr -d ' '; }

# ---------------------------------------------------------------- 1. four repeated closes
# The shape of the original incident: the same end event delivered four times.
start_event rep startup >/dev/null 2>&1
rep="$(sid_of "$OPEN/rep.yaml")"
must "the start event opened no episode" [ -n "$rep" ]
for _ in 1 2 3 4; do end_event rep clear >/dev/null 2>&1; done
must "four end events left $(records_for "$rep") records for one episode, not 1" [ "$(records_for "$rep")" = 1 ]
must "four end events left the episode open" [ ! -f "$OPEN/rep.yaml" ]

# ---------------------------------------------------------------- 2. a close after a crash
# The record was published and the process died before the open record was torn down — the
# state `majordomus recover` was written for, and the state a re-delivered end event walks
# into. The record that exists is the record; the second close adds none and finishes the
# teardown the first one did not reach.
start_event crash startup >/dev/null 2>&1
crash="$(sid_of "$OPEN/crash.yaml")"
"$MJ" session close --provider-session crash >/dev/null 2>&1
must "the close wrote no record" [ "$(records_for "$crash")" = 1 ]
# put the open record back, byte for byte, as an interrupted teardown would have left it
cat > "$OPEN/crash.yaml" <<Y
session_id: $crash
started_at: $(date -u +%Y-%m-%dT%H:%M:%SZ)
owner: "tester"
provider: "claude-code"
provider_session: "crash"
repository_id: $(git rev-parse --absolute-git-dir)
worktree: $(pwd -P)
branch: $(git rev-parse --abbrev-ref HEAD)
start_head: $(git rev-parse HEAD)
start_working_tree: dirty
Y
end_event crash clear >/dev/null 2>"$T/crash.err"
must "the close after a crash published a second record for one episode" [ "$(records_for "$crash")" = 1 ]
must "the close after a crash left the episode open" [ ! -f "$OPEN/crash.yaml" ]
expect_grep 'already has a record' "$T/crash.err"
# and it named the record that exists rather than the file it did not write
grep -q "$(grep -rl "^session_id: $crash\$" "$STORE" | head -n 1 | sed 's#^\./##')" "$T/crash.err" \
  || { echo "    the second close did not name the record the episode actually has:"; sed 's/^/    | /' "$T/crash.err"; exit 1; }

# ---------------------------------------------------------------- 3. closing what is closed
# No episode here at all. To a person that is a mistake worth stopping; to a provider's end
# event it is the normal case, and `--if-none ignore` is the difference.
expect_exit 12 "$MJ" session close --provider-session crash
expect_grep 'no open session'
expect_exit 0 "$MJ" session close --provider-session crash --if-none ignore
must "closing an already-closed episode wrote a record" [ "$(records_for "$crash")" = 1 ]

# ---------------------------------------------------------------- 4. two closes at once
# THE REGRESSION. Both processes get to the guard before either has published, both read an
# empty store, and both publish. Repeated, because a race that is only sometimes lost is
# still lost: a single round passed on master roughly one time in three.
#
# Three at once rather than two: a provider that re-delivers an event re-delivers it to
# whatever is listening, and two is the smallest number that can race while three is the
# smallest that can race *while one waits*, which is the path the lock's queue takes.
round=0
while [ "$round" -lt 4 ]; do
  round=$((round + 1))
  key="race-$round"
  start_event "$key" startup >/dev/null 2>&1
  sid="$(sid_of "$OPEN/$key.yaml")"
  must "round $round opened no episode" [ -n "$sid" ]
  printf '{"session_id":"%s","reason":"clear"}' "$key" > "$T/payload.json"
  # Started with `&` inside the case's own shell, which the runner has already put in a
  # process group of its own; nothing here outlives the case.
  ./.claude/hooks/majordomus-session-end < "$T/payload.json" >/dev/null 2>&1 &
  a=$!
  ./.claude/hooks/majordomus-session-end < "$T/payload.json" >/dev/null 2>&1 &
  b=$!
  ./.claude/hooks/majordomus-session-end < "$T/payload.json" >/dev/null 2>&1 &
  c=$!
  wait "$a" "$b" "$c" 2>/dev/null || true
  n="$(records_for "$sid")"
  [ "$n" = 1 ] || { printf '    round %s: three simultaneous closes of episode %s left %s canonical records; one episode has one record\n' "$round" "$sid" "$n"
                    grep -rl "^session_id: $sid\$" "$STORE" | sed 's/^/    | /'; exit 1; }
  must "round $round left the episode open after closing it" [ ! -f "$OPEN/$key.yaml" ]
  # The publish stages inside the tracked section. A close that names a temp file as the
  # episode's record names a path that is about to be removed, and a temp left behind is a
  # file in a tracked directory that looks like a record and is not.
  must "round $round left $(strays) staging file(s) in the tracked store" [ "$(strays)" = 0 ]
  # The lock is released on every exit, including the ones that did not take it.
  must "round $round left a close lock behind" [ "$(find .ai/local/state/locks -maxdepth 1 -type d -name 'session-close.*' 2>/dev/null | wc -l | tr -d ' ')" = 0 ]
done

# ---------------------------------------------------------------- and the store agrees
# The identity contract is enforced in two places: the close path refuses to write a second
# record, and `doctor`'s session doctrine refuses a store that holds one ("two records claim
# this identity"). Everything above tested the first; this is the store read as a whole, so
# that a case which somehow satisfied every episode individually while leaving a duplicate
# behind still fails. `doctor` itself is not run here — it is a minute of a suite that is
# already too long, it reports on a fixture repository's missing git hooks, and
# 63_session_records owns the doctrine.
dup="$(grep -h '^session_id: ' "$STORE"/*.md 2>/dev/null | LC_ALL=C sort | uniq -d)"
[ -z "$dup" ] || { printf '    the store holds two records claiming one episode:\n'; printf '%s\n' "$dup" | sed 's/^/    | /'; exit 1; }
files="$(find "$STORE" -maxdepth 1 -name '2*.md' | wc -l | tr -d ' ')"
ids="$(grep -h '^session_id: ' "$STORE"/*.md 2>/dev/null | LC_ALL=C sort -u | wc -l | tr -d ' ')"
must "the store holds $files records for $ids episodes" [ "$files" = "$ids" ]
must "the run left staging files in the tracked store" [ "$(strays)" = 0 ]
