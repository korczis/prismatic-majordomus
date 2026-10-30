# A new worktree starts from the remote's trunk as it is now, not from the local trunk. The
# primary checkout's local master is never advanced and was 779 commits behind origin/master
# on 2026-10-16; `worktree create` cut every new branch from it, said "(new, from master)",
# and every worker reset by hand. A branch that exists only on the remote was started again
# from that stale master instead of being taken up. `create` now fetches the trunk and the
# branch from the trunk's remote, bounded and quiet: a branch the remote has is checked out
# where it stands and tracks it, any other starts at <remote>/<trunk> and the printed line
# names the exact commit. A remote that cannot be read is said, with how far behind the local
# trunk is, and never silently becomes the start.
. "$ROOT/test/lib.sh"
RB="$(rust_bin)" || rust_bin_exit $?

# Nested one level down so the container (<repo>-wt) stays under $T; resolved, because git
# reports the resolved path and on macOS that is not the $TMPDIR spelling.
mkdir -p "$T/wt"; W="$(cd "$T/wt" && pwd -P)"; R="$W/repo"; O="$W/origin.git"; P="$W/peer"
fixture_repo "$R" >/dev/null
gitc() { git -c user.email=t@example.com -c user.name=t -c core.hooksPath=/dev/null "$@"; }
git -C "$R" init -q -b master .
git -C "$R" add -A >/dev/null
gitc -C "$R" commit -qm fixture >/dev/null
git init -q --bare -b master "$O"
git -C "$R" remote add origin "$O"
git -C "$R" push -q origin master 2>/dev/null
git -C "$R" fetch -q origin
git -C "$R" remote set-head origin master >/dev/null

mj() { local cwd="$1"; shift; ( cd "$cwd" && MAJORDOMUS_SHARE="$R/share" "$RB" "$@" ); }

# The remote moves on without the primary checkout: two commits on its trunk and a branch
# somebody else pushed, none of them fetched here.
git clone -q "$O" "$P" 2>/dev/null
gitc -C "$P" commit -q --allow-empty -m "landed one"
gitc -C "$P" commit -q --allow-empty -m "landed two"
git -C "$P" push -q origin master 2>/dev/null
gitc -C "$P" checkout -q -b fix/pushed-elsewhere
gitc -C "$P" commit -q --allow-empty -m "their work"
git -C "$P" push -q origin fix/pushed-elsewhere 2>/dev/null
remote_trunk="$(git -C "$P" rev-parse master)"
their_head="$(git -C "$P" rev-parse fix/pushed-elsewhere)"
local_trunk="$(git -C "$R" rev-parse master)"
[ "$remote_trunk" != "$local_trunk" ] || { echo "    the fixture's remote did not move"; exit 1; }

# ---------------------------------------------------------------- 1. new work: the remote trunk
expect_exit 0 mj "$R" worktree create feature/new-work
expect_grep "branch  feature/new-work [(]new, from origin/master at ${remote_trunk}[)]"
expect_no_grep "warning"
[ "$(git -C "$R-wt/feature/new-work" rev-parse HEAD)" = "$remote_trunk" ] \
  || { echo "    the new branch starts at $(git -C "$R-wt/feature/new-work" rev-parse HEAD), not the remote trunk $remote_trunk"; exit 1; }
# it does not track the trunk: its upstream is its own branch, set on its first push
[ -z "$(git -C "$R" config --get branch.feature/new-work.merge)" ] \
  || { echo "    the new branch tracks $(git -C "$R" config --get branch.feature/new-work.merge)"; exit 1; }
# the local trunk is not moved: the primary checkout is somebody's working tree
[ "$(git -C "$R" rev-parse master)" = "$local_trunk" ] || { echo "    the local trunk was moved"; exit 1; }
# the report names the start and the commit for a reader of the JSON
expect_exit 0 mj "$R" worktree create feature/new-json --format json
expect_grep '"from": *"remote_trunk"'
expect_grep "\"start_commit\": *\"$remote_trunk\""

# ---------------------------------------------------------------- 2. a pushed branch: taken up
expect_exit 0 mj "$R" worktree create fix/pushed-elsewhere
expect_grep "branch  fix/pushed-elsewhere [(]from origin/fix/pushed-elsewhere at $their_head, tracking it[)]"
[ "$(git -C "$R-wt/fix/pushed-elsewhere" rev-parse HEAD)" = "$their_head" ] \
  || { echo "    the pushed branch was started again at $(git -C "$R-wt/fix/pushed-elsewhere" rev-parse HEAD)"; exit 1; }
[ "$(git -C "$R-wt/fix/pushed-elsewhere" rev-parse --abbrev-ref '@{upstream}')" = origin/fix/pushed-elsewhere ] \
  || { echo "    the pushed branch does not track its remote branch"; exit 1; }

# ---------------------------------------------------------------- 3. --base is as given
expect_exit 0 mj "$R" worktree create feature/from-local --base master
expect_grep "branch  feature/from-local [(]new, from master at ${local_trunk}[)]"
expect_no_grep "warning"

# ---------------------------------------------------------------- 4. unreachable: said, with the distance
git -C "$R" remote set-url origin "$W/gone.git"
expect_exit 0 mj "$R" worktree create feature/offline
expect_grep "branch  feature/offline [(]new, from master at ${local_trunk}[)]"
expect_grep "warning could not fetch master from origin"
expect_grep "started from the local master, 2 commits behind origin/master as last fetched"
expect_grep "git fetch origin && git reset --hard origin/master"
[ "$(git -C "$R-wt/feature/offline" rev-parse HEAD)" = "$local_trunk" ] \
  || { echo "    the offline branch starts at $(git -C "$R-wt/feature/offline" rev-parse HEAD)"; exit 1; }
expect_exit 0 mj "$R" worktree create feature/offline-json --format json
expect_grep '"from": *"local_trunk"'
expect_grep '"unreachable": *"origin"'
expect_grep '"behind": *2'

# ---------------------------------------------------------------- 5. no remote: no fetch, no warning
git -C "$R" remote remove origin
expect_exit 0 mj "$R" worktree create feature/no-remote
expect_grep "branch  feature/no-remote [(]new, from master at ${local_trunk}[)]"
expect_no_grep "warning"
