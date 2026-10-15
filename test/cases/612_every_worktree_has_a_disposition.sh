# majordomus-covers: none
# Every branch and every worktree is in one state, with one next step, read and never acted on.
#
# On 2026-10-10 this repository held 274 registered worktrees and 297 local branches. Three
# readings said why none of them went away:
#
#   never nominated   `worktree cleanup` offers a branch only when the trunk reaches it and its
#                     worktree is clean; 64 of the 81 merged branches held a dirty one
#   always refused    its reclaim refuses a branch whose upstream is gone, which is the state of
#                     every branch a forge deleted after merging it
#   no state at all   a branch the trunk does not reach was "unmerged", whether its merge would
#                     change nothing, conflict, or it was being worked on that minute
#
# So this case plants one branch in each state git can put it in and asks three things: that
# each is named for what it is, that exactly the ones whose proof is complete are marked
# removable on it, and that the reading removes nothing, naming git's own guarded command for
# each removal instead. Carrying those out is ADR 0127's second half and not this case's.
. "$ROOT/test/lib.sh"
[ -f "$ROOT/apps/majordomus-cli/Cargo.toml" ] || { echo "    apps/majordomus-cli/Cargo.toml is missing"; exit 1; }
RB="$(rust_bin)" || rust_bin_exit $?
[ -x "$RB" ] || { echo "    the build produced no executable at $RB"; exit 1; }
command -v jq >/dev/null 2>&1 || skip "no jq"
command -v lsof >/dev/null 2>&1 || skip "no lsof: occupancy cannot be read, and nothing would be removable"
MAJORDOMUS_SHARE="$ROOT/share"; export MAJORDOMUS_SHARE
# A case run from a git hook inherits the hook's repository; this fixture is its own.
unset GIT_DIR GIT_WORK_TREE GIT_INDEX_FILE GIT_PREFIX

S="$T/reconcile"
mkdir -p "$S"
cd "$S" || { echo "    could not enter the fixture"; exit 1; }
git init -q --bare origin.git
git init -q repo
cd repo || exit 1
git config user.email t@example.com; git config user.name t; git config commit.gpgsign false
git remote add origin ../origin.git
printf 'one\n' > file.txt
git add -A && git commit -qm trunk
git branch -M master
git push -q -u origin master

publish() {  # publish <branch> <file> <text>: one commit on a new branch, pushed
  git switch -q -c "$1" master
  printf '%s\n' "$3" > "$2"
  git add -A && git commit -qm "$1"
  git push -q -u origin "$1"
  git switch -q master
}
land() {  # land <branch>: a merge commit on the trunk, pushed
  git merge -q --no-ff -m "merge $1" "$1" && git push -q origin master
}
tree() {  # tree <branch>: its canonical worktree
  "$RB" worktree create "$1" >/dev/null 2>&1 || { echo "    could not create the worktree of $1"; exit 1; }
}

# landed, and the forge deleted its branch afterwards: the usual state of finished work
publish landed landed.txt landed; land landed; tree landed
git push -q origin --delete landed && git fetch -q --prune origin \
  || { echo "    could not delete the remote branch of landed"; exit 1; }

# landed, and a file nobody committed is still in its worktree
publish dirty dirty.txt dirty; land dirty; tree dirty
printf 'half written\n' > "$("$RB" worktree path dirty)/draft.txt"

# landed, and a process is working inside
publish occupied occupied.txt occupied; land occupied; tree occupied
occ="$("$RB" worktree path occupied)"
( cd "$occ" && sleep 300 ) & INSIDE=$!
trap 'kill "$INSIDE" 2>/dev/null; :' EXIT
sleep 1

# the same change reached the trunk by another commit: unmerged by ancestry, and a no-op
publish same same.txt identical; tree same
printf 'identical\n' > same.txt
git add -A && git commit -qm "the same change, another way" && git push -q origin master

# conflicts with what the trunk says now
publish conflict file.txt theirs
printf 'ours\n' > file.txt
git commit -qam "the trunk moves the same line" && git push -q origin master

# merges cleanly; the trunk moved after it left
publish stale stale.txt stale
printf 'on\n' > moved.txt
git add -A && git commit -qm "the trunk moves on" && git push -q origin master

# a commit no remote has, and a branch nobody has committed to
git switch -q -c local master
printf 'only here\n' > local.txt
git add -A && git commit -qm "only here"
git switch -q master
tree cut
# cut from the remote trunk, which git then records as its upstream: published by nobody
git branch -q --track fresh origin/master

# --- the reading
"$RB" worktree reconcile --format json > "$T/plan.json" 2>"$T/plan.err" \
  || { echo "    reconcile exited non-zero:"; tail -3 "$T/plan.err" | sed 's/^/      /'; exit 1; }
state() { jq -r --arg b "$1" '[.entries[] | select(.branch == $b)][0] | "\(.state) \(.step) \(.automatic)"' "$T/plan.json"; }
want() {  # want <branch> <state step automatic>
  got="$(state "$1")"
  if [ "$got" != "$2" ]; then
    echo "    $1 reads '$got', wanted '$2'"
    jq --arg b "$1" '[.entries[] | select(.branch == $b)][0]' "$T/plan.json" | sed 's/^/      /'
    exit 1
  fi
}
want landed   "merged remove_worktree_and_delete_branch true"
want dirty    "dirty commit false"
want occupied "active keep false"
want same     "equivalent remove_worktree_and_delete_branch true"
want conflict "conflicted resolve false"
want stale    "stale refresh false"
want local    "unpublished publish false"
want cut      "unstarted keep false"
want fresh    "unstarted keep false"
echo "    nine branches, each in the one state git supports"

jq -e '.base == "origin/master" and .automatic == 2 and .settled == false' "$T/plan.json" >/dev/null \
  || { echo "    the verdict is wrong:"; jq '{base, automatic, settled}' "$T/plan.json"; exit 1; }
echo "    decided against the remote trunk; exactly two are removable on proof"

# --- the same reading over MCP, as a client sends it
# The server wants a layer to index; the reading itself is about git, so the four files it
# opens are enough, and they sit untracked in the primary checkout, which is not a subject.
mkdir -p .ai/repo/knowledge
cp "$ROOT/.ai/manifest.yaml" .ai/
cp "$ROOT/.ai/repo/policy.yaml" "$ROOT/.ai/repo/scope.yaml" .ai/repo/
cp "$ROOT/.ai/repo/knowledge/sources.yaml" .ai/repo/knowledge/
{ printf '{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2025-06-18","capabilities":{},"clientInfo":{"name":"case612","version":"0"}}}\n'
  printf '{"jsonrpc":"2.0","method":"notifications/initialized"}\n'
  printf '{"jsonrpc":"2.0","id":2,"method":"tools/call","params":{"name":"majordomus_worktree_reconciliation","arguments":{}}}\n'
} | "$RB" mcp --standalone --repo "$PWD" 2>"$T/mcp.err" | sed -n 2p > "$T/frame.json"
jq -e '.result.structuredContent' "$T/frame.json" > "$T/mcp.json" 2>/dev/null \
  || { echo "    majordomus_worktree_reconciliation returned no typed answer:"; head -c 600 "$T/frame.json"; echo; tail -3 "$T/mcp.err" | sed 's/^/      /'; exit 1; }
if ! jq -e --slurpfile cli "$T/plan.json" \
     '.schema == "majordomus/worktree-reconciliation/v1" and .automatic == 2
      and ([.entries[] | {branch, state, step, automatic}] == [$cli[0].entries[] | {branch, state, step, automatic}])' \
     "$T/mcp.json" >/dev/null; then
  echo "    the MCP tool and the command line disagree about the same repository:"
  jq -c '[.entries[] | {branch, state, automatic}]' "$T/mcp.json" | head -c 600; echo; exit 1
fi
echo "    the MCP tool answers with the same states the command line printed"

# --- one subject, and why
"$RB" worktree reconcile landed > "$T/why.txt" 2>/dev/null \
  || { echo "    reconcile <branch> exited non-zero"; exit 1; }
if ! grep -q 'because   the trunk reaches the branch, and it was published' "$T/why.txt"; then
  echo "    the explanation does not give the reading that decided it:"; sed 's/^/      /' "$T/why.txt"; exit 1
fi
code=0; "$RB" worktree reconcile no-such-branch >/dev/null 2>&1 || code=$?
[ "$code" = 12 ] || { echo "    an unknown subject exited $code, not the missing-artifact code"; exit 1; }
echo "    a subject says why it stands where it does, and an unknown one is not an empty answer"

# --- the reading changes nothing
for b in landed dirty occupied same conflict stale local cut fresh; do
  git rev-parse --verify --quiet "refs/heads/$b" >/dev/null || { echo "    the branch $b is gone after a reading"; exit 1; }
done
for b in landed dirty same cut; do
  [ -d "$("$RB" worktree path "$b")" ] || { echo "    the worktree of $b is gone after a reading"; exit 1; }
done
[ -d "$occ" ] || { echo "    a worktree with a process inside is gone after a reading"; exit 1; }
[ -f "$("$RB" worktree path dirty)/draft.txt" ] || { echo "    uncommitted work changed under a reading"; exit 1; }
echo "    nothing was removed: every branch and worktree the reading judged is still there"

# a removal is named in git's own guarded form: a worktree without --force, a branch by a
# compare-and-delete on the commit that was judged
landed_head="$(git rev-parse refs/heads/landed)"
want_cmd="git worktree remove $("$RB" worktree path landed) && git update-ref -d refs/heads/landed $landed_head"
got_cmd="$(jq -r '[.entries[] | select(.branch == "landed")][0].command' "$T/plan.json")"
[ "$got_cmd" = "$want_cmd" ] || { echo "    the step of landed is not named in its guarded form:"; echo "      got  $got_cmd"; echo "      want $want_cmd"; exit 1; }
"$RB" worktree reconcile --apply >/dev/null 2>&1 && { echo "    reconcile accepted --apply, which this half does not have"; exit 1; }
echo "    each removal is named as the guarded git command that takes it, and nothing takes it here"
