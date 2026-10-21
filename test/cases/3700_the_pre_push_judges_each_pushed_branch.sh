# majordomus-timeout: 300
# majordomus-covers: start check finish
# The pre-push contract judges the branches a push carries (I1990, prepush_refs_case; ADR 0105,
# amendment of 2026-10-15). Each pushed refs/heads/<b> is judged by the task of the checkout
# that has <b> checked out, and this checkout's task only of its own branch. Before, every push
# asked `finish --check` of the pushing checkout's task, so pushing branch X from the checkout
# of branch Y was judged by Y's scope, and an adopter pushed feature branches from its main
# checkout to get round it.
. "$ROOT/test/lib.sh"
"$MJ" init >/dev/null; "$MJ" update >/dev/null
mkdir -p lib other && echo a > lib/a && echo o > other/o && git add . && git commit -qm base
ZERO=0000000000000000000000000000000000000000
nocommit() { git -c core.hooksPath=/dev/null "$@"; }

# checkout Y: branch y, its task scoped to lib
git checkout -q -b y
"$MJ" start "the task of y" --scope lib >/dev/null

# checkout X: a linked worktree on branch x, its own task scoped to other
wt="$T-x"; rm -rf "$wt"
git worktree add -q -b x "$wt"
"$MJ" --repo "$wt" start "the task of x" --scope other >/dev/null
( cd "$wt" && echo lib >> lib/a && nocommit commit -qam "x touches lib, outside its scope" )
X="$(git rev-parse x)"
pushed="$T-pushed"

# pushing x from Y: x's own checkout judges it, and lib/a is outside x's scope
printf 'refs/heads/x %s refs/heads/x %s\n' "$X" "$ZERO" > "$pushed"
expect_exit 10 "$MJ" finish --check --pushed "$pushed"
expect_grep 'pushed branch x is refused by the task of'
expect_grep 'FAIL scope +lib/a'

# with x's work inside its scope the same push is admitted, and Y's task is not asked:
# Y holds an out-of-scope change of its own that would refuse Y's own check
( cd "$wt" && nocommit reset -q --hard HEAD~1 && echo other >> other/o && nocommit commit -qam "x stays in other" )
X="$(git rev-parse x)"
printf 'refs/heads/x %s refs/heads/x %s\n' "$X" "$ZERO" > "$pushed"
echo y-out >> other/o
expect_exit 10 "$MJ" check
expect_grep 'FAIL scope +other/o'
expect_exit 0 "$MJ" finish --check --pushed "$pushed"
expect_no_grep 'refused'
expect_no_grep 'FAIL scope +other/o'

# pushing y from Y asks Y's task, and its out-of-scope change refuses it, naming y
printf 'refs/heads/y %s refs/heads/y %s\n' "$(git rev-parse y)" "$ZERO" > "$pushed"
expect_exit 10 "$MJ" finish --check --pushed "$pushed"
expect_grep "pushed branch y is refused by this checkout's task"
git checkout -q -- other/o

# a branch no checkout holds is admitted, named; a deleted ref and refs/majordomus/* are skipped
git branch -q z
{ printf 'refs/heads/z %s refs/heads/z %s\n' "$(git rev-parse z)" "$ZERO"
  printf '(delete) %s refs/heads/gone %s\n' "$ZERO" "$X"
  printf 'refs/majordomus/h %s refs/majordomus/h %s\n' "$X" "$ZERO"; } > "$pushed"
expect_exit 0 "$MJ" finish --check --pushed "$pushed"
expect_grep 'pushed branch z: no checkout holds it'
expect_no_grep 'gone'

# --pushed is a question about a push: it goes with --check only
expect_exit 2 "$MJ" finish --pushed "$pushed"
expect_grep 'goes with --check'
git worktree remove --force "$wt"
