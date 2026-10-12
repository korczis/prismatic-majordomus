# majordomus-covers: none
# A stacked link is planned against the trunk, not against the branch it is stacked on.
#
# validate.yml's plan job decides which gates a pull request runs at all, from the paths that
# changed between a base and HEAD. The base was HEAD^1 of the merge commit CI checks out: the
# tip of the branch the pull request merges into. For a pull request into the trunk that is the
# point the change leaves the trunk. For a link of a stacked train it is the lower link's tip,
# so the plan held the link's own delta only: a docs-only link above a Rust link planned no
# rust and no coverage job. Seventeen links were planned and passed that way, and their top
# then showed 2615 uncovered changed items against master (#963).
#
# This case does not read the workflow for a phrase. It cuts the step's own lines out of
# validate.yml, from `PR_BASE=HEAD^1` to the `fi` that closes them, and runs them the way the
# runner does (`bash -e -o pipefail`) in the checkout the runner has: the pull request's merge
# ref fetched at depth 2 from an origin whose trunk moved on after the fork. It holds:
#
#   1  the checkout is the runner's: shallow, and the fork point is not in it
#   2  a link stacked on another link's branch is planned from the fork point — not from the
#      lower link's tip, and not from where the trunk has moved to — so the lower link's crate
#      path is in what the plan reads; a second run in the same, no longer shallow, clone
#      answers the same
#   3  a pull request into the trunk, a push and a dispatch keep HEAD^1 and fetch nothing
#   4  a trunk that cannot be fetched, and a trunk that shares no history with HEAD, are each
#      a named refusal with exit 12, with and without errexit, never a plan
#   5  what the lines compute is what the pull request is planned from, and the two names they
#      read arrive through the environment, never interpolated into the script
#
# The real scripts/ci-plan is not called: it diffs in its own repository, not in a fixture.
. "$ROOT/test/lib.sh"
VL="$ROOT/.github/workflows/validate.yml"
expect_file "$VL"

# ---------------------------------------------------------------- the step's own lines
BLOCK="$T/plan-base.sh"
awk '
  !f && /^[ \t]*PR_BASE=HEAD\^1$/ { f = 1; match($0, /^[ \t]*/); cut = RLENGTH }
  f { print substr($0, cut + 1) }
  f && /^[ \t]*fi$/ { exit }
' "$VL" > "$BLOCK"
[ -s "$BLOCK" ] || { echo "    validate.yml has no PR_BASE=HEAD^1 line: the plan's base is computed somewhere this case does not read"; exit 1; }
[ "$(tail -n 1 "$BLOCK")" = "fi" ] || { echo "    the lines after PR_BASE=HEAD^1 do not close with fi at their own depth:"; cat "$BLOCK"; exit 1; }
printf '%s\n' 'printf "PR_BASE=%s\n" "$PR_BASE"' >> "$BLOCK"

# the runner's shell for a `run:` step, with the three names the step reads from its env
plan_base() {
  local dir="$1"; shift
  ( cd "$dir" && env "$@" bash --noprofile --norc -e -o pipefail "$BLOCK" )
}
# the same lines with no errexit: a refusal that is the step's own does not depend on it
plan_base_lax() {
  local dir="$1"; shift
  ( cd "$dir" && env "$@" bash --noprofile --norc "$BLOCK" )
}

# ---------------------------------------------------------------- an origin with a stacked train
MR=7
CRATE="apps/majordomus-cli/src/x.rs"
O="$T/origin.git"
S="$T/src"
git init -q --bare "$O"
git init -q "$S"
git -C "$S" symbolic-ref HEAD refs/heads/master
git -C "$S" config user.email t@example.com
git -C "$S" config user.name t
echo trunk > "$S/README.md"
git -C "$S" add -A >/dev/null; git -C "$S" commit -qm "trunk"
FORK="$(git -C "$S" rev-parse HEAD)"
# the lower link touches the crate; the upper link touches a document only
git -C "$S" checkout -qb link1
mkdir -p "$S/$(dirname "$CRATE")"
echo 'pub fn x() {}' > "$S/$CRATE"
git -C "$S" add -A >/dev/null; git -C "$S" commit -qm "link1: a crate source"
LINK1="$(git -C "$S" rev-parse HEAD)"
git -C "$S" checkout -qb link2
echo doc > "$S/DOC.md"
git -C "$S" add -A >/dev/null; git -C "$S" commit -qm "link2: a document"
# the trunk moves on after the fork, as it does under every train
git -C "$S" checkout -q master
echo moved > "$S/MOVED.md"
git -C "$S" add -A >/dev/null; git -C "$S" commit -qm "trunk: moved on"
MOVED="$(git -C "$S" rev-parse HEAD)"
# a branch that shares no history with any of them
git -C "$S" checkout -q --orphan island
git -C "$S" rm -rqf . >/dev/null
echo island > "$S/ISLAND.md"
git -C "$S" add -A >/dev/null; git -C "$S" commit -qm "island"
# the merge commit GitHub makes for link2's pull request into link1
git -C "$S" checkout -q --detach link1
git -C "$S" merge -q --no-ff -m "merge link2 into link1" link2
MERGE="$(git -C "$S" rev-parse HEAD)"
git -C "$S" push -q "$O" master link1 link2 island "HEAD:refs/pull/${MR}/merge"

# actions/checkout with fetch-depth: 2 — over file://, because a local path ignores the depth
runner_checkout() {
  git init -q "$1"
  git -C "$1" remote add origin "file://$O"
  git -C "$1" fetch -q --no-tags --depth=2 origin "+refs/pull/${MR}/merge:refs/remotes/pull/${MR}/merge"
  git -C "$1" checkout -q --detach "refs/remotes/pull/${MR}/merge"
}
names() { git -C "$1" --no-pager diff --name-only "$2" HEAD | LC_ALL=C sort | tr '\n' ' '; }

# ---------------------------------------------------------------- 1. the runner's checkout
C="$T/ci"
runner_checkout "$C"
[ "$(git -C "$C" rev-parse HEAD)" = "$MERGE" ] || { echo "    the fixture checked out something other than the merge ref"; exit 1; }
[ "$(git -C "$C" rev-parse --is-shallow-repository)" = true ] \
  || { echo "    the fixture's checkout is not shallow, so it proves nothing about the runner's"; exit 1; }
git -C "$C" cat-file -e "$FORK^{commit}" 2>/dev/null \
  && { echo "    the fork point is already in the depth-2 checkout, so nothing here needs the trunk's history"; exit 1; }
[ "$(git -C "$C" rev-parse HEAD^1)" = "$LINK1" ] || { echo "    HEAD^1 of the merge ref is not the lower link's tip"; exit 1; }
# the shape that hid the debt: against HEAD^1 the upper link changed a document and nothing else
[ "$(names "$C" HEAD^1)" = "DOC.md " ] || { echo "    against HEAD^1 the link's delta is '$(names "$C" HEAD^1)', not its own document alone"; exit 1; }

# ---------------------------------------------------------------- 2. a stacked link
expect_exit 0 plan_base "$C" EVENT=pull_request BASE_REF=link1 TRUNK=master
expect_grep "^PR_BASE=$FORK\$"
expect_grep "^stacked on link1: planned from the merge-base with master, $FORK\$"
expect_no_grep "PR_BASE=($MOVED|$LINK1|HEAD)"
got="$(names "$C" "$FORK")"
[ "$got" = "DOC.md $CRATE " ] \
  || { echo "    from the fork point the plan would read '$got', not the whole stack (DOC.md and $CRATE) and nothing the trunk added"; exit 1; }
# the fetch deepened the clone; a re-run of the job meets a complete one and answers the same
[ "$(git -C "$C" rev-parse --is-shallow-repository)" = false ] \
  || { echo "    the clone is still shallow after the stacked plan, so the merge-base was read from a cut history"; exit 1; }
expect_exit 0 plan_base "$C" EVENT=pull_request BASE_REF=link1 TRUNK=master
expect_grep "^PR_BASE=$FORK\$"

# ---------------------------------------------------------------- 3. everything else keeps HEAD^1
C3="$T/ci3"
runner_checkout "$C3"
expect_exit 0 plan_base "$C3" EVENT=pull_request BASE_REF=master TRUNK=master
expect_grep '^PR_BASE=HEAD\^1$'
expect_no_grep 'stacked on'
expect_exit 0 plan_base "$C3" EVENT=push BASE_REF= TRUNK=master
expect_grep '^PR_BASE=HEAD\^1$'
expect_exit 0 plan_base "$C3" EVENT=workflow_dispatch BASE_REF= TRUNK=master
expect_grep '^PR_BASE=HEAD\^1$'
# and none of them paid for the trunk's history
[ "$(git -C "$C3" rev-parse --is-shallow-repository)" = true ] \
  || { echo "    a pull request into the trunk, a push or a dispatch fetched the trunk's history it does not need"; exit 1; }
git -C "$C3" rev-parse -q --verify refs/remotes/origin/master >/dev/null \
  && { echo "    a run that plans from HEAD^1 fetched origin/master"; exit 1; }

# ---------------------------------------------------------------- 4. no base is a refusal
# a trunk that cannot be fetched
expect_exit 12 plan_base "$C3" EVENT=pull_request BASE_REF=link1 TRUNK=absent
expect_grep '^::error::origin/absent could not be fetched; a pull request stacked on link1 cannot be planned against the trunk$'
expect_no_grep '^PR_BASE='
# the refusal ends the step where it is made: without errexit too, nothing goes on to look for a
# merge-base with a ref that was never fetched and report that instead
expect_exit 12 plan_base_lax "$C3" EVENT=pull_request BASE_REF=link1 TRUNK=absent
expect_grep '^::error::origin/absent could not be fetched'
expect_no_grep 'has no merge-base'
expect_no_grep '^PR_BASE='
# a trunk that is there and shares no history with HEAD
C4="$T/ci4"
runner_checkout "$C4"
expect_exit 12 plan_base "$C4" EVENT=pull_request BASE_REF=link1 TRUNK=island
expect_grep '^::error::HEAD has no merge-base with origin/island; a pull request stacked on link1 cannot be planned against the trunk$'
expect_no_grep '^PR_BASE='
expect_exit 12 plan_base_lax "$C4" EVENT=pull_request BASE_REF=link1 TRUNK=island
expect_grep '^::error::HEAD has no merge-base with origin/island'
expect_no_grep '^PR_BASE='

# ---------------------------------------------------------------- 5. the wiring
line_of() { grep -nF -- "$2" "$1" | head -n 1 | cut -d: -f1; }
set_at="$(grep -nE '^[[:space:]]*PR_BASE=HEAD\^1$' "$VL" | head -n 1 | cut -d: -f1)"
used_at="$(line_of "$VL" '*) scripts/ci-plan --base "$PR_BASE" --head HEAD > plan.json ;;')"
[ -n "$used_at" ] || { echo "    a pull request is not planned from PR_BASE, so what these lines compute decides nothing"; exit 1; }
[ "$used_at" -gt "$set_at" ] || { echo "    the pull request is planned (line $used_at) before PR_BASE is computed (line $set_at)"; exit 1; }
sed -n "$((used_at - 4)),$((used_at - 1))p" "$VL" | grep -qE '^[[:space:]]*pull_request\)$' \
  || { echo "    the arm planned from PR_BASE is not the pull request's"; exit 1; }
grep -qE '^[[:space:]]*BASE_REF: \$\{\{ github\.base_ref \}\}$' "$VL" \
  || { echo "    BASE_REF is not the branch the pull request merges into"; exit 1; }
grep -qE '^[[:space:]]*TRUNK: \$\{\{ github\.event\.repository\.default_branch \}\}$' "$VL" \
  || { echo "    TRUNK is not the repository's default branch"; exit 1; }
grep -qF '${{' "$BLOCK" \
  && { echo "    the lines interpolate an expression: a branch name would be written into the script"; exit 1; }
exit 0
