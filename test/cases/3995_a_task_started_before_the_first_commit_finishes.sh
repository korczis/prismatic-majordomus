# majordomus-covers: start check finish
# A task started before the first commit finishes (I3995). In a repository with no commit,
# `start` records the head as NONE. After the first commits the state doctrine asked
# `git merge-base --is-ancestor NONE HEAD`, which fails for a name that is no revision, so
# the task read as diverged and every `check` and completed `finish` was refused for good.
# An empty history is an ancestor of every commit on its branch: the label is `advanced`.
. "$ROOT/test/lib.sh"

R="$T/unborn"
mkdir -p "$R"
cd "$R" || exit 1
git init -q .
git config user.email t@example.com
git config user.name t
"$MJ" init >/dev/null
"$MJ" update >/dev/null

# the task begins on an unborn branch, and records it so
expect_exit 0 "$MJ" start "the first work" --scope .
grep -q '^head: NONE' .ai/local/state/current.yaml \
  || { echo "    start did not record the unborn head as NONE"; exit 1; }

# two commits later the recorded head is an ancestor of HEAD, not a rewritten history
mkdir -p lib && echo a > lib/a && git add -A && git commit -qm first
echo b >> lib/a && git commit -qam second
expect_exit 0 "$MJ" check
expect_no_grep 'diverged'
expect_grep 'advanced'

printf '# Objective\no\n# Current State\nc\n# Next Action\nn\n' > "$T/note.md"
expect_exit 0 "$MJ" finish --outcome completed --verify-command "test -f lib/a" --note "$T/note.md"
expect_grep 'OK +state .*advanced'
grep -q '^outcome: completed' .ai/local/state/current.yaml \
  || { echo "    finish did not record the task completed"; exit 1; }

# the next start is not refused by the closed task
expect_exit 0 "$MJ" start "the second work" --scope lib
