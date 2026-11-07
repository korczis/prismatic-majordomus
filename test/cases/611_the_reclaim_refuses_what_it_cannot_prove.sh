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
#   no upstream, or   a branch never pushed, or whose remote branch was deleted: nothing but
#   upstream gone     this disk holds its history, whatever the trunk says about it
#   something inside      two worktrees looked untouched since the 12th and had live processes in
#                         them; mtime lies and only a reading taken now is true
#   cannot tell           no lsof, or git could not compare with the upstream: not knowing is not
#                         the same as nothing there, and the difference is somebody's afternoon
#
# And no branch is ever deleted: the disk was scarce, not the names.
#
# Whether anybody uses a worktree is one predicate's answer (I4346), `worktree.liveness`, which
# `worktree cleanup` asks in process and `scripts/reap-orphans --targets` asks through the
# executable. The second half of this case holds the reaper to it: every build directory the
# predicate answers keep for is kept with the predicate's reason, a repository that declares a
# mesh whose claims cannot be read gets nothing removed and a non-zero exit, and a build
# directory nothing names is reclaimed. Every fixture's git state is aged past the predicate's
# idle window first: a tree made a second ago is somebody's work in progress. The rule is
# project.reclaim-only-what-you-own v2.
. "$ROOT/test/lib.sh"
[ -f "$ROOT/apps/majordomus-cli/Cargo.toml" ] || { echo "    apps/majordomus-cli/Cargo.toml is missing"; exit 1; }
RB="$(rust_bin)" || rust_bin_exit $?
[ -x "$RB" ] || { echo "    the build produced no executable at $RB"; exit 1; }
command -v jq >/dev/null 2>&1 || skip "no jq"
command -v lsof >/dev/null 2>&1 || skip "no lsof, which this case is partly about"
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
# build output and the local half are ignored here as they are in the real repository
printf 'target\n.ai/local/\n' > .gitignore
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
mk upstream-gone

# Merged into the trunk and never pushed: its only upstream is this disk.
git switch -q -c no-upstream master
printf 'no-upstream\n' > no-upstream.txt
git add -A && git commit -qm no-upstream
git switch -q master
git merge -q --no-ff -m "merge no-upstream" no-upstream
git push -q origin master
"$RB" worktree create no-upstream >/dev/null 2>&1 \
  || { echo "    could not create the worktree of no-upstream"; exit 1; }

# the states, each made after the worktree exists
git push -q origin --delete upstream-gone && git fetch -q --prune origin \
  || { echo "    could not delete the remote branch of upstream-gone"; exit 1; }
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
occ="$("$RB" worktree path occupied)"
( cd "$occ" && sleep 300 ) & INSIDE=$!
trap 'kill "$INSIDE" 2>/dev/null; :' EXIT
sleep 1
printf 'uncommitted\n' >> "$("$RB" worktree path dirty-tree)/dirty-tree.txt"

# A worktree's index, HEAD and HEAD reflog, aged past any idle window.
age_git() {
  local gd
  gd="$(git -C "$1" rev-parse --absolute-git-dir)" || return 1
  touch -t 202001010000 "$gd/HEAD"
  [ ! -e "$gd/index" ] || touch -t 202001010000 "$gd/index"
  [ ! -e "$gd/logs/HEAD" ] || touch -t 202001010000 "$gd/logs/HEAD"
}
for b in spare-one ahead-of-remote occupied dirty-tree upstream-gone no-upstream; do
  age_git "$("$RB" worktree path "$b")" || { echo "    could not age the git state of $b"; exit 1; }
done

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
  "in use: "*working\ directory\ inside*) ;;
  *) echo "    it was refused for the wrong reason: $(why occupied)"; exit 1 ;;
esac
[ -d "$occ" ] || { echo "    the occupied worktree was removed"; exit 1; }
echo "    a worktree something is working in is refused, measured now rather than from its mtime"

for pair in "no-upstream:no upstream" "upstream-gone:is gone"; do
  b="${pair%%:*}"; want="${pair#*:}"
  [ "$(refused "$b")" = 1 ] || { echo "    $b was not refused"; cat "$T/out.json"; exit 1; }
  case "$(why "$b")" in
    *"$want"*) ;;
    *) echo "    $b was refused for the wrong reason: $(why "$b")"; exit 1 ;;
  esac
  [ -d "$("$RB" worktree path "$b")" ] || { echo "    $b was refused and removed anyway"; exit 1; }
done
echo "    a branch with no upstream, or whose upstream is gone, is refused"

# Uncommitted work present when the listing is taken keeps the branch off the list, so this
# asserts the outcome, not which layer refused it; work that appears between the listing and
# the removal is re-read by the reclaim (the unit tests in commands/worktree.rs plant it) and
# again by `worktree remove` under its lock.
[ "$(removed dirty-tree)" = 0 ] || { echo "    a worktree with uncommitted work was removed"; exit 1; }
[ -f "$("$RB" worktree path dirty-tree)/dirty-tree.txt" ] \
  || { echo "    the uncommitted work is gone"; exit 1; }
echo "    uncommitted work is never removed"

# --- and the names survive
for b in spare-one ahead-of-remote occupied dirty-tree upstream-gone no-upstream; do
  git show-ref --verify --quiet "refs/heads/$b" \
    || { echo "    branch $b was deleted; the reclaim is about disk, not names"; exit 1; }
done
echo "    no branch was deleted: the disk was what was scarce"

# --- the reaper asks the same predicate
# Four more worktrees, each merged, pushed, clean and holding cargo's build output, so that the
# reaper's own checks call every one spare and liveness alone decides: one nothing names, one
# with an open task, one a process builds into from elsewhere (CARGO_TARGET_DIR, the shape the
# 2026-10-28 incident had), and one whose HEAD moved a moment ago.
for b in reap-idle reap-task reap-env reap-head; do mk "$b"; done
for b in reap-idle reap-task reap-env reap-head; do
  p="$("$RB" worktree path "$b")"
  mkdir -p "$p/apps/majordomus-cli/target/debug"
  printf 'Signature: 8a477f597d28d172789f06886806bc55\n# This file is a cache directory tag created by cargo.\n' \
    > "$p/apps/majordomus-cli/target/CACHEDIR.TAG"
  age_git "$p" || { echo "    could not age the git state of $b"; exit 1; }
done
mkdir -p "$("$RB" worktree path reap-task)/.ai/local/state"
printf 'id: t-20261107000000-beef\ntask: still building\noutcome: active\n' \
  > "$("$RB" worktree path reap-task)/.ai/local/state/current.yaml"
# The builder is this case's own bash and not `sleep`: macOS withholds the environment of
# Apple's platform binaries from every reader, and a bash from anywhere else is not one. Where
# the bash running this case is Apple's too, the environment cannot be planted and that one
# expectation is said to be left out rather than passed.
env_target="$("$RB" worktree path reap-env)/apps/majordomus-cli/target"
( cd "$S" && exec env CARGO_TARGET_DIR="$env_target" "$BASH" -c 'sleep 300; :' ) &
BUILDER=$!
trap 'pkill -P "$BUILDER" 2>/dev/null || true; kill "$INSIDE" "$BUILDER" 2>/dev/null || true' EXIT
touch "$(git -C "$("$RB" worktree path reap-head)" rev-parse --absolute-git-dir)/HEAD"
sleep 1
ENV_SEEN=1
if [ "$(uname -s)" = Darwin ] \
  && ! ps -E -ww -o command= -p "$BUILDER" 2>/dev/null | grep -q "CARGO_TARGET_DIR=$env_target"; then
  ENV_SEEN=0
  echo "    (this bash, $BASH, is Apple's: macOS withholds its environment, so the CARGO_TARGET_DIR expectation is left out here)"
fi

reap() { MAJORDOMUS_BIN="$RB" MJ_ROOT="$S/repo" "$ROOT/scripts/reap-orphans" --targets "$@"; }
rc=0; reap > "$T/reap.txt" 2>&1 || rc=$?
LAST_OUT="$(cat "$T/reap.txt")"
[ "$rc" = 10 ] || { echo "    the reaper's report exited $rc, not 10:"; sed 's/^/      /' "$T/reap.txt"; exit 1; }
expect_grep 'reap-idle.*would reclaim'
expect_grep 'reap-task.*keep — merged, but somebody is working in it: task: task t-20261107000000-beef'
[ "$ENV_SEEN" = 0 ] \
  || expect_grep 'reap-env.*keep — merged, but somebody is working in it: process_environment: .*CARGO_TARGET_DIR='
expect_grep 'reap-head.*keep — merged, but somebody is working in it: git_activity: its HEAD changed'
echo "    the reaper keeps every build directory the predicate answers keep for, with its reason"

# the same predicate as a client asks it over MCP: one answer, the command line's
task_path="$("$RB" worktree path reap-task)"
{ printf '{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2025-06-18","capabilities":{},"clientInfo":{"name":"case611","version":"0"}}}\n'
  printf '{"jsonrpc":"2.0","method":"notifications/initialized"}\n'
  printf '{"jsonrpc":"2.0","id":2,"method":"tools/call","params":{"name":"majordomus_worktree_liveness","arguments":{"paths":["%s"]}}}\n' "$task_path"
} | "$RB" mcp --standalone --repo "$ROOT" 2>"$T/mcp.err" | sed -n 2p > "$T/frame.json"
jq -e '.result.structuredContent' "$T/frame.json" > "$T/tool.json" 2>/dev/null \
  || { echo "    majordomus_worktree_liveness returned no typed answer:"; head -c 800 "$T/frame.json"; echo; head -5 "$T/mcp.err"; exit 1; }
[ "$(jq -r '.paths[0].verdict' "$T/tool.json")" = keep ] \
  && jq -e '[.paths[0].reasons[].signal] | index("task")' "$T/tool.json" >/dev/null \
  || { echo "    the tool did not keep the worktree with an open task:"; cat "$T/tool.json"; exit 1; }
echo "    majordomus_worktree_liveness answers what the command line does"

# a repository that declares a mesh nobody can be asked about: nothing at all is removed
mkdir -p "$S/repo/.ai/repo/mesh"
printf 'kind: mesh-declaration\nenabled: true\n' > "$S/repo/.ai/repo/mesh/fixture.yaml"
rc=0; reap --reclaim > "$T/blind.txt" 2>&1 || rc=$?
LAST_OUT="$(cat "$T/blind.txt")"
[ "$rc" = 12 ] || { echo "    a sweep whose mesh claims cannot be read exited $rc, not 12:"; sed 's/^/      /' "$T/blind.txt"; exit 1; }
expect_grep 'liveness could not be measured \(.*mesh claims'
expect_grep 'nothing is reclaimed'
expect_file "$("$RB" worktree path reap-idle)/apps/majordomus-cli/target/CACHEDIR.TAG"
echo "    a declared mesh whose claims cannot be read removes nothing and exits non-zero"
mv "$S/repo/.ai/repo/mesh/fixture.yaml" "$T/mesh-declaration.yaml"

rc=0; reap --reclaim > "$T/reclaimed.txt" 2>&1 || rc=$?
LAST_OUT="$(cat "$T/reclaimed.txt")"
[ "$rc" = 0 ] || { echo "    the reclaim exited $rc, not 0:"; sed 's/^/      /' "$T/reclaimed.txt"; exit 1; }
expect_grep 'reap-idle.*reclaimed'
[ -e "$("$RB" worktree path reap-idle)/apps/majordomus-cli/target" ] \
  && { echo "    the build output nothing names survived the reclaim"; exit 1; }
kept_ones="reap-task reap-head"; [ "$ENV_SEEN" = 0 ] || kept_ones="$kept_ones reap-env"
for b in $kept_ones; do
  expect_file "$("$RB" worktree path "$b")/apps/majordomus-cli/target/CACHEDIR.TAG"
done
echo "    a build directory with no signal at all is reclaimed, and the rest are not"

echo "    the reclaim removes what it can prove is spare, and refuses the rest by name"
