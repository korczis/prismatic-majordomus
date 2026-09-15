# majordomus-covers: none
# The reclaim removes what it can prove is spare, and refuses the rest by name.
#
# 212 worktrees, 111 GB, 66 of them on branches already merged: the repository could say what
# was spare (`worktree cleanup`) and could remove one thing at a time (`worktree remove`), and
# nothing joined the two. Creation is automatic — the topology rule, the pre-commit guard,
# `worktree create` — and reclamation was a report plus a command per branch, which is why it
# never finished.
#
# Removing in bulk is only safe if every refusal is real, so this case is mostly refusals. Each
# one is a thing that actually happened in this repository on 2026-09-15:
#
#   ahead of its remote   commits that exist on one disk; `merged into the trunk` is about the
#                         branch's remote history and says nothing about a local head that ran on
#   something inside      two worktrees looked untouched since the 12th and had live processes in
#                         them; mtime lies and only a reading taken now is true
#   cannot tell           no lsof, or git could not compare with the upstream: not knowing is not
#                         the same as nothing there, and the difference is somebody's afternoon
#
# And no branch is ever deleted: the disk was scarce, not the names.
. "$ROOT/test/lib.sh"
[ -f "$ROOT/apps/majordomus-cli/Cargo.toml" ] || { echo "    apps/majordomus-cli/Cargo.toml is missing"; exit 1; }
RB="$(rust_bin)" || rust_bin_exit $?
[ -x "$RB" ] || { echo "    the build produced no executable at $RB"; exit 1; }
command -v jq >/dev/null 2>&1 || { echo "    skip: no jq"; exit 0; }
command -v lsof >/dev/null 2>&1 || { echo "    skip: no lsof, which this case is partly about"; exit 0; }
MAJORDOMUS_SHARE="$ROOT/share"; export MAJORDOMUS_SHARE

S="$T/reclaim"
mkdir -p "$S"
cd "$S" || { echo "    could not enter the fixture"; exit 1; }
git init -q --bare origin.git
git init -q repo
cd repo || exit 1
git config user.email t@t; git config user.name t
git remote add origin ../origin.git
printf 'one\n' > file.txt
git add -A && git commit -qm trunk
git push -q -u origin HEAD:master
git branch -M master

# `majordomus worktree create` puts every worktree at <repo>-wt/<branch>; the container is the
# fixture's own, so nothing here touches the checkout the suite runs in.
mk() {  # mk <branch> <what it does after branching>
  git switch -q -c "$1" master
  printf '%s\n' "$1" > "$1.txt"
  git add -A && git commit -qm "$1"
  git push -q -u origin "$1"
  git switch -q master
  git merge -q --no-ff -m "merge $1" "$1"
  git push -q origin master
  "$RB" worktree create "$1" >/dev/null 2>&1 || { echo "    could not create the worktree of $1"; exit 1; }
}
mk spare-one
mk ahead-of-remote
mk occupied
mk dirty-tree

# the four states, each made after the worktree exists
# Merged into the trunk *and* ahead of its remote, which is the combination that makes this
# refusal necessary: the branch's merged history is on origin, the commit on top of it is on
# this disk only, and `merged into the trunk` cannot see the difference. 9e met exactly this
# on 2026-09-15 — fix/the-order-ratchet-is-paid-not-moved, local bd4176282 against remote
# eb7f1fad2, in a worktree nobody claimed.
ahead_path="$("$RB" worktree path ahead-of-remote)"
( cd "$ahead_path" && printf 'local only\n' > local-only.txt \
  && git add -A && git -c user.email=t@t -c user.name=t commit -qm "on one disk only" ) \
  || { echo "    could not put a local-only commit on ahead-of-remote"; exit 1; }
git merge -q --no-ff -m "merge the local-only commit" ahead-of-remote \
  || { echo "    could not merge the local-only commit into the trunk"; exit 1; }
git push -q origin master   # the trunk goes to the remote; the branch deliberately does not
sleep 300 & SLEEPER=$!
trap 'kill "$SLEEPER" 2>/dev/null; :' EXIT
occ="$("$RB" worktree path occupied)"
( cd "$occ" && sleep 300 ) & INSIDE=$!
trap 'kill "$SLEEPER" "$INSIDE" 2>/dev/null; :' EXIT
sleep 1
printf 'uncommitted\n' >> "$("$RB" worktree path dirty-tree)/dirty-tree.txt"

# --- what the listing offers, before anything is removed
"$RB" worktree cleanup --format json > "$T/list.json" 2>/dev/null || true
jq -e '[.[] | select(.name == "spare-one")] | length == 1' "$T/list.json" >/dev/null \
  || { echo "    the listing does not offer the branch that is merged, clean and pushed"; exit 1; }

# --- the reclaim
"$RB" worktree cleanup --remove --format json > "$T/out.json" 2>"$T/out.err" \
  || { echo "    cleanup --remove exited non-zero:"; tail -3 "$T/out.err" | sed 's/^/      /'; exit 1; }

removed() { jq -r --arg b "$1" '[.removed[] | select(. == $b)] | length' "$T/out.json"; }
refused() { jq -r --arg b "$1" '[.refused[] | select(.branch == $b)] | length' "$T/out.json"; }
why()     { jq -r --arg b "$1" '[.refused[] | select(.branch == $b)][0].reason // ""' "$T/out.json"; }

[ "$(removed spare-one)" = 1 ] \
  || { echo "    the spare worktree was not removed"; cat "$T/out.json"; exit 1; }
[ -d "$("$RB" worktree path spare-one)" ] && { echo "    it was reported removed and is still there"; exit 1; }
echo "    a worktree whose branch is merged, clean and pushed is removed"

[ "$(refused ahead-of-remote)" = 1 ] || { echo "    a branch ahead of its remote was not refused"; exit 1; }
case "$(why ahead-of-remote)" in
  *ahead*) ;;
  *) echo "    it was refused for the wrong reason: $(why ahead-of-remote)"; exit 1 ;;
esac
[ -d "$("$RB" worktree path ahead-of-remote)" ] \
  || { echo "    it was refused and removed anyway"; exit 1; }
echo "    a branch whose commits reach no remote is refused, with the count"

[ "$(refused occupied)" = 1 ] || { echo "    a worktree with a process inside was not refused"; exit 1; }
case "$(why occupied)" in
  *working\ directory\ inside*) ;;
  *) echo "    it was refused for the wrong reason: $(why occupied)"; exit 1 ;;
esac
[ -d "$occ" ] || { echo "    the occupied worktree was removed"; exit 1; }
echo "    a worktree something is working in is refused, measured now rather than from its mtime"

[ "$(removed dirty-tree)" = 0 ] || { echo "    a worktree with uncommitted work was removed"; exit 1; }
echo "    uncommitted work is never removed"

# --- and the names survive
for b in spare-one ahead-of-remote occupied dirty-tree; do
  git show-ref --verify --quiet "refs/heads/$b" \
    || { echo "    branch $b was deleted; the reclaim is about disk, not names"; exit 1; }
done
echo "    no branch was deleted: the disk was what was scarce"

echo "    the reclaim removes what it can prove is spare, and refuses the rest by name"
