# majordomus-covers: check
# majordomus-negative: check
# A task that fast-forwards or rebases onto the trunk has not touched what the trunk
# brought. Case 108 proves this for a merge; a fast-forward and a rebase bring the trunk's
# commits in without a merge commit, so they sit on this line's first parent and the scope
# doctrine counted every file they changed as the task's. An adopting repository (OSCILLA)
# met it on every `git pull --ff-only` after `majordomus start`: the pre-push hook refused
# "outside claimed scope" for work the trunk had already integrated, and the only way out
# was to close the task and start another.
#
# The rule: a commit the trunk's remote-tracking branch already holds, and that this
# checkout did not make, is not the task's. A commit the task made and pushed is still
# its own, so the doctrine loses no teeth.
. "$ROOT/test/lib.sh"
"$MJ" init >/dev/null; "$MJ" update >/dev/null
mkdir -p lib/auth deploy && echo x > lib/auth/a.txt && echo d > deploy/Dockerfile && git add . && git commit -qm base
trunk="$(git branch --show-current)"

# the remote and a second clone that pushes other people's work. Both live inside the git
# directory, so neither is part of the working tree the doctrine reads.
GD="$(cd "$(git rev-parse --git-dir)" && pwd -P)"
git init -q --bare "$GD/remote.git"
git remote add origin "$GD/remote.git"
git push -q origin "$trunk"
git remote set-head origin -a >/dev/null
git branch -q --set-upstream-to="origin/$trunk"
git clone -q "$GD/remote.git" "$GD/other" 2>/dev/null
git -C "$GD/other" config user.email o@example.com; git -C "$GD/other" config user.name o
others() { # $1 file, $2 content: somebody else's commit, on the trunk
  printf '%s\n' "$2" > "$GD/other/$1"
  git -C "$GD/other" add -A && git -C "$GD/other" commit -qm "other: $1"
  git -C "$GD/other" pull -q --rebase && git -C "$GD/other" push -q origin "$trunk"
}

# ---------------------------------------------------------------- a fast-forward on the trunk
expect_exit 0 "$MJ" start "fix auth on the trunk" --scope lib/auth --owner alice
echo y >> lib/auth/a.txt && git commit -qam "auth: in scope"
git push -q origin "$trunk"
others fly.toml fly
others deploy/Dockerfile v2
git pull -q --ff-only
# fly.toml and deploy/Dockerfile arrived by a fast-forward; neither is the task's, and the
# task's own pushed commit still is
expect_exit 0 "$MJ" check
expect_grep 'OK +scope .* 1 touched file\(s\), all within scope'
expect_no_grep 'outside claimed scope'
# an escape the task commits and pushes to the trunk is still the task's
echo v3 >> deploy/Dockerfile && git commit -qam "deploy: an escape committed on the trunk"
git push -q origin "$trunk"
expect_exit 10 "$MJ" check
expect_grep 'FAIL scope +deploy/Dockerfile — outside claimed scope'
printf '# Objective\n\nfix auth\n\n# Current State\n\nan escape was committed\n\n# Next Action\n\nnone\n' > "$GD/note.md"
"$MJ" finish --outcome partial --note "$GD/note.md" >/dev/null

# ---------------------------------------------------------------- a rebase on a branch
git switch -q -c feature/auth
expect_exit 0 "$MJ" start "fix auth on a branch" --scope lib/auth --owner alice
echo z >> lib/auth/a.txt && git commit -qam "auth: in scope, on the branch"
others deploy/compose.yaml c
git fetch -q origin
git rebase -q "origin/$trunk"
expect_exit 0 "$MJ" check
expect_grep 'OK +scope .* 1 touched file\(s\), all within scope'
expect_no_grep 'outside claimed scope'
# and an escape on the branch is still caught
echo v5 >> deploy/Dockerfile
expect_exit 10 "$MJ" check
expect_grep 'FAIL scope +deploy/Dockerfile — outside claimed scope'
