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
# expect_row_under <listing header> <row>: the row is in the listing that header opens, not
# merely somewhere in the output: a related row and a collision row print alike, so only the
# header above a row says which it is
expect_row_under() {
  H="$1" R="$2" awk '/^collision-check: / { in_s = ($0 ~ ENVIRON["H"]); next } in_s && $0 ~ ENVIRON["R"] { found = 1 } END { exit !found }' <<< "$LAST_OUT" \
    || { printf '    expected /%s/ under /%s/ in output:\n%s\n' "$2" "$1" "$LAST_OUT"; return 1; }
}
NUMBER_RELATED='not collisions — branches carrying another name for a number'
DECL_RELATED='not collisions — branches declaring what the head declares'

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
expect_exit 2 env MJ_ROOT="$W" MJ_COLLISION_RECENT_DAYS=soon "$CHECK" --added
expect_grep 'MJ_COLLISION_RECENT_DAYS must be a number of days'
expect_exit 2 env MJ_ROOT="$W" MJ_COLLISION_REWRITE_LINES=many "$CHECK" --added
expect_grep 'MJ_COLLISION_REWRITE_LINES must be a number of lines'

# ---------------------------------------------------------------- 7. what no added path shows
# Two branches can take one thing from each other without adding one path: a number under two
# names (ADR 0168 and I4296 were each taken twice), one flag of one command (PR #2748 and I4268
# both added `evidence --run-gates`), one capability id, one authored file rewritten twice
# (#2741 and I4400 both rewrote site/charts.js). The base gets the files those are declared in.
g checkout -q master
mkdir -p "$W/share" "$W/apps/majordomus-cli/src/capability/builtin" "$W/site"
cat > "$W/share/commands.yaml" <<'YAML'
commands:
  - id: evidence
    title: evidence
    syntax: "majordomus evidence --covers <token> [--json]"
  - id: plan
    title: plan
    syntax: majordomus plan [--json]
YAML
cat > "$W/apps/majordomus-cli/src/capability/builtin/evidence.rs" <<'RS'
pub fn declare() {
            capability! {
                id: "evidence.record",
                title: "Record evidence",
            }
}
RS
awk 'BEGIN { for (i = 1; i <= 100; i++) print "chart line " i ", version one" }' > "$W/site/charts.js"
g add -- share apps site
g commit -q -m 'the commands, a capability and the charts'
g push -q origin master
# a branch forked before the base rewrote the charts: older than the base, not a rewrite
g checkout -q -b feature/stale-charts master
commit_file docs/unrelated.md 'unrelated' 'docs: something else'
g push -q origin feature/stale-charts
g checkout -q master
awk 'BEGIN { for (i = 1; i <= 100; i++) print "chart line " i ", version two" }' > "$W/site/charts.js"
g commit -q -am 'the charts, version two'
g push -q origin master

# one number, two names, on two branches neither of which contains the other
g checkout -q -b feature/adr-a master
commit_file .ai/repo/adrs/0300-a.md '# ADR 0300: design a' 'docs(adr): 0300, design a'
g push -q origin feature/adr-a
g checkout -q -b feature/adr-b master
commit_file .ai/repo/adrs/0300-b.md '# ADR 0300: design b' 'docs(adr): 0300, design b'
g push -q origin feature/adr-b
# a branch built on adr-a, carrying its number under its name, a second name under the same
# number (an amendment of it), and the next number
g checkout -q -b feature/adr-a-follow feature/adr-a
commit_file .ai/repo/adrs/0300-a-amended.md '# ADR 0300, amended' 'docs(adr): amend 0300'
commit_file .ai/repo/adrs/0301-follow.md '# ADR 0301: follows 0300' 'docs(adr): 0301'
g push -q origin feature/adr-a-follow
g checkout -q -b feature/case-a master
commit_file test/cases/9990_a.sh 'echo a' 'test: case 9990, a'
g push -q origin feature/case-a
g checkout -q -b feature/case-b master
commit_file test/cases/9990_b.sh 'echo b' 'test: case 9990, b'
g push -q origin feature/case-b

# one flag of one command, declared on two branches; the same flag on another command
with_syntax() {   # with_syntax <command> <extra syntax>: the base's commands.yaml, one line longer
  git -C "$W" show master:share/commands.yaml \
    | awk -v c="$1" -v x="$2" '/^  - id: / { id = $3 } /^    syntax: / && id == c { sub(/"?$/, ""); sub(/syntax: "?/, "syntax: \""); $0 = $0 " | " x "\"" } { print }' \
    > "$W/share/commands.yaml"
}
g checkout -q -b feature/flag-a master
with_syntax evidence 'majordomus evidence --run-gates [--all]'
g commit -q -am 'feat(evidence): --run-gates, a'
g push -q origin feature/flag-a
g checkout -q -b feature/flag-b master
with_syntax evidence 'majordomus evidence --run-gates <gate>'
g commit -q -am 'feat(evidence): --run-gates, b'
g push -q origin feature/flag-b
g checkout -q -b feature/flag-other master
with_syntax plan 'majordomus plan --run-gates'
g commit -q -am 'feat(plan): --run-gates'
g push -q origin feature/flag-other
# a train top that merged flag-a: it carries the flag through flag-a's commit
g checkout -q -b integration/train master
g merge -q --no-ff --no-edit feature/flag-a
g push -q origin integration/train

# one capability id, declared on two branches in two different files
g checkout -q -b feature/cap-a master
printf '%s\n' 'pub fn more() {' '            capability! {' '                id: "evidence.run_gates",' '            }' '}' \
  >> "$W/apps/majordomus-cli/src/capability/builtin/evidence.rs"
g commit -q -am 'feat(capability): evidence.run_gates, a'
g push -q origin feature/cap-a
g checkout -q -b feature/cap-b master
commit_file apps/majordomus-cli/src/capability/builtin/gates.rs \
  "$(printf '%s\n' 'pub fn declare() {' '            capability! {' '                id: "evidence.run_gates",' '            }' '}')" \
  'feat(capability): evidence.run_gates, b'
g push -q origin feature/cap-b

# one authored file rewritten on two recent branches; a small edit, and an old rewrite
rewrite_charts() {   # rewrite_charts <version>
  awk -v v="$1" 'BEGIN { for (i = 1; i <= 100; i++) print "chart line " i ", version " v }' > "$W/site/charts.js"
}
g checkout -q -b feature/rw-a master
rewrite_charts three-a
g commit -q -am 'feat(site): the charts, rewritten, a'
g push -q origin feature/rw-a
g checkout -q -b feature/rw-b master
rewrite_charts three-b
g commit -q -am 'feat(site): the charts, rewritten, b'
g push -q origin feature/rw-b
# a follow-up built on rw-a that rewrites the charts again: built on the head, not beside it
g checkout -q -b feature/rw-a-follow feature/rw-a
rewrite_charts four-a
g commit -q -am 'feat(site): the charts, rewritten again on top of a'
g push -q origin feature/rw-a-follow
g checkout -q -b feature/rw-small master
awk 'NR <= 3 { print "chart line " NR ", touched"; next } { print }' "$W/site/charts.js" > "$W/site/charts.js.new"
mv "$W/site/charts.js.new" "$W/site/charts.js"
g commit -q -am 'fix(site): three chart lines'
g push -q origin feature/rw-small
g checkout -q -b feature/rw-old master
rewrite_charts old
GIT_COMMITTER_DATE='2020-01-01T00:00:00Z' GIT_AUTHOR_DATE='2020-01-01T00:00:00Z' \
  g commit -q -am 'feat(site): the charts, rewritten long ago'
g push -q origin feature/rw-old
g checkout -q master
git -C "$C" fetch -q origin '+refs/heads/*:refs/remotes/origin/*'

# ---------------------------------------------------------------- 7a. one ADR number, two names
ci_merge feature/adr-a
expect_exit 11 pr feature/adr-a "$GATE"
expect_grep 'COLLISION  number 0300 of \.ai/repo/adrs/ is taken twice: the head adds \.ai/repo/adrs/0300-a\.md'
expect_grep '^  origin/feature/adr-b +\.ai/repo/adrs/0300-b\.md$'
expect_grep 'REFUSED'
# the branch built on it carries a second name for 0300 too: listed as related, not refused
expect_grep 'not collisions — branches carrying another name for a number the head adds'
expect_row_under "$NUMBER_RELATED" '^  origin/feature/adr-a-follow +\.ai/repo/adrs/0300-a-amended\.md$'
expect_no_grep 'feature/case-|feature/flag-|feature/cap-'

# ---------------------------------------------------------------- 7b. two_branches_taking_one_number_are_refused
ci_merge feature/case-a
expect_exit 11 pr feature/case-a "$GATE"
expect_grep 'COLLISION  number 9990 of test/cases/ is taken twice: the head adds test/cases/9990_a\.sh'
expect_grep '^  origin/feature/case-b +test/cases/9990_b\.sh$'
expect_grep 'REFUSED'
# and the other way round, naming the other
ci_merge feature/case-b
expect_exit 11 pr feature/case-b "$GATE"
expect_grep '^  origin/feature/case-a +test/cases/9990_a\.sh$'
expect_grep 'REFUSED'

# ---------------------------------------------------------------- 7c. built on the head, the same number
# With the independent claimant set aside, nothing refuses: the follow-up takes 0300 under a
# second name, but it is built on the head (it carries the head's name, and contains it).
# From the follow-up's side the branch it is built on carries 0300 only under a name the
# follow-up carries too, so it is no second name at all and the number check names nothing;
# the branch is listed by the path check, as the same bytes. Where the head contains a branch
# holding another name for its number, the merge base decides: 7j.
expect_exit 0 env MJ_ROOT="$C" "$CHECK" --added --head origin/feature/adr-a \
  --exclude origin/feature/adr-a --exclude origin/feature/adr-b
expect_no_grep 'COLLISION'
expect_row_under "$NUMBER_RELATED" '^  origin/feature/adr-a-follow +\.ai/repo/adrs/0300-a-amended\.md$'
expect_grep 'OK   no other branch takes a number or a declaration `origin/feature/adr-a` adds \(1 numbered name'
expect_exit 0 env MJ_ROOT="$C" "$CHECK" --added --head origin/feature/adr-a-follow \
  --exclude origin/feature/adr-a-follow --exclude origin/feature/adr-b
expect_no_grep 'COLLISION'
expect_row_under 'not collisions — branches carrying paths the head adds' '^  origin/feature/adr-a +1 path\(s\) with the same bytes, e\.g\. \.ai/repo/adrs/0300-a\.md$'
expect_no_grep "$NUMBER_RELATED"

# ---------------------------------------------------------------- 7d. one flag of one command, twice
ci_merge feature/flag-a
expect_exit 11 pr feature/flag-a "$GATE"
expect_grep 'COLLISION  the flag --run-gates of the command `evidence` \(share/commands\.yaml\) is declared independently by:'
expect_grep '^  origin/feature/flag-b +'
expect_grep 'REFUSED'
# the same flag on another command is another declaration; --all is flag-a's alone
expect_no_grep 'feature/flag-other'
expect_no_grep 'flag --all'
# the train that merged flag-a carries it through flag-a's own commit
expect_no_grep '^  origin/integration/train +[0-9]'
expect_row_under "$DECL_RELATED" '^  origin/integration/train +evidence --run-gates$'

# ---------------------------------------------------------------- 7e. one capability id, twice
ci_merge feature/cap-a
expect_exit 11 pr feature/cap-a "$GATE"
expect_grep 'COLLISION  the capability id evidence\.run_gates \(apps/majordomus-cli/src/capability/builtin\) is declared independently by:'
expect_grep '^  origin/feature/cap-b +'
expect_no_grep '^  origin/feature/cap-a +'
expect_grep 'REFUSED'

# ---------------------------------------------------------------- 7f. a train is not refused against its member
# Neither the member against the train that merged it, nor the train against its member; the
# independent flag-b is set aside, because against it both are rightly refused.
expect_exit 0 env MJ_ROOT="$C" "$CHECK" --added --head origin/feature/flag-a \
  --exclude origin/feature/flag-a --exclude origin/feature/flag-b
expect_no_grep 'COLLISION'
expect_row_under "$DECL_RELATED" '^  origin/integration/train +evidence --run-gates$'
expect_exit 0 env MJ_ROOT="$C" "$CHECK" --added --head origin/integration/train \
  --exclude origin/integration/train --exclude origin/feature/flag-b
expect_no_grep 'COLLISION'
expect_row_under "$DECL_RELATED" '^  origin/feature/flag-a +evidence --run-gates$'

# ---------------------------------------------------------------- 7g. one file rewritten twice is reported
# Reported and never refused: the exit stays 0. The small edit, the rewrite older than the
# window and the branch that is only older than the base are not rewrites of it.
ci_merge feature/rw-a
expect_exit 0 pr feature/rw-a "$GATE"
expect_grep 'OVERLAP  site/charts\.js is also rewritten by:'
expect_grep '^  origin/feature/rw-b +200 line\(s\) '
expect_grep 'an overlap is reported, not refused'
expect_no_grep 'feature/rw-small|feature/rw-old|feature/stale-charts|feature/rw-a-follow|COLLISION'
# the window is the owner's to set: with a smaller line bound the small edit is a rewrite too
expect_exit 0 env MJ_ROOT="$C" MJ_COLLISION_REWRITE_LINES=6 "$CHECK" --added --head origin/feature/rw-a \
  --exclude origin/feature/rw-a
expect_grep '^  origin/feature/rw-small +6 line\(s\) '
expect_no_grep 'feature/rw-old|feature/stale-charts'


# ---------------------------------------------------------------- 7h. trains that merged master
# The shape every train has here ("Merge master 54c317f11ad (#2654, I0201) into train 13c"):
# the train merges its member and then master; master moves on, and the member, re-pushed,
# merges master too. The two then have two merge bases, the member's own commit and the master
# commit the train took, and plain `git merge-base` names one of them by commit date. Neither
# refuses the other, whichever it names, and neither reports the other's copy of the member's
# rewrite as an overlap: that rewrite is the member's own.
g checkout -q master
awk 'BEGIN { for (i = 1; i <= 100; i++) print "board line " i ", version one" }' > "$W/site/board.js"
g add -- site/board.js
g commit -q -m 'the board'
g push -q origin master
g checkout -q -b feature/watch-m master
with_syntax plan 'majordomus plan --watch'
awk 'NR <= 90 { print "board line " NR ", watched"; next } { print }' "$W/site/board.js" > "$W/site/board.js.new"
mv "$W/site/board.js.new" "$W/site/board.js"
g commit -q -am 'feat(plan): --watch, and the board rewritten for it'
g push -q origin feature/watch-m
g checkout -q -b integration/watch-train master
g merge -q --no-ff --no-edit feature/watch-m
g checkout -q master
commit_file docs/x.md 'x' 'docs: master moves on'
g push -q origin master
g checkout -q integration/watch-train
g merge -q --no-ff --no-edit master
g push -q origin integration/watch-train
g checkout -q master
awk 'NR == 100 { print "board line 100, changed on master"; next } { print }' "$W/site/board.js" > "$W/site/board.js.new"
mv "$W/site/board.js.new" "$W/site/board.js"
g commit -q -am 'the board, its last line changed'
g push -q origin master
g checkout -q feature/watch-m
g merge -q --no-ff --no-edit master >/dev/null
g push -q origin feature/watch-m
# Two train tops merging the member and an unrelated member in opposite orders: criss-cross
# merge bases, the two members. Plain `git merge-base` names whichever was committed later, so
# the unrelated member is built both later and earlier than the one that declares --watch. It
# edits a file rather than adding one: an added path with the same bytes on the other top would
# make it a copy of this work, and the merge bases would never be asked; and the first top
# commits its own derived edit, so the two tops do not hold one identical commands.yaml.
for order in late early; do
  case "$order" in late) when='2030-01-01T00:00:00Z' ;; *) when='2020-01-01T00:00:00Z' ;; esac
  g checkout -q -b "feature/watch-x-$order" master
  printf '%s\n' "unrelated, $order" >> "$W/README.md"
  g add -- README.md
  GIT_COMMITTER_DATE="$when" GIT_AUTHOR_DATE="$when" g commit -q -m "docs: an unrelated member, $order"
  g push -q origin "feature/watch-x-$order"
  g checkout -q -b "integration/watch-t1-$order" master
  g merge -q --no-ff --no-edit "feature/watch-x-$order"
  g merge -q --no-ff --no-edit feature/watch-m
  awk '/^    title: plan$/ { print "    title: plan, as the first top derived it"; next } { print }' \
    "$W/share/commands.yaml" > "$W/share/commands.yaml.new"
  mv "$W/share/commands.yaml.new" "$W/share/commands.yaml"
  g commit -q -am "chore(derive): the first top, $order"
  g push -q origin "integration/watch-t1-$order"
  g checkout -q -b "integration/watch-t2-$order" master
  g merge -q --no-ff --no-edit feature/watch-m
  g merge -q --no-ff --no-edit "feature/watch-x-$order"
  g push -q origin "integration/watch-t2-$order"
done
g checkout -q master
git -C "$C" fetch -q origin '+refs/heads/*:refs/remotes/origin/*'
# the shape is the one that refused before: two merge bases, one of them master's
[ "$(git -C "$C" merge-base --all origin/feature/watch-m origin/integration/watch-train | grep -c .)" = 2 ] \
  || { echo "    the member and its train were meant to have two merge bases"; exit 1; }
expect_exit 0 env MJ_ROOT="$C" "$CHECK" --added --head origin/feature/watch-m --exclude origin/feature/watch-m
expect_no_grep 'COLLISION|OVERLAP'
expect_row_under "$DECL_RELATED" '^  origin/integration/watch-train +plan --watch$'
expect_exit 0 env MJ_ROOT="$C" "$CHECK" --added --head origin/integration/watch-train \
  --exclude origin/integration/watch-train
expect_no_grep 'COLLISION|OVERLAP'
expect_row_under "$DECL_RELATED" '^  origin/feature/watch-m +plan --watch$'
for order in late early; do
  expect_exit 0 env MJ_ROOT="$C" "$CHECK" --added --head "origin/integration/watch-t2-$order" \
    --exclude "origin/integration/watch-t2-$order"
  expect_no_grep 'COLLISION|OVERLAP'
  expect_row_under "$DECL_RELATED" "^  origin/integration/watch-t1-$order +plan --watch\$"
done

# ---------------------------------------------------------------- 7i. copies of the head declare what it declares
# A rescue that copied only the flag (the same file, byte for byte) and a re-cut of the whole
# of it onto a newer master (the same bytes on the path it adds; its share/commands.yaml differs,
# master having changed another line of it) are this work, and are listed, never refused. A
# branch that declares the same flag in its own words is still refused.
g checkout -q -b feature/since-h master
with_syntax plan 'majordomus plan --since <rev>'
g commit -q -am 'feat(plan): --since'
commit_file src/since.rs 'pub fn since() {}' 'feat(plan): what --since reads'
g push -q origin feature/since-h
g checkout -q -b rescue/since-h-flag master
with_syntax plan 'majordomus plan --since <rev>'
g commit -q -am 'rescue: --since'
g push -q origin rescue/since-h-flag
g checkout -q -b feature/since-other master
with_syntax plan 'majordomus plan --since <date>'
g commit -q -am 'feat(plan): --since, another design'
g push -q origin feature/since-other
g checkout -q master
awk '/^    title: evidence$/ { print "    title: the evidence"; next } { print }' "$W/share/commands.yaml" > "$W/share/commands.yaml.new"
mv "$W/share/commands.yaml.new" "$W/share/commands.yaml"
g commit -q -am 'the evidence command, retitled'
g push -q origin master
g checkout -q -b feature/since-h-recut master
g cherry-pick feature/since-h~1 feature/since-h >/dev/null
g push -q origin feature/since-h-recut
g checkout -q master
git -C "$C" fetch -q origin '+refs/heads/*:refs/remotes/origin/*'
expect_exit 0 env MJ_ROOT="$C" "$CHECK" --added --head origin/feature/since-h \
  --exclude origin/feature/since-h --exclude origin/feature/since-other
expect_no_grep 'COLLISION'
expect_row_under "$DECL_RELATED" '^  origin/rescue/since-h-flag +plan --since$'
expect_row_under "$DECL_RELATED" '^  origin/feature/since-h-recut +plan --since$'
expect_grep '^  origin/feature/since-h-recut +1 path\(s\) with the same bytes, e\.g\. src/since\.rs$'
expect_exit 11 env MJ_ROOT="$C" "$CHECK" --added --head origin/feature/since-h --exclude origin/feature/since-h
expect_grep 'COLLISION  the flag --since of the command `plan` \(share/commands\.yaml\) is declared independently by:'
expect_grep '^  origin/feature/since-other +'
# the copies stay listed as this work beside the refusal, never among the refused
expect_row_under "$DECL_RELATED" '^  origin/rescue/since-h-flag +plan --since$'
expect_row_under "$DECL_RELATED" '^  origin/feature/since-h-recut +plan --since$'

# ---------------------------------------------------------------- 7j. the merge base decides a number
# The head renamed a draft it is built on (0310-c.md, on an unmerged branch, became
# 0310-c-final.md), and a sibling is built on the same draft. Neither carries the head's name,
# so only the merge base says the other name is shared history, not a second taking of 0310.
g checkout -q -b feature/adr-c master
commit_file .ai/repo/adrs/0310-c.md '# ADR 0310: a draft' 'docs(adr): 0310, a draft'
g push -q origin feature/adr-c
g checkout -q -b feature/adr-c-final feature/adr-c
g mv .ai/repo/adrs/0310-c.md .ai/repo/adrs/0310-c-final.md
g commit -q -m 'docs(adr): 0310, its final name'
g push -q origin feature/adr-c-final
g checkout -q -b feature/adr-c-other feature/adr-c
commit_file .ai/repo/adrs/0311-other.md '# ADR 0311: on the 0310 draft' 'docs(adr): 0311'
g push -q origin feature/adr-c-other
g checkout -q master
git -C "$C" fetch -q origin '+refs/heads/*:refs/remotes/origin/*'
expect_exit 0 env MJ_ROOT="$C" "$CHECK" --added --head origin/feature/adr-c-final --exclude origin/feature/adr-c-final
expect_no_grep 'COLLISION'
expect_row_under "$NUMBER_RELATED" '^  origin/feature/adr-c +\.ai/repo/adrs/0310-c\.md$'
expect_row_under "$NUMBER_RELATED" '^  origin/feature/adr-c-other +\.ai/repo/adrs/0310-c\.md$'

echo "    a new path added independently elsewhere is refused by name; its own branch, related work and copies pass; blindness is 12"
echo "    one number under two names and one flag or capability id on two branches are refused; trains and follow-ups pass; a double rewrite is reported"
