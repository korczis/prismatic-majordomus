# majordomus-covers: none
# The collision guard, driven against a fixture repository with a bare origin.
#
# scripts/collision-check answered "is another pushed branch already building this path?"
# and had no caller: CLAUDE.md told every worker to run it, and nothing did. It also had no
# "could not measure" answer. In a single-branch or shallow checkout it compared the paths
# against no branch at all and printed `OK nothing else claims ...` — a confident verdict
# about nothing, which is the shape of most of this repository's broken gates.
#
# scripts/ci/collision-gate is the caller: on a pull request it checks every path the pull
# request adds. A gate that refuses legitimate work gets switched off, so the cases below
# are mostly about what it must NOT refuse, each in the shape CI actually has:
#   - the pull request's own branch: in CI HEAD is a detached merge commit, so the branch
#     cannot be recognised from the checkout and has to be excluded by name;
#   - the pushed branch a follow-up pull request is built on (related work);
#   - a rescue branch holding the same bytes (the same work, copied);
#   - a branch merged long ago that carries an older version of a file master has changed.
# And what it must refuse: another unmerged branch that independently adds the same new path
# with different content — named, so the person knows whose work to read.
. "$ROOT/test/lib.sh"
CHECK="$ROOT/scripts/collision-check"
GATE="$ROOT/scripts/ci/collision-gate"
[ -x "$CHECK" ] || { echo "    $CHECK is not executable"; exit 1; }
[ -x "$GATE" ] || { echo "    $GATE is not executable"; exit 1; }
command -v jq >/dev/null 2>&1 || { echo "    jq is required: the gate reads the pull_request event with it"; exit 1; }

export GIT_AUTHOR_NAME=t GIT_AUTHOR_EMAIL=t@example.com GIT_COMMITTER_NAME=t GIT_COMMITTER_EMAIL=t@example.com
unset GITHUB_ACTIONS GITHUB_EVENT_NAME GITHUB_BASE_REF GITHUB_HEAD_REF GITHUB_EVENT_PATH MJ_ROOT MJ_COLLISION_BASE

O="$T/origin.git"
W="$T/work"
g() { git -C "$W" "$@"; }
commit_file() {   # commit_file <path> <content> <message>
  mkdir -p "$(dirname "$W/$1")"
  printf '%s\n' "$2" > "$W/$1"
  g add -- "$1"
  g commit -q -m "$3"
}

# ---------------------------------------------------------------- the fixture
git init -q --bare "$O"
git -C "$O" symbolic-ref HEAD refs/heads/master
git init -q "$W"
g symbolic-ref HEAD refs/heads/master
g remote add origin "$O"
commit_file README.md 'a repository' 'initial'
commit_file .gitattributes 'gen/** merge=derived' 'declare the derived tree'
g push -q origin master

# a branch merged long ago, carrying an older version of a file master has since changed
g checkout -q -b feature/merged master
commit_file docs/guide.md 'version one' 'guide, version one'
g push -q origin feature/merged
g checkout -q master
g merge -q --no-ff --no-edit feature/merged
commit_file docs/guide.md 'version two' 'guide, version two'
g push -q origin master

# an unmerged branch editing a file master has: an ordinary edit, not a new path
g checkout -q -b feature/edits-guide master
commit_file docs/guide.md 'version three' 'guide, version three'
g push -q origin feature/edits-guide

# the claimant: another session's unmerged branch inventing the same new module
g checkout -q -b feature/claimant master
commit_file src/release/mod.rs 'pub fn release() { /* design A */ }' 'feat: the release module, design A'
g push -q origin feature/claimant

# a pushed branch that a follow-up pull request is built on
g checkout -q -b feature/foundation master
commit_file src/stacked.rs 'version one' 'feat: the foundation'
g push -q origin feature/foundation

# the pull request: built on the foundation, adding a colliding module and a path of its own
g checkout -q -b feature/pr feature/foundation
commit_file src/stacked.rs 'version two' 'feat: build on the foundation'
commit_file src/release/mod.rs 'pub fn release() { /* design B */ }' 'feat: the release module, design B'
commit_file src/only-pr.rs 'nobody else has this' 'feat: a path only the pull request adds'
g push -q origin feature/pr

# a rescue copy of the pull request's own work, committed independently of its commits
g checkout -q -b rescue/pr-copy master
commit_file src/only-pr.rs 'nobody else has this' 'rescue: a copy of the pull request work'
g push -q origin rescue/pr-copy

# an older rescue of the same work, taken before design B was finished: byte-identical on one
# path the pull request adds, different on another. A copy that moved on, not a second design.
g checkout -q -b rescue/pr-older master
commit_file src/only-pr.rs 'nobody else has this' 'rescue: an older copy of the pull request work'
commit_file src/release/mod.rs 'pub fn release() { /* design B, a draft */ }' 'rescue: design B, a draft'
g push -q origin rescue/pr-older

# another branch generating the derived tree differently: a generation, not a design
g checkout -q -b feature/regen master
commit_file gen/index.json '{"generation": "other"}' 'chore: regenerate the derived tree'
g push -q origin feature/regen

# a pull request that adds only what nobody claims, and one derived file
g checkout -q -b feature/clean master
commit_file src/fresh.rs 'fresh' 'feat: a fresh path'
commit_file gen/index.json '{"generation": "clean"}' 'chore: derive'
g push -q origin feature/clean
g checkout -q master

# The checkout CI has: a full clone, HEAD detached on the merge of the head into the base.
C="$T/ci"
git clone -q "$O" "$C"
ci_merge() {   # ci_merge <head branch>
  git -C "$C" checkout -q --detach origin/master
  git -C "$C" merge -q --no-ff --no-edit "origin/$1"
}
pr() {   # pr <head branch> <command...>: the environment of a pull_request run
  local head="$1"; shift
  env GITHUB_ACTIONS=true GITHUB_EVENT_NAME=pull_request GITHUB_BASE_REF=master \
    GITHUB_HEAD_REF="$head" MJ_ROOT="$C" "$@"
}

# ---------------------------------------------------------------- 1. a collision is refused
ci_merge feature/pr
expect_exit 11 pr feature/pr "$GATE"
expect_grep 'COLLISION  src/release/mod.rs is added independently, with different content, by:'
expect_grep '^  origin/feature/claimant +'
expect_grep 'REFUSED'
# the pull request's own branch is not reported — not as a collision, not as a related branch,
# not as a copy: no listing row names it (the header naming the head under test is not a row)
expect_no_grep '^  origin/feature/pr +'
# the pushed branch it is built on is related work, and passes
expect_grep '^  origin/feature/foundation +1 path\(s\) sharing the commit that added them, e\.g\. src/stacked\.rs$'
expect_no_grep 'COLLISION  src/stacked.rs'
# the rescue copy carries the same bytes: the same work, not a competitor
expect_grep '^  origin/rescue/pr-copy +1 path\(s\) with the same bytes, e\.g\. src/only-pr\.rs$'
expect_no_grep 'COLLISION  src/only-pr.rs'
# the older rescue differs on the module but is byte-identical on another path: a diverged
# copy of this work, listed as such and not among the collisions
expect_grep 'DIVERGED   copies of this work'
expect_grep '^  origin/rescue/pr-older +1 path\(s\) differ, e\.g\. src/release/mod\.rs$'
# a branch that touches nothing this pull request adds is not mentioned
expect_no_grep 'feature/clean|feature/merged|feature/edits-guide|feature/regen'

# A diverged copy alone is not refused. With the claimant set aside the same question passes,
# and still names the copy, so the person landing can see that one of the two is stale.
expect_exit 0 env MJ_ROOT="$C" "$CHECK" --added --head origin/feature/pr \
  --exclude origin/feature/pr --exclude origin/feature/claimant
expect_grep 'DIVERGED   copies of this work'
expect_grep '^  origin/rescue/pr-older +1 path\(s\) differ, e\.g\. src/release/mod\.rs$'
expect_no_grep 'COLLISION'

# The head is the pull request's, never whatever the checkout's second parent is. HEAD here
# is the merge of a different, clean branch — the shape of any checkout sitting on master's
# chain of merge commits. A gate that took HEAD^2 for the head measured feature/clean, found
# nothing, and passed: 131 of 131 branches of this repository did exactly that in a sweep.
ci_merge feature/clean
expect_exit 11 pr feature/pr "$GATE"
expect_grep 'head origin/feature/pr'
expect_grep '^  origin/feature/claimant +'

# ---------------------------------------------------------------- 2. what only this PR adds passes
ci_merge feature/clean
expect_exit 0 pr feature/clean "$GATE"
expect_grep 'OK   no other branch collides on the 1 path\(s\)'
expect_grep '^  src/fresh.rs$'
expect_grep '[0-9]+ unmerged of [0-9]+ branch\(es\) compared against origin/master'
expect_no_grep '^  origin/feature/clean +'
# feature/regen adds the same derived file differently; the merge regenerates it, so it is
# counted as not examined rather than refused
expect_grep '1 derived path\(s\) not examined, the merge regenerates them'
expect_no_grep 'gen/index.json|feature/regen'

# ---------------------------------------------------------------- 3. the event, and a person's acceptance
# The event names the head commit; a label a person added accepts a collision they have
# read. Any other label does not.
ci_merge feature/pr
headsha="$(git -C "$C" rev-parse origin/feature/pr)"
printf '{"pull_request":{"head":{"sha":"%s"},"labels":[{"name":"ci:full"}]}}\n' "$headsha" > "$T/event-plain.json"
expect_exit 11 pr feature/pr env GITHUB_EVENT_PATH="$T/event-plain.json" "$GATE"
expect_grep "head $headsha"
expect_grep '^  origin/feature/claimant +'
printf '{"pull_request":{"head":{"sha":"%s"},"labels":[{"name":"ci:full"},{"name":"collision:accepted"}]}}\n' "$headsha" > "$T/event-accepted.json"
expect_exit 0 pr feature/pr env GITHUB_EVENT_PATH="$T/event-accepted.json" "$GATE"
expect_grep '^  origin/feature/claimant +'
expect_grep 'ACCEPTED  the collision above is accepted by the label collision:accepted'

# a run that is not a pull request adds nothing and says that is why it passes
expect_exit 0 env GITHUB_ACTIONS=true GITHUB_EVENT_NAME=push MJ_ROOT="$C" "$GATE"
expect_grep 'a push run, not a pull request'

# ---------------------------------------------------------------- 4. --new, and merged branches
# A path the base already has is not the --new question. Without --new an unmerged edit to it
# is reported; with it, not. A merged branch carrying an older version is reported by neither.
expect_exit 11 env MJ_ROOT="$W" "$CHECK" docs/guide.md
expect_grep 'COLLISION  docs/guide.md is carried differently by:'
expect_grep '^  origin/feature/edits-guide +'
expect_no_grep 'feature/merged'
expect_exit 0 env MJ_ROOT="$W" "$CHECK" --new docs/guide.md
expect_no_grep 'feature/merged|feature/edits-guide'

# Before implementing, from master: both branches inventing the module are named.
expect_exit 11 env MJ_ROOT="$W" "$CHECK" --new src/release/mod.rs
expect_grep '^  origin/feature/claimant +'
expect_grep '^  origin/feature/pr +'
# ...and from the pull request's own branch, that branch is not its own claimant.
g checkout -q feature/pr
expect_exit 11 env MJ_ROOT="$W" "$CHECK" --new src/release/mod.rs
expect_grep '^  origin/feature/claimant +'
expect_no_grep '^  (origin/)?feature/pr +'
g checkout -q master

# ---------------------------------------------------------------- 5. could not measure is 12, never 0
# A shallow clone of the default branch: the checkout a depth-limited CI job has.
git clone -q --depth 1 "file://$O" "$T/shallow"
expect_exit 12 env MJ_ROOT="$T/shallow" "$CHECK" --new src/release/mod.rs
expect_grep 'CANNOT MEASURE  the repository is shallow'
expect_no_grep 'OK'
# shallow with every branch fetched is still refused: ancestry cannot be decided
git clone -q --depth 1 --no-single-branch "file://$O" "$T/shallow-all"
expect_exit 12 env MJ_ROOT="$T/shallow-all" "$CHECK" --new src/release/mod.rs
expect_grep 'CANNOT MEASURE  the repository is shallow'
# full history but one branch: nothing to compare against
git clone -q --single-branch --branch master "$O" "$T/single"
expect_exit 12 env MJ_ROOT="$T/single" "$CHECK" --new src/release/mod.rs
expect_grep 'CANNOT MEASURE  no branch but `origin/master` and this work is visible'
expect_no_grep 'OK'
# and through the gate, in the pull request's shape, the 12 is passed on
expect_exit 12 pr feature/pr env MJ_ROOT="$T/single" "$GATE"
expect_no_grep 'OK'
# a base that does not resolve
expect_exit 12 env MJ_ROOT="$W" "$CHECK" --since origin/nowhere --new src/x
expect_grep 'the base `origin/nowhere` does not resolve'

# ---------------------------------------------------------------- 6. usage
expect_exit 2 env MJ_ROOT="$W" "$CHECK"
expect_grep 'name at least one path'
expect_exit 2 env MJ_ROOT="$W" "$CHECK" --added src/x
expect_grep '--added reads the paths from the head'
expect_exit 2 env MJ_ROOT="$W" "$CHECK" --quick src/x
expect_grep 'unknown option: --quick'

echo "    a new path added independently elsewhere is refused by name; its own branch, related work and copies pass; blindness is 12"
