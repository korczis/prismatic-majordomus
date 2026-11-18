# majordomus-covers: start session
# majordomus-negative: start
# start's refusal over an active task says whose it is (I4652). On 2026-11-17 sessions sharing
# one checkout were told only to finish or hand over a task that was another live session's.
# The refusal still exits 15; it now names the episode that opened the task, whether that
# episode is open here, its worker and the task's age, and when the task is another
# session's it names a worktree of the caller's own as the way forward.
. "$ROOT/test/lib.sh"
unset MAJORDOMUS_PROVIDER_SESSION CLAUDE_CODE_SESSION_ID MJ_SESSION_KEY
MAJORDOMUS_RUNTIME=off; export MAJORDOMUS_RUNTIME

fixture() {
  mkdir -p "$1"; cd "$1" || exit 1
  git init -q .; git config user.email t@example.com; git config user.name Test
  "$MJ" init >/dev/null; "$MJ" update >/dev/null
  mkdir -p lib; echo a > lib/a; git add .; git commit -qm base
}
a() { MAJORDOMUS_PROVIDER_SESSION=worker-a-4652 USER=shared-user "$@"; }
b() { MAJORDOMUS_PROVIDER_SESSION=worker-b-4652 USER=shared-user "$@"; }

# ---------------------------------------------------------------- another open session's task
(
  fixture "$T/two-sessions"
  MAJORDOMUS_NOW=2026-11-17T10:00:00Z; export MAJORDOMUS_NOW
  expect_exit 0 "$MJ" session start --provider-session worker-a-4652 --worker worker-a
  sid_a="$(sed -n 's/^session_id: //p' .ai/local/state/sessions-open/worker-a-4652.yaml)"
  [ -n "$sid_a" ] || { echo "    A has no episode"; exit 1; }
  expect_exit 0 a "$MJ" start "A's work" --scope lib
  expect_exit 0 "$MJ" session start --provider-session worker-b-4652 --worker worker-b
  MAJORDOMUS_NOW=2026-11-17T12:30:00Z; export MAJORDOMUS_NOW
  expect_exit 15 b "$MJ" start "B's work" --scope lib
  expect_grep "task t-[0-9a-f-]+ is active \\('A's work'\\), started 2h ago by session $sid_a \\(open here, worker worker-a, provider session worker-a-4652\\)"
  expect_grep "another session's work"
  expect_grep 'worktree of its own: majordomus worktree create <branch>'
  grep -q "^task: \"A's work\"" .ai/local/state/current.yaml \
    || { echo "    the refusal changed A's task"; exit 1; }

  # --- the same task, asked by the session that opened it: close it, nothing about another
  expect_exit 15 a "$MJ" start "A's next work" --scope lib
  expect_grep "by session $sid_a"
  expect_grep "it is this session's task: run majordomus finish \\(or majordomus handover --close\\) first"
  expect_no_grep "another session"
  expect_no_grep 'worktree create'
) || exit 1

# ---------------------------------------------------------------- no episode names the opener
(
  fixture "$T/no-episode"
  expect_exit 0 "$MJ" start "nobody's work" --scope lib
  expect_exit 15 "$MJ" start "more work" --scope lib
  expect_grep "is active \\('nobody's work'\\), started .* by a session the ledger does not name"
  expect_grep 'if the task is yours or stranded, run majordomus finish'
  expect_grep 'start yours in a worktree of its own: majordomus worktree create <branch>'
) || exit 1
exit 0
