# majordomus-covers: none
# scripts/unblock takes the conflict off a branch that only GitHub thinks is conflicted: it
# merges the trunk in where the derived merge driver exists, regenerates the projections,
# and pushes the branch. What must hold is not that a merge succeeds — case 57 already holds
# the driver to that, over the real .gitattributes — but that this tool refuses everything
# it cannot decide, and leaves nothing behind when it does.
#
# A conflict on an authored file belongs to whoever wrote it: the merge is aborted and the
# files are named rather than resolved, because a tool that resolved them would be the union
# merge `project.land-and-publish` clause 3 forbids, wearing a better name. And every path
# out of the script, the refusals included, removes the scratch worktree, leaves the branch's
# own checkout alone, and pushes nothing it did not say it was pushing.
#
# The repository here is a small one built in the case, with a local bare repository as its
# remote: no network, no GitHub, and a second of setup rather than a clone of this checkout.
# The derive is stubbed through UNBLOCK_DERIVE — a real one is three to six minutes, and
# everything this script decides happens on either side of it.
. "$ROOT/test/lib.sh"

S="$(mktemp -d "${TMPDIR:-/tmp}/mj110.XXXXXX")"; trap 'rm -rf "$S"' EXIT

# ---------------------------------------------------------------- the declaration
# The driver command is declared once, by the `derive-merge-driver` recipe. A copy inside
# this script would be a second declaration of one fact, and the day the two disagreed would
# be the day a merge silently stopped resolving.
if grep -q 'git config merge.derived.driver' "$ROOT/scripts/unblock"; then
  echo "    scripts/unblock writes merge.derived.driver itself instead of running the recipe that declares it"; exit 1
fi
grep -q 'derive-merge-driver' "$ROOT/scripts/unblock" || {
  echo "    scripts/unblock does not name the recipe that declares the merge driver"; exit 1; }
JF="$(just_declaration)"
grep -qE '^unblock ' "$JF" || { echo "    no workflow declares an unblock recipe"; exit 1; }

# ---------------------------------------------------------------- usage
expect_exit 2 "$ROOT/scripts/unblock"
expect_grep 'name the branch'
expect_exit 2 "$ROOT/scripts/unblock" --nope
expect_grep 'unknown option --nope'
expect_exit 2 "$ROOT/scripts/unblock" one two
expect_grep 'one branch at a time'
expect_exit 0 "$ROOT/scripts/unblock" --help
expect_grep 'merge master in, derive, commit, push'

# ---------------------------------------------------------------- a repository with a remote
# The script is what it is because of where it lives: it takes its root from its own path,
# so the fixture is a repository with the two scripts in it and one marked derived file.
W="$S/work"
mkdir -p "$W/scripts" "$W/docs"
cp "$ROOT/scripts/unblock" "$ROOT/scripts/merge-derived" "$W/scripts/"
chmod +x "$W/scripts/unblock" "$W/scripts/merge-derived"
git init -q --bare "$S/remote.git"
git init -q "$W"
cd "$W" || exit 1
git symbolic-ref HEAD refs/heads/master
git config user.email t@example.com
git config user.name t
git config merge.derived.name "derived"
git config merge.derived.driver "$W/scripts/merge-derived %O %A %B %P"
git remote add origin "$S/remote.git"
printf 'docs/PLAN_STATUS.md merge=derived\n' > .gitattributes
printf 'Generated from canonical inputs `aaaaaaaaaaaa`.\n' > docs/PLAN_STATUS.md
printf 'the README nobody has touched yet\n' > README.md
git add -A && git commit -qm base
git push -q origin HEAD:refs/heads/master
base="$(git rev-parse HEAD)"

# a branch that moved the derived file, and a trunk that moved the same line of it — the
# shape of the reported conflict. No textual merge of the two is possible, so a merge that
# succeeds here is one the driver resolved.
git checkout -q -b feature/derived
printf 'Generated from canonical inputs `bbbbbbbbbbbb`.\n' > docs/PLAN_STATUS.md
git commit -qam "the branch regenerated it"
git push -q origin HEAD:refs/heads/feature/derived
branch_tip="$(git rev-parse HEAD)"
git checkout -q master
printf 'Generated from canonical inputs `cccccccccccc`.\n' > docs/PLAN_STATUS.md
git commit -qam "master regenerated it"
git push -q origin HEAD:refs/heads/master
trunk_tip="$(git rev-parse HEAD)"
git branch -q -D feature/derived

# ---------------------------------------------------------------- it unblocks the branch
expect_exit 0 env UNBLOCK_DERIVE=true "$W/scripts/unblock" feature/derived
expect_grep "now carries origin/master"
git fetch -q origin feature/derived
git merge-base --is-ancestor "$trunk_tip" FETCH_HEAD || {
  echo "    the pushed branch does not contain the trunk"; exit 1; }
git merge-base --is-ancestor "$branch_tip" FETCH_HEAD || {
  echo "    the pushed branch is not a descendant of what the branch was"; exit 1; }
git show FETCH_HEAD:docs/PLAN_STATUS.md | grep -q bbbbbbbbbbbb || {
  echo "    the derived file was not resolved to the branch's side"; exit 1; }

# nothing was checked out, nothing was left behind, and the checkout it ran from is as it was
[ "$(git rev-parse --abbrev-ref HEAD)" = master ] || { echo "    unblock moved HEAD"; exit 1; }
if git rev-parse --verify -q refs/heads/feature/derived >/dev/null; then
  echo "    unblock created a local branch instead of working detached"; exit 1
fi
[ "$(git worktree list | wc -l | tr -d ' ')" = 1 ] || {
  echo "    unblock left a scratch worktree behind:"; git worktree list | sed 's/^/    | /'; exit 1; }
[ -z "$(git status --porcelain --untracked-files=no)" ] || {
  echo "    unblock dirtied the working tree it ran from"; exit 1; }

# ---------------------------------------------------------------- and says so when there is nothing to do
expect_exit 0 env UNBLOCK_DERIVE=true "$W/scripts/unblock" feature/derived
expect_grep 'already contains origin/master'

# ---------------------------------------------------------------- an authored conflict is refused
git checkout -q -b feature/authored "$base"
printf 'the branch wrote this\n' > README.md
git commit -qam "the branch edited the README"
git push -q origin HEAD:refs/heads/feature/authored
authored_tip="$(git rev-parse HEAD)"
git checkout -q master
printf 'master wrote this\n' > README.md
git commit -qam "master edited the README"
git push -q origin HEAD:refs/heads/master
git branch -q -D feature/authored

expect_exit 10 env UNBLOCK_DERIVE=true "$W/scripts/unblock" feature/authored
expect_grep "authored file"
expect_grep 'README.md'
git fetch -q origin feature/authored
[ "$(git rev-parse FETCH_HEAD)" = "$authored_tip" ] || {
  echo "    a refused run moved the branch"; exit 1; }
[ "$(git worktree list | wc -l | tr -d ' ')" = 1 ] || {
  echo "    a refused run left a scratch worktree behind"; exit 1; }

# ---------------------------------------------------------------- a dry run pushes nothing
git checkout -q -b feature/dry "$base"
printf 'Generated from canonical inputs `dddddddddddd`.\n' > docs/PLAN_STATUS.md
git commit -qam "the branch regenerated it"
git push -q origin HEAD:refs/heads/feature/dry
dry_tip="$(git rev-parse HEAD)"
git checkout -q master
git branch -q -D feature/dry

expect_exit 0 env UNBLOCK_DERIVE=true "$W/scripts/unblock" feature/dry --dry-run
expect_grep 'dry run: would commit'
git fetch -q origin feature/dry
[ "$(git rev-parse FETCH_HEAD)" = "$dry_tip" ] || { echo "    a dry run pushed the branch"; exit 1; }

# ---------------------------------------------------------------- what it will not touch
expect_exit 10 env UNBLOCK_DERIVE=true "$W/scripts/unblock" master
expect_grep 'not a branch to unblock'
expect_exit 12 env UNBLOCK_DERIVE=true "$W/scripts/unblock" feature/no-such-branch
expect_grep 'no branch feature/no-such-branch'
