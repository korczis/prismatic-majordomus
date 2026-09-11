# majordomus-covers: session
# An episode's record says which work it belonged to, and it says it from evidence.
#
# The defect this case was written against, reproduced below before it was fixed: a task
# that was already active when the episode opened produced
#
#     task_id: t-...
#     tasks: []
#
# because `tasks` was derived only from task lifecycle *events inside the window* —
# `task.started`, `task.checkpoint`, `task.handed_over`, `task.finished` — and a worker
# who spends a whole episode implementing an already-started task emits none of them. The
# record therefore contradicted itself: one field named the task, the list beside it said
# the episode touched no task at all, and nothing refused the pair.
#
# The same shape applied to issues, whose only evidence class was a `plan_*` transition
# during that exact episode, and to `task.evidence`, which the ledger recorded and the
# record dropped.
#
# What is asserted here is not that the lists are non-empty. It is that the record can
# tell two different silences apart: nothing happened, and nothing could be attributed.
. "$ROOT/test/lib.sh"
"$MJ" init >/dev/null
"$MJ" update >/dev/null
mkdir -p lib docs && echo a > lib/a && echo d > docs/d
git add -A >/dev/null; git commit -qm base

# ---------------------------------------------------------------- a task active before the episode
# The order is the whole point: the task starts, and only then does the episode open. Every
# task event the ledger will ever hold for this task is already behind us.
expect_exit 0 "$MJ" start "implement the thing" --scope lib
expect_exit 0 "$MJ" session start --worker "test/worker"
echo change >> lib/a                       # work happens; no task event accompanies it
printf 'Worked on the active task.\n' | "$MJ" session close > closed.txt
rec="$(cat closed.txt)"
[ -f "$rec" ] || { echo "    close printed no record"; exit 1; }

task="$(sed -n 's/^task_id: //p' "$rec" | head -n 1)"
[ -n "$task" ] && [ "$task" != none ] || { echo "    the record names no task_id"; exit 1; }

# The contradiction: task_id names a task and tasks claims the episode touched none.
grep -q '^tasks: \[\]$' "$rec" && {
  echo "    task_id is $task and tasks is empty: the record contradicts itself"; exit 1; }
grep -qF "  - \"$task\"" "$rec" || {
  echo "    tasks does not carry $task, which was active for the whole episode"; exit 1; }

# And it says *why* it believes that, rather than leaving the reader to guess which
# evidence class put the id there.
expect_grep "$rec" '^attribution:'
grep -q 'task_active_at_open' "$rec" || {
  echo "    the record does not record how the task was attributed"; exit 1; }

# ---------------------------------------------------------------- an episode that truly did nothing
# Absence has to be provable, not merely printed. This episode opens and closes with no
# task, no mutation and no event, and its record must say so in as many words.
"$MJ" finish --outcome completed >/dev/null 2>&1 || "$MJ" finish --outcome abandoned >/dev/null 2>&1 || true
git add -A >/dev/null; git commit -qm work >/dev/null
expect_exit 0 "$MJ" session start --worker "test/worker"
"$MJ" session close > empty.txt < /dev/null
emp="$(cat empty.txt)"
[ -f "$emp" ] || { echo "    the empty close printed no record"; exit 1; }
grep -q '^completeness: ' "$emp" || { echo "    no completeness on the empty record"; exit 1; }
grep -q '^completeness: complete$' "$emp" || {
  echo "    an episode with provably nothing to attribute is not 'complete': $(sed -n 's/^completeness: //p' "$emp")"; exit 1; }
grep -qi 'no attributable' "$emp" || {
  echo "    the record does not state that no attributable work was observed"; exit 1; }

# ---------------------------------------------------------------- the two silences differ
# A record whose episode demonstrably changed the repository but which can attribute none
# of it is not 'complete'; it is a defect the repository must be able to name.
grep -q '^completeness: complete$' "$rec" || {
  echo "    the attributed record is not complete: $(sed -n 's/^completeness: //p' "$rec")"; exit 1; }

echo "    ok"
