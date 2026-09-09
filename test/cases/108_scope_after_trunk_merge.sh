# majordomus-covers: check
# majordomus-negative: check
# A task that merges the trunk has not touched what the trunk touched. The scope doctrine
# reads the files this line's own commits changed; a merge commit's second parent is other
# people's work and stays out of the count — otherwise every merge of master would be
# refused as an escape from scope, and the trunk would be merged as rarely as possible.
. "$ROOT/test/lib.sh"
"$MJ" init >/dev/null; "$MJ" update >/dev/null
mkdir -p lib/auth deploy && echo x > lib/auth/a.txt && echo d > deploy/Dockerfile && git add . && git commit -qm base
trunk="$(git branch --show-current)"
git checkout -qb feature/auth
expect_exit 0 "$MJ" start "fix auth" --scope lib/auth --owner alice
# the task's own commit is in scope
echo z >> lib/auth/a.txt && git commit -qam "auth: in scope"
expect_exit 0 "$MJ" check
expect_grep 'OK +scope .* 1 touched file\(s\), all within scope'
# the trunk moves under the task, outside the task's scope
git checkout -q "$trunk"
echo fly > fly.toml && echo v2 > deploy/Dockerfile && git add . && git commit -qm "deploy: the trunk changes files the task never claimed"
git checkout -q feature/auth
git merge -q --no-edit "$trunk"
# the merge brought fly.toml and deploy/Dockerfile into the tree; neither is the task's
expect_exit 0 "$MJ" check
expect_grep 'OK +scope .* 1 touched file\(s\), all within scope'
expect_no_grep 'outside claimed scope'
# an escape after the merge is still caught: the doctrine lost none of its teeth
echo v3 >> deploy/Dockerfile
expect_exit 10 "$MJ" check
expect_grep 'FAIL scope +deploy/Dockerfile — outside claimed scope'
git checkout -q -- deploy/Dockerfile
# a merge commit's conflict resolution is the one edit the walk does not see: say so, so
# that a reader who relies on it finds the limit written down rather than discovered
echo v4 >> deploy/Dockerfile && git commit -qam "deploy: an escape committed on the line"
expect_exit 10 "$MJ" check
expect_grep 'FAIL scope +deploy/Dockerfile — outside claimed scope'
