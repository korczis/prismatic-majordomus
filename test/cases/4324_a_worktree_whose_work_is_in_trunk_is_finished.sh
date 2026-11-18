# majordomus-covers: none
# A worktree whose work is already in the trunk is offered by `worktree cleanup` and removed by
# `--remove`, however the work got there; one with a single byte the trunk lacks is not.
#
# On 2026-11-16 one machine held about 1,500 worktrees across six repositories. 794 were merged
# by ancestry and clean, and went with a plain `git worktree remove`. 205 more, about 44 GiB,
# held nothing the trunk lacked and were offered by nothing:
#
#   squash-merged      the forge squashed the branch, so its tip is no ancestor of the trunk;
#                      `git merge-tree --write-tree <trunk> <branch>` writes the trunk's own
#                      tree, which is the proof that merging it would change nothing
#   gone upstream      merged on the forge and deleted there; every commit it holds is on the
#                      remote through the trunk, and the reclaim refused it as "gone"
#   trunk-held changes uncommitted records that landed through another branch: `git status`
#                      is not empty, and every path holds exactly what the trunk holds
#
# They were found by those three readings and removed by hand with --force. This case is that
# hand made into the tool, with the refusals that keep it safe: a byte the trunk lacks, in a
# commit or in the work tree, keeps the worktree, and no branch is ever deleted.
. "$ROOT/test/lib.sh"
[ -f "$ROOT/apps/majordomus-cli/Cargo.toml" ] || { echo "    apps/majordomus-cli/Cargo.toml is missing"; exit 1; }
RB="$(rust_bin)" || rust_bin_exit $?
[ -x "$RB" ] || { echo "    the build produced no executable at $RB"; exit 1; }
command -v jq >/dev/null 2>&1 || skip "no jq"
command -v lsof >/dev/null 2>&1 || skip "no lsof, which the reclaim needs to say nothing runs inside"
git merge-tree --write-tree HEAD HEAD >/dev/null 2>&1 || skip "git without merge-tree --write-tree (2.38+)"
MAJORDOMUS_SHARE="$ROOT/share"; export MAJORDOMUS_SHARE

S="$T/finished"
mkdir -p "$S"
cd "$S" || { echo "    could not enter the fixture"; exit 1; }
git init -q --bare origin.git
git init -q repo
cd repo || exit 1
git config user.email t@t; git config user.name t
git remote add origin ../origin.git
printf 'one\n' > file.txt
git add -A && git commit -qm trunk
git branch -M master
git push -q -u origin master

branch() {  # branch <name>: one commit of its own, pushed
  git switch -q -c "$1" master
  printf '%s\n' "$1" > "$1.txt"
  git add -A && git commit -qm "$1"
  git push -q -u origin "$1"
  git switch -q master
}

# squash-merged on the forge, its remote branch deleted afterwards
branch squashed
git merge -q --squash squashed && git commit -qm "squash squashed" && git push -q origin master
# merged by ancestry; its worktree will hold changes the trunk already has
branch held
git merge -q --no-ff -m "merge held" held && git push -q origin master
# merged by ancestry; its worktree will hold one file the trunk lacks
branch real-work
git merge -q --no-ff -m "merge real-work" real-work && git push -q origin master
# merged by ancestry, then deleted on the forge: every commit is on the remote's trunk
branch gone-merged
git merge -q --no-ff -m "merge gone-merged" gone-merged && git push -q origin master
# never merged, deleted on the forge: its commit is on this disk only
branch gone-open
# never merged, still on the forge
branch open

for b in squashed held real-work gone-merged gone-open open; do
  "$RB" worktree create "$b" >/dev/null 2>&1 || { echo "    could not create the worktree of $b"; exit 1; }
done
git push -q origin --delete squashed gone-merged gone-open && git fetch -q --prune origin \
  || { echo "    could not delete the remote branches"; exit 1; }

# the records that landed on the trunk through another branch, then the same bytes, uncommitted,
# in the worktree of held (one new file, one edited tracked file)
printf 'late\n' > late.txt
printf 'held, edited on the trunk\n' > held.txt
git add -A && git commit -qm "records" && git push -q origin master
held_path="$("$RB" worktree path held)"
printf 'late\n' > "$held_path/late.txt"
printf 'held, edited on the trunk\n' > "$held_path/held.txt"
printf 'mine alone\n' > "$("$RB" worktree path real-work)/mine.txt"

# --- the listing
"$RB" worktree cleanup --format json > "$T/list.json" 2>"$T/list.err" \
  || { echo "    cleanup exited non-zero:"; tail -3 "$T/list.err" | sed 's/^/      /'; exit 1; }
listed() { jq -r --arg b "$1" '[.[] | select(.name == $b)] | length' "$T/list.json"; }
field()  { jq -r --arg b "$1" --arg f "$2" '[.[] | select(.name == $b)][0][$f] // "absent"' "$T/list.json"; }

[ "$(listed squashed)" = 1 ] \
  || { echo "    a squash-merged, clean worktree is not offered"; cat "$T/list.json"; exit 1; }
[ "$(field squashed landed_by_content)" = true ] \
  || { echo "    squashed is offered without saying it landed by content"; exit 1; }
echo "    a squash-merged worktree is offered: merging it would change nothing"

[ "$(listed held)" = 1 ] \
  || { echo "    a merged worktree whose uncommitted changes are all in the trunk is not offered"; cat "$T/list.json"; exit 1; }
[ "$(field held uncommitted_in_trunk)" = true ] \
  || { echo "    held is offered without saying its changes are in the trunk"; exit 1; }
echo "    a worktree whose every uncommitted change is in the trunk is offered"

for b in real-work gone-open open; do
  [ "$(listed "$b")" = 0 ] || { echo "    $b holds work the trunk lacks and was offered"; exit 1; }
done
echo "    a byte the trunk lacks, committed or not, keeps a worktree off the list"

# --- the reclaim
"$RB" worktree cleanup --remove --format json > "$T/out.json" 2>"$T/out.err" \
  || { echo "    cleanup --remove exited non-zero:"; tail -3 "$T/out.err" | sed 's/^/      /'; exit 1; }
removed() { jq -r --arg b "$1" '[.removed[] | select(. == $b)] | length' "$T/out.json"; }
why()     { jq -r --arg b "$1" '[.refused[] | select(.branch == $b)][0].reason // ""' "$T/out.json"; }

for b in squashed held gone-merged; do
  [ "$(removed "$b")" = 1 ] || { echo "    $b was not removed: $(why "$b")"; cat "$T/out.json"; exit 1; }
  [ -d "$("$RB" worktree path "$b")" ] && { echo "    $b was reported removed and is still there"; exit 1; }
done
echo "    squashed, trunk-held and gone-but-merged worktrees are removed"

for b in real-work gone-open open; do
  [ "$(removed "$b")" = 0 ] || { echo "    $b was removed with work the trunk lacks"; exit 1; }
  [ -d "$("$RB" worktree path "$b")" ] || { echo "    the worktree of $b is gone"; exit 1; }
done
[ -f "$("$RB" worktree path real-work)/mine.txt" ] || { echo "    the uncommitted file of real-work is gone"; exit 1; }
echo "    every worktree holding work the trunk lacks is kept, its files with it"

# --- and the names survive
for b in squashed held real-work gone-merged gone-open open; do
  git show-ref --verify --quiet "refs/heads/$b" \
    || { echo "    branch $b was deleted; the reclaim is about disk, not names"; exit 1; }
done
echo "    no branch was deleted"

echo "    a worktree whose work is in the trunk is finished, however the work got there"
