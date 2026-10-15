# majordomus-covers: finish
# A pushed branch is judged by the task of the checkout that holds it (I1990, evidence
# prepush_refs_case; ADR 0105, amendment of 2026-10-15).
#
# The pre-push hook used to ask `finish --check` of the pushing checkout's task whatever the
# push carried, so pushing branch X from a checkout whose task is on Y failed with
# `different_context`; OSCILLA pushed feature branches from its main checkout to get round it.
# Now the hook passes the refs git hands it and each pushed branch is judged by the task of the
# checkout that has it checked out. Held here, through a real `git push` whose pre-push hook
# hands its standard input to `finish --check --refs -`:
#
#   1  from checkout Y (task on Y, scope lib) push Y and X together, where X's worktree has
#      its own task (scope other) and has changed lib/a: X is refused and named, Y passes
#   2  X's out-of-scope change undone: both are admitted, each judged by its own task, and
#      so from X's linked worktree, where git exports GIT_DIR to the hook
#   3  Y's task is not asked of X: with Y's own tree out of scope, pushing only X is admitted
#   4  a branch no checkout holds is admitted with one line naming it
#   5  a deleted ref and refs/majordomus/* are judged by nobody
. "$ROOT/test/lib.sh"
"$MJ" init >/dev/null; "$MJ" update >/dev/null
mkdir -p lib other && echo a > lib/a && echo o > other/o && git add . && git commit -qm base
Y="$(git branch --show-current)"

remote="$T-remote.git"; rm -rf "$remote"; git init -q --bare "$remote"
git remote add origin "$remote"
hooks="$T-hooks"; rm -rf "$hooks"; mkdir -p "$hooks"
printf '#!/bin/sh\nexec %s finish --check --refs -\n' "$MJ" > "$hooks/pre-push"
chmod +x "$hooks/pre-push"
git config core.hooksPath "$hooks"

# checkout Y: a task on its own branch, scope lib
"$MJ" start "task on Y" --scope lib >/dev/null
echo a2 >> lib/a && git commit -qam "Y works in lib"

# checkout X: a worktree of its own on branch feature-x, with its own task, scope other
wt="$T-wt-x"; rm -rf "$wt"
git worktree add -q -b feature-x "$wt"
WT="$(git -C "$wt" rev-parse --show-toplevel)"
"$MJ" --repo "$wt" start "task on X" --scope other >/dev/null
echo o1 >> "$wt/other/o" && git -C "$wt" commit -qam "X works in other"
echo x >> "$wt/lib/a"

# 1  both pushed from Y: X refused by X's task and named, Y judged by Y's task and passing
expect_exit 1 git push origin "$Y" feature-x
expect_grep "INFO finish +feature-x — judged by the task of its checkout $WT"
expect_grep "INFO finish +$Y — judged by the task of its checkout"
expect_grep 'FAIL scope +lib/a — outside claimed scope \(other\)'
expect_grep 'finish --check: 2 pushed branch\(es\) judged, 1 refused: feature-x$'
expect_no_grep 'different_context|recorded on branch'

# 2  X back within its scope: both admitted, each by its own task
git -C "$wt" checkout -q -- lib/a
expect_exit 0 git push origin "$Y" feature-x
expect_grep 'finish --check: 2 pushed branch\(es\) judged, 0 refused$'
expect_no_grep 'FAIL'
# the same from X's linked worktree, where git exports GIT_DIR to the hook: Y's branch is
# still judged by Y's task in Y's checkout, and X's own by X's
echo o2 >> "$wt/other/o" && git -C "$wt" commit -qam "X once more"
echo a3 >> lib/a && git commit -qam "Y once more"
expect_exit 0 git -C "$wt" push origin "$Y" feature-x
expect_grep "INFO finish +$Y — judged by the task of its checkout $(git rev-parse --show-toplevel)\$"
expect_grep 'finish --check: 2 pushed branch\(es\) judged, 0 refused$'
expect_no_grep 'FAIL|different_context'

# 3  Y's own tree out of its scope does not refuse a push of X alone: Y's task is not asked
echo stray >> other/o
expect_exit 10 "$MJ" finish --check
expect_grep 'FAIL scope +other/o'
echo o3 >> "$wt/other/o" && git -C "$wt" commit -qam "X again"
expect_exit 0 git push origin feature-x
expect_grep 'finish --check: 1 pushed branch\(es\) judged, 0 refused$'
expect_no_grep 'other/o'
git checkout -q -- other/o

# 4  a branch no checkout holds is admitted with a line naming it
git branch held-nowhere
expect_exit 0 git push origin held-nowhere
expect_grep 'INFO finish +held-nowhere — no checkout holds this branch: pushed without a task; admitted'

# 5  a deleted ref and the continuity store are judged by nobody; a checkout without a task
#    admits its branch with a line naming it
expect_exit 0 git push origin --delete held-nowhere
expect_grep 'finish --check: 0 pushed branch\(es\) judged, 0 refused$'
expect_exit 0 "$MJ" finish --check --refs - <<<"refs/majordomus/h/1 $(git rev-parse HEAD) refs/majordomus/h/1 0000000000000000000000000000000000000000"
expect_grep 'judged, 0 refused$'
sed -i.bak 's/^outcome: active$/outcome: partial/' "$wt/.ai/local/state/current.yaml"; rm -f "$wt/.ai/local/state/current.yaml.bak"
expect_exit 0 git push origin feature-x:feature-x-again
expect_grep "INFO finish +feature-x — its checkout $WT has no active task: pushed without a task; admitted"

# --refs is a question asked of --check only
expect_exit 2 "$MJ" finish --outcome completed --refs -
git worktree remove --force "$wt"

# and this repository's own hook hands git's refs to it
grep -qx '  printf .%s. "\$refs" | bin/majordomus finish --check --refs - || exit \$?' \
  "$ROOT/.githooks/pre-push" \
  || { echo "    .githooks/pre-push does not pass the pushed refs to finish --check"; exit 1; }
