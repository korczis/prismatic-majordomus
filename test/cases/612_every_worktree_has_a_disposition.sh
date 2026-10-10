# majordomus-covers: none
# Every branch and every worktree is in one state, with one next step, and the proven ones go.
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
# each is named for what it is, that `--apply` takes exactly the ones whose proof is complete,
# and that a second `--apply` does nothing.
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

# --- the act
status_before="$(git -C "$("$RB" worktree path dirty)" status --porcelain)"
"$RB" worktree reconcile --apply --format json > "$T/out.json" 2>"$T/out.err" \
  || { echo "    reconcile --apply exited non-zero:"; tail -3 "$T/out.err" | sed 's/^/      /'; exit 1; }
jq -e '(.removed | length) == 2 and ([.deleted[].branch] | sort) == ["landed", "same"] and (.refused | length) == 0' "$T/out.json" >/dev/null \
  || { echo "    the act did not take exactly the two proven steps:"; sed 's/^/      /' "$T/out.json"; exit 1; }
for b in landed same; do
  [ -d "$("$RB" worktree path "$b")" ] && { echo "    the worktree of $b was reported removed and is still there"; exit 1; }
  git rev-parse --verify --quiet "refs/heads/$b" >/dev/null && { echo "    the branch $b was reported deleted and is still there"; exit 1; }
done
for b in dirty occupied conflict stale local cut fresh; do
  git rev-parse --verify --quiet "refs/heads/$b" >/dev/null || { echo "    the branch $b is gone, and nothing proved it could go"; exit 1; }
done
[ -d "$occ" ] || { echo "    a worktree with a process inside was removed"; exit 1; }
[ "$(git -C "$("$RB" worktree path dirty)" status --porcelain)" = "$status_before" ] \
  || { echo "    uncommitted work changed under the act"; exit 1; }
echo "    the two proven subjects went; uncommitted work, an occupied worktree and unpublished commits stayed"

# a deleted branch names the commit it pointed at, and that commit is still there
head="$(jq -r '.deleted[] | select(.branch == "landed") | .head' "$T/out.json")"
git cat-file -e "$head^{commit}" 2>/dev/null \
  || { echo "    the head recorded for the deleted branch is not a commit"; exit 1; }
echo "    a deleted branch is one command from being back"

# --- the act again
"$RB" worktree reconcile --apply --format json > "$T/again.json" 2>/dev/null \
  || { echo "    the second reconcile --apply exited non-zero"; exit 1; }
jq -e '(.removed | length) == 0 and (.deleted | length) == 0 and (.refused | length) == 0' "$T/again.json" >/dev/null \
  || { echo "    the second run did something:"; sed 's/^/      /' "$T/again.json"; exit 1; }
if ! "$RB" worktree reconcile | grep -q 'nothing to reconcile'; then
  echo "    after the act the reading does not say there is nothing to reconcile"; exit 1
fi
echo "    run twice, the second run does nothing"
