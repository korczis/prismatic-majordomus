# majordomus-covers: finish
# majordomus-negative: finish
#
# Once a task is finished, the pre-push `finish --check` compares the tree a push publishes
# for this branch with the tree the branch's last finished task verified (I4610). It used to
# return at "already completed" or "no active task" and compare nothing: an integrator
# verified one tree and would have pushed a different HEAD, and nothing said so. A merge of
# the trunk after the verification changes the tree legitimately, so the default is a
# warning that names both trees and what to do; `verification.pushed_tree_unverified`
# raises it to a refusal or silences it.
. "$ROOT/test/lib.sh"
unset MAJORDOMUS_PUSHED
"$MJ" init >/dev/null; "$MJ" update >/dev/null
mkdir -p lib && echo a > lib/a && git add . && git commit -qm base
note() { printf '# Objective\no\n# Current State\nc\n# Next Action\nn\n' | "$MJ" handover >/dev/null; }
branch="$(git symbolic-ref --short HEAD)"

"$MJ" start "t1" --scope lib >/dev/null
id1="$(sed -n 's/^id: //p' .ai/local/state/current.yaml)"
echo b >> lib/a
note
expect_exit 0 "$MJ" finish --outcome completed --verify-command "test -d .ai"
verified="$(sed -n 's/.*"event":"task.finished".*"tree":"\([0-9a-f]*\)".*/\1/p' .ai/local/state/ledger.jsonl | tail -n1)"
[ -n "$verified" ] || { printf '    the finish recorded no verified tree\n'; exit 1; }
git add -A && git commit -qm t1
c1="$(git rev-parse HEAD)"
[ "$(git rev-parse 'HEAD^{tree}')" = "$verified" ] \
  || { printf '    the committed tree is not the verified one; the fixture is wrong\n'; exit 1; }

# The tree that was verified is the tree that is pushed: nothing to say.
expect_exit 0 "$MJ" finish --check
expect_no_grep 'pushed-tree'

# A commit after the verification: the push publishes a tree nobody verified. Warned by
# default, both tree ids named in full, with the task, its command and the two ways out.
echo c >> lib/a && git add -A && git commit -qm after-verify
pushed="$(git rev-parse 'HEAD^{tree}')"
expect_exit 0 "$MJ" finish --check
expect_grep "^WARN pushed-tree +$id1 — "
expect_grep "tree $pushed"
expect_grep "verified tree $verified"
expect_grep 'test -d \.ai'
expect_grep 'Re-run the verification over this tree'
expect_grep 'say why not'
expect_grep "reproduce: git diff --stat $verified $pushed"

# --pushed names what the push publishes: the verified commit is silent, an empty list
# compares nothing, the unverified commit is named.
expect_exit 0 "$MJ" finish --check --pushed "$c1"
expect_no_grep 'pushed-tree'
expect_exit 0 "$MJ" finish --check --pushed ""
expect_no_grep 'pushed-tree'
expect_exit 0 "$MJ" finish --check --pushed "$(git rev-parse HEAD)"
expect_grep "^WARN pushed-tree +$id1 — "
# A hook says the same through MAJORDOMUS_PUSHED, so the line doctor verifies is unchanged;
# set and empty is nothing of this branch, and the flag wins over it.
expect_exit 0 env MAJORDOMUS_PUSHED="" "$MJ" finish --check
expect_no_grep 'pushed-tree'
expect_exit 0 env MAJORDOMUS_PUSHED="$c1" "$MJ" finish --check
expect_no_grep 'pushed-tree'
expect_exit 0 env MAJORDOMUS_PUSHED="$c1 $(git rev-parse HEAD)" "$MJ" finish --check
expect_grep "^WARN pushed-tree +$id1 — "
expect_exit 0 env MAJORDOMUS_PUSHED="$(git rev-parse HEAD)" "$MJ" finish --check --pushed "$c1"
expect_no_grep 'pushed-tree'
# --pushed is a question of --check alone.
expect_exit 2 "$MJ" finish --outcome completed --pushed "$c1"

# The policy decides the level: error refuses the push, off says nothing.
cp .ai/repo/policy.yaml policy.orig
awk '{ print } /^verification:$/ { print "  pushed_tree_unverified: error" }' policy.orig > .ai/repo/policy.yaml
expect_exit 10 "$MJ" finish --check
expect_grep "^FAIL pushed-tree +$id1 — "
awk '{ print } /^verification:$/ { print "  pushed_tree_unverified: off" }' policy.orig > .ai/repo/policy.yaml
expect_exit 0 "$MJ" finish --check
expect_no_grep 'pushed-tree'
cp policy.orig .ai/repo/policy.yaml && rm -f policy.orig

# With no task record at all the ledger still says what the branch last verified.
mv .ai/local/state/current.yaml current.kept
expect_exit 0 "$MJ" finish --check
expect_grep "^WARN pushed-tree +$id1 — "
mv current.kept .ai/local/state/current.yaml

# Another branch's finished task is not this branch's verification.
git checkout -qb elsewhere
expect_exit 0 "$MJ" finish --check
expect_no_grep 'pushed-tree'
git checkout -q "$branch"

# An active task's own finish verifies its tree; the push of work in progress is judged by
# the task's scope and state, as before, and no earlier verification is compared.
"$MJ" start "t2" --scope lib >/dev/null
expect_exit 0 "$MJ" finish --check
expect_no_grep 'pushed-tree'
