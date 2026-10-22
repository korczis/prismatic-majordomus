# majordomus-covers: capture checkpoint handover
# The session lifecycle is the episode's, not the task's — the 2026-09-05 outage, kept.
#
# This case is the genetic memory of a production failure. On 2026-09-05 task
# t-20260905034523-a9f1 was marked `outcome: handed_over` and never replaced. From that
# moment this repository wrote no checkpoint and no handover for six days, while episodes
# went on opening and closing normally and `doctor` reported health throughout. Three
# guards shared one premise — that a checkpoint and a continuation record are things that
# happen inside an *active task* — and each of them refused correctly, wrote a line to
# stderr nobody keeps, and returned 0, which is what a provider hook must do.
#
# Meanwhile the briefing resolved the newest handover on the branch and quoted its
# `Next Action` verbatim into every new episode, labelled `advanced`: a true statement
# about git topology that says nothing whatever about age. Six days of workers were told to
# continue work that had been finished for six days.
#
# The seed below is that state, to the day. Everything after it asserts that the lifecycle
# now runs anyway (ADR 0052). A failure here is not a style regression; it is the same
# outage returning.
. "$ROOT/test/lib.sh"

# The suite is itself developed inside a provider session. A case that drives named
# provider sessions must not silently be a member of the one running it.
unset MAJORDOMUS_PROVIDER_SESSION CLAUDE_CODE_SESSION_ID

"$MJ" init >/dev/null; "$MJ" update >/dev/null
git add .gitignore >/dev/null 2>&1; git commit -qm "ignore local ai state" >/dev/null 2>&1 || true
"$MJ" capture install >/dev/null
PATH="$(dirname "$MJ"):$PATH"; export PATH

STATE=.ai/local/state
LEDGER="$STATE/ledger.jsonl"
must() { local why="$1"; shift; "$@" || { printf '    %s\n' "$why"; exit 1; }; }
newest() { find "$1" -maxdepth 1 -name '2*.md' 2>/dev/null | LC_ALL=C sort | tail -n 1; }
count_in() { find "$1" -maxdepth 1 -name '2*.md' 2>/dev/null | wc -l | tr -d ' '; }
event_count() { grep -c "\"event\":\"$1\"" "$LEDGER" 2>/dev/null || true; }
start_event() { printf '{"session_id":"%s","source":"%s"}' "$1" "${2:-startup}" | ./.claude/hooks/majordomus-session-start; }
end_event()   { printf '{"session_id":"%s","reason":"%s"}' "$1" "${2:-clear}" | ./.claude/hooks/majordomus-session-end; }
compact_event() { printf '{"session_id":"%s","trigger":"auto"}' "$1" | ./.claude/hooks/majordomus-session-compact; }

# ---------------------------------------------------------------- seed: six days ago
# The clock is moved rather than the records forged. Every writer below is the real one,
# so the fixture is a state this tool can actually reach, and the timestamps in the front
# matter agree with the ones in the filenames because both read the same instant.
SIX_DAYS_AGO=2026-09-05T03:45:23Z
THEN_HANDOVER=2026-09-05T03:50:03Z

echo "seed" > seed.txt && git add seed.txt && git commit -qm "the commit the stale records name"
OLD_HEAD="$(git rev-parse HEAD)"

MAJORDOMUS_NOW="$SIX_DAYS_AGO" "$MJ" start "the task that was handed over and never replaced" \
  --scope . >/dev/null
MAJORDOMUS_NOW="$SIX_DAYS_AGO" "$MJ" checkpoint --derive >/dev/null
MAJORDOMUS_NOW="$THEN_HANDOVER" "$MJ" handover --derive --close >/dev/null

must "the seed did not leave the task handed over" grep -q '^outcome: handed_over' "$STATE/current.yaml"
STALE_HANDOVER="$(newest "$STATE/handovers")"
must "the seed wrote no handover to go stale" [ -n "$STALE_HANDOVER" ]
must "the stale handover is not dated six days ago" \
  grep -q "^created_at: $THEN_HANDOVER" "$STALE_HANDOVER"

# The old head must be an ancestor of the current one, so that divergence reports
# `advanced` — the label that made this outage invisible. Without this the record would be
# `diverged` and the briefing would already have warned about it for the wrong reason.
echo "work done since" > later.txt && git add later.txt && git commit -qm "six days of other work"
must "the stale record's commit is not an ancestor of HEAD, so this is not the outage" \
  git merge-base --is-ancestor "$OLD_HEAD" HEAD

SEED_CHECKPOINTS="$(count_in "$STATE/checkpoints")"
SEED_HANDOVERS="$(count_in "$STATE/handovers")"

# ---------------------------------------------------------------- SessionStart
# The briefing must show the record and must not hand its Next Action to the worker as the
# thing to do now. Both halves are asserted: suppressing the record entirely would lose
# context that is genuinely useful, and quoting it is the defect.
start_event win-stale startup > "$T/brief.txt" 2>"$T/brief.err"

must "the briefing does not mention the handover at all; history is context, not nothing" \
  grep -q "$(basename "$STALE_HANDOVER")" "$T/brief.txt"
must "the briefing does not say the record is stale" grep -qi 'stale' "$T/brief.txt"
# Printed so that a reader of a passing run can see what a worker is actually handed.
printf '    briefing on a six-day-old handover:\n'; sed 's/^/    | /' "$T/brief.txt"
must "the briefing does not say how old the record is" grep -qE '[0-9]+d ago' "$T/brief.txt"
grep -q 'Its Next Action:' "$T/brief.txt" && {
  echo "    the briefing quoted a six-day-old Next Action as the current instruction"
  sed 's/^/    | /' "$T/brief.txt"; exit 1; }

# Divergence is still reported, and still says `advanced`. Freshness did not replace it;
# it sits beside it, which is the whole point — the two answer different questions.
must "the briefing lost the divergence label" grep -q 'advanced' "$T/brief.txt"

# The receipt is written before anything decides what to do about the event.
must "the start event left no receipt in the ledger" \
  grep -q '"event":"provider.event.received".*"provider_event":"start"' "$LEDGER"

# ---------------------------------------------------------------- PreCompact
# A compaction discards the conversation whether or not a task is open, and whatever
# outcome the last task reached. This is the guard that stopped six days of checkpoints.
compact_event win-stale > "$T/compact.out" 2>"$T/compact.err"

must "the compaction event left no receipt" \
  grep -q '"event":"provider.event.received".*"provider_event":"compact"' "$LEDGER"
AFTER_COMPACT="$(count_in "$STATE/checkpoints")"
[ "$AFTER_COMPACT" -gt "$SEED_CHECKPOINTS" ] || {
  echo "    the compaction wrote no checkpoint: $SEED_CHECKPOINTS before, $AFTER_COMPACT after"
  echo "    this is the 2026-09-05 outage — a handed-over task suppressing the episode's records"
  sed 's/^/    | /' "$T/compact.err"; exit 1; }

# ---------------------------------------------------------------- SessionEnd
end_event win-stale clear > "$T/end.out" 2>"$T/end.err"

must "the end event left no receipt" \
  grep -q '"event":"provider.event.received".*"provider_event":"end"' "$LEDGER"
AFTER_END="$(count_in "$STATE/handovers")"
[ "$AFTER_END" -gt "$SEED_HANDOVERS" ] || {
  echo "    the end event wrote no continuation record: $SEED_HANDOVERS before, $AFTER_END after"
  echo "    a task that was already handed over must not suppress the episode's own handover"
  sed 's/^/    | /' "$T/end.err"; exit 1; }
must "the episode did not close" [ ! -f "$STATE/sessions-open/win-stale.yaml" ]
must "the close wrote no session record" [ -n "$(find .ai/repo/sessions -name '2*.md' 2>/dev/null | head -n 1)" ]

# ---------------------------------------------------------------- no task at all
# The same three events, with no task record in the checkout whatsoever. A checkpoint and a
# continuation record belong to the episode; an episode that worked outside a task still
# produced work, and the only honest thing to write is a record that says so.
rm -f "$STATE/current.yaml"
BEFORE_C="$(count_in "$STATE/checkpoints")"; BEFORE_H="$(count_in "$STATE/handovers")"

start_event win-notask startup >/dev/null 2>&1
compact_event win-notask >/dev/null 2>"$T/nc.err"
end_event win-notask clear >/dev/null 2>"$T/ne.err"

[ "$(count_in "$STATE/checkpoints")" -gt "$BEFORE_C" ] || {
  echo "    a compaction outside a task wrote no checkpoint"; sed 's/^/    | /' "$T/nc.err"; exit 1; }
[ "$(count_in "$STATE/handovers")" -gt "$BEFORE_H" ] || {
  echo "    an end event outside a task wrote no continuation record"; sed 's/^/    | /' "$T/ne.err"; exit 1; }
must "the episode opened outside a task did not close" [ ! -f "$STATE/sessions-open/win-notask.yaml" ]

# The records written outside a task say so rather than naming a task nobody opened, and
# the events they emit carry no task_id — every reader of those events collects the field's
# distinct values as task identifiers, and a literal "none" would enter a session record's
# task list as though somebody had opened a task by that name.
must "a checkpoint written outside a task did not record task_id: none" \
  grep -q '^task_id: none' "$(newest "$STATE/checkpoints")"
grep '"event":"task.checkpoint"' "$LEDGER" | tail -n 1 | grep -q '"task_id"' && {
  echo "    a checkpoint outside a task emitted a task_id anyway"; exit 1; }

# ---------------------------------------------------------------- the command itself
# `majordomus checkpoint` used to exit non-zero when no task was open. The refusal was the
# defect, so the command now succeeds and says what it wrote.
rm -f "$STATE/current.yaml"
expect_exit 0 "$MJ" checkpoint --derive
