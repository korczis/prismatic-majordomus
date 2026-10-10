# majordomus-covers: none
# claims: fixture-cannot-write-its-repository
# A fixture cannot write the repository it runs in.
#
# Git tells a hook which repository it is in through the environment: a pre-push hook run
# from a linked worktree gets GIT_DIR. A suite started from that hook hands the variable to
# every case, and a case builds repositories: `git init`, `git branch -M master`,
# `git commit`, `git config user.email t@example.com`, `git worktree add`, `git tag`. With
# GIT_DIR set, each of those acts on the repository being pushed and not on the fixture. On
# 2026-10-10 that renamed a feature branch to master, moved master and gh-pages onto fixture
# commits, and left a fixture identity and core.bare=true in the configuration every
# worktree of the repository shares. Nothing reported it for an hour.
#
# test/run.sh, scripts/ci/run-plan and scripts/shell-coverage, which builds the cases' fixtures
# itself, therefore drop every variable git reads its repository from before anything they
# start can run. This case holds that from both sides:
#
#   1  a suite started by a real pre-push hook, in a real push from a linked worktree, runs
#      its fixture and leaves the pushed repository byte-identical: refs, both HEADs, the
#      worktree list, the configuration and the files of both checkouts
#   2  the same with all five variables exported by hand, which is what other hooks and
#      `git -C` add to the one a push exports
#   3  the same for a gate dispatched by scripts/ci/run-plan, and for a case run under
#      tracing by scripts/shell-coverage
#   4  the two variables that carry `git -c` settings are kept, so a case's git is still
#      told to start no housekeeping
#   5  with the loop removed from a copy of each of the three scripts, the same run does
#      write the repository it was started from, so 1 to 3 are not passing for another reason
. "$ROOT/test/lib.sh"

command -v jq >/dev/null 2>&1 || skip "no jq"

no() { printf '    %s\n' "$*"; exit 1; }

# ---------------------------------------------------------------- a suite of one fixture
# A copy of the runner, of the dispatcher and of the coverage harness with one case, which
# does what the cases of the real suite do in their fixtures. It asserts nothing and stops at nothing: what it saw
# and what it moved are read from outside.
suite() {  # directory
  mkdir -p "$1/test/cases" "$1/scripts/ci"
  cp "$ROOT/test/run.sh" "$1/test/run.sh"
  cp "$ROOT/scripts/ci/run-plan" "$1/scripts/ci/run-plan"
  cp "$ROOT/scripts/shell-coverage" "$1/scripts/shell-coverage"
  cat > "$1/test/cases/zz_builds_a_repository.sh" <<'CASE'
set +e
env | grep -E '^GIT_(DIR|WORK_TREE|INDEX_FILE|COMMON_DIR|PREFIX)=' > "$MJ_1030_SEEN"
git config --get gc.auto > "$MJ_1030_SEEN.quiet"
git init -q .
git config user.email t@example.com
git config user.name t
git branch -M master
git commit -q --allow-empty -m fixture
git tag fixture-tag
git worktree add -q linked -b fixture/linked
git init -q --bare bare.git
exit 0
CASE
  # the gate the dispatcher runs: the same fixture, in a directory of its own
  cat > "$1/gate.sh" <<GATE
d="\$(mktemp -d "\${TMPDIR:-/tmp}/mj1030-gate.XXXXXX")" || exit 1
( cd "\$d" && bash "$1/test/cases/zz_builds_a_repository.sh" ); rc=\$?
rm -rf "\$d"; exit "\$rc"
GATE
  jq -n --arg runs "bash $1/gate.sh" \
    '{mode: "full", reason: "case 1030", selected: ["builds-a-repository"],
      gates: [{id: "builds-a-repository", job: "structure", selected: true, runs: $runs}]}' \
    > "$1/plan.json"
}

# ---------------------------------------------------------------- the repository being pushed
# A repository with a commit and a file, a linked worktree on a branch of its own, and a
# pre-push hook that starts the suite it is given. Its commits carry an identity through the
# command line, so that its configuration holds none before the fixture runs.
pushed() {  # name suite-directory entry-point...
  local name="$1" dir="$2"; shift 2
  git init -q "$T/$name"
  git -C "$T/$name" symbolic-ref HEAD refs/heads/trunk
  printf 'a file of the checkout\n' > "$T/$name/README.md"
  git -C "$T/$name" add README.md
  git -C "$T/$name" -c user.email=owner@example.org -c user.name=owner commit -qm 'the repository being pushed'
  git -C "$T/$name" worktree add -q -b feature/work "$T/$name-wt"
  git init -q --bare "$T/$name-remote.git"
  mkdir -p "$T/$name-hooks"
  {
    printf '#!/bin/sh\n'
    printf 'env | grep "^GIT_DIR=" > "%s"\n' "$T/$name.hook-env"
    printf 'exec'; printf ' "%s"' "$@"; printf ' > "%s" 2>&1\n' "$T/$name.log"
  } > "$T/$name-hooks/pre-push"
  chmod +x "$T/$name-hooks/pre-push"
  git -C "$T/$name" config core.hooksPath "$T/$name-hooks"
}

# Everything of that repository a fixture could move, as bytes: every ref with its commit,
# the HEAD of both checkouts, the worktree list, the shared configuration, and the files
# and status of both checkouts. Errors are part of it: a repository that stopped answering
# has changed.
snapshot() {  # name file
  {
    git -C "$T/$1" for-each-ref --format='%(refname) %(objectname)'
    cat "$T/$1/.git/HEAD" "$T/$1/.git/worktrees/$1-wt/HEAD"
    ls "$T/$1/.git/worktrees"
    git -C "$T/$1-wt" worktree list --porcelain
    cat "$T/$1/.git/config"
    ( cd "$T/$1" && find . -path ./.git -prune -o -print | LC_ALL=C sort )
    ( cd "$T/$1-wt" && find . -name .git -prune -o -print | LC_ALL=C sort )
    git -C "$T/$1" status --porcelain
    git -C "$T/$1-wt" status --porcelain
  } > "$2" 2>&1 || true
}

# What the fixture saw, for the run whose record is $1: no variable naming a repository, and
# the housekeeping setting the test library put in the environment of this case.
saw_no_repository() {  # record what
  [ -f "$1" ] || no "$2: the fixture left no record, so it did not run"
  [ ! -s "$1" ] || no "$2: git's repository variables reached the fixture: $(tr '\n' ' ' < "$1")"
  [ "$(cat "$1.quiet")" = 0 ] \
    || no "$2: the fixture's git was no longer told gc.auto=0 (got '$(cat "$1.quiet")'): the \`git -c\` settings of the environment were dropped with the repository variables"
}
unchanged() {  # name what
  snapshot "$1" "$T/$1.after"
  cmp -s "$T/$1.before" "$T/$1.after" || {
    printf '    %s: the fixture wrote the repository the suite was started from:\n' "$2"
    diff "$T/$1.before" "$T/$1.after" | sed 's/^/    | /' | head -20
    exit 1; }
}

S="$T/suite"; suite "$S"
grep -q -- '--local-env-vars' "$S/test/run.sh" || no "test/run.sh no longer asks git which variables name a repository"
grep -q -- '--local-env-vars' "$S/scripts/ci/run-plan" || no "scripts/ci/run-plan no longer asks git which variables name a repository"
grep -q -- '--local-env-vars' "$S/scripts/shell-coverage" || no "scripts/shell-coverage no longer asks git which variables name a repository"

# ---------------------------------------------------------------- 1. a real push, a real hook
pushed outer "$S" bash "$S/test/run.sh" zz_builds_a_repository
snapshot outer "$T/outer.before"
MJ_1030_SEEN="$T/outer.seen" git -C "$T/outer-wt" push -q "$T/outer-remote.git" feature/work \
  || no "the push was refused: the suite failed under the hook: $(tail -3 "$T/outer.log" 2>/dev/null)"
grep -q '^GIT_DIR=.*/outer/\.git/worktrees/outer-wt$' "$T/outer.hook-env" \
  || no "git handed the pre-push hook no GIT_DIR naming the linked worktree ($(cat "$T/outer.hook-env" 2>/dev/null)), so this run proves nothing about a hook"
grep -q '^ok   zz_builds_a_repository$' "$T/outer.log" || no "the fixture case did not pass under the hook: $(tail -3 "$T/outer.log")"
saw_no_repository "$T/outer.seen" "a suite started by a pre-push hook"
unchanged outer "a suite started by a pre-push hook"

# ---------------------------------------------------------------- 2. all five, by hand
hook_environment() {  # name command...
  local name="$1"; shift
  GIT_DIR="$T/$name/.git/worktrees/$name-wt" GIT_WORK_TREE="$T/$name-wt" \
    GIT_INDEX_FILE="$T/$name/.git/worktrees/$name-wt/index" GIT_COMMON_DIR="$T/$name/.git" \
    GIT_PREFIX="" "$@"
}
rm -f "$T/outer.seen" "$T/outer.seen.quiet"
MJ_1030_SEEN="$T/outer.seen" hook_environment outer bash "$S/test/run.sh" zz_builds_a_repository > "$T/outer.log" 2>&1 \
  || no "the suite failed with git's five repository variables exported: $(tail -3 "$T/outer.log")"
grep -q '^ok   zz_builds_a_repository$' "$T/outer.log" || no "the fixture case did not pass: $(tail -3 "$T/outer.log")"
saw_no_repository "$T/outer.seen" "a suite started with all five variables"
unchanged outer "a suite started with all five variables"

# ---------------------------------------------------------------- 3. a gate of the dispatcher
rm -f "$T/outer.seen" "$T/outer.seen.quiet"
MJ_1030_SEEN="$T/outer.seen" hook_environment outer bash "$S/scripts/ci/run-plan" --plan "$S/plan.json" > "$T/outer.log" 2>&1 \
  || no "the dispatcher failed with git's five repository variables exported: $(tail -3 "$T/outer.log")"
grep -q '^==> builds-a-repository: ok' "$T/outer.log" || no "the gate did not run: $(tail -3 "$T/outer.log")"
saw_no_repository "$T/outer.seen" "a gate dispatched with all five variables"
unchanged outer "a gate dispatched with all five variables"

# ---------------------------------------------------------------- 3. and a case under tracing
# The coverage harness makes the fixture itself and then reports over library files this
# copy of it does not have, so its own status says nothing here: the fixture's record does.
rm -f "$T/outer.seen" "$T/outer.seen.quiet"
MJ_1030_SEEN="$T/outer.seen" hook_environment outer bash "$S/scripts/shell-coverage" zz_builds_a_repository > "$T/outer.log" 2>&1 || true
[ -f "$T/outer.seen" ] || no "the coverage harness did not run the fixture: $(tail -3 "$T/outer.log")"
[ ! -s "$T/outer.seen" ] || no "a case run under tracing: git's repository variables reached the fixture: $(tr '\n' ' ' < "$T/outer.seen")"
unchanged outer "a case run under tracing with all five variables"

# ---------------------------------------------------------------- 5. without the loop, it does write
# The same suite with the loop cut out of all three scripts, each against a repository of
# its own. Before the mutants run, the cut is shown to have removed what it was aimed at and
# nothing that makes the script unreadable.
M="$T/mutant"; suite "$M"
for f in test/run.sh scripts/ci/run-plan scripts/shell-coverage; do
  sed '/^for mj_git_local in/,/^done$/d' "$S/$f" > "$M/$f"
  ! grep -q -- '--local-env-vars' "$M/$f" || no "the mutation left the loop in $f, so it would prove nothing"
  bash -n "$M/$f" || no "the mutation broke the syntax of $f"
  [ "$(( $(wc -l < "$S/$f") - $(wc -l < "$M/$f") ))" = 5 ] \
    || no "the mutation removed $(( $(wc -l < "$S/$f") - $(wc -l < "$M/$f") )) lines of $f and the loop is five"
done

pushed moved "$M" bash "$M/test/run.sh" zz_builds_a_repository
snapshot moved "$T/moved.before"
MJ_1030_SEEN="$T/moved.seen" git -C "$T/moved-wt" push -q "$T/moved-remote.git" feature/work > "$T/moved.push" 2>&1 || true
grep -q '^GIT_DIR=' "$T/moved.seen" 2>/dev/null \
  || no "without the loop the fixture still saw no GIT_DIR, so the loop is not what keeps it out: $(tail -3 "$T/moved.log" 2>/dev/null)"
snapshot moved "$T/moved.after"
! cmp -s "$T/moved.before" "$T/moved.after" \
  || no "without the loop a fixture started by the hook left the pushed repository untouched, so part 1 would pass with or without it"
[ "$(git config -f "$T/moved/.git/config" --get user.email)" = t@example.com ] \
  || no "without the loop the fixture's identity did not reach the shared configuration; the mutation no longer shows what happened on 2026-10-10"
[ "$(git config -f "$T/moved/.git/config" --get core.bare)" = true ] \
  || no "without the loop the fixture's \`git init --bare\` did not mark the pushed repository bare"
[ "$(cat "$T/moved/.git/worktrees/moved-wt/HEAD")" = "ref: refs/heads/master" ] \
  || no "without the loop the fixture's \`git branch -M master\` did not rename the branch being pushed"
[ -n "$(git --git-dir="$T/moved/.git" for-each-ref refs/tags/fixture-tag)" ] \
  || no "without the loop the fixture's tag did not reach the pushed repository"

pushed gated "$M" true
snapshot gated "$T/gated.before"
MJ_1030_SEEN="$T/gated.seen" hook_environment gated bash "$M/scripts/ci/run-plan" --plan "$M/plan.json" > "$T/gated.log" 2>&1 || true
grep -q '^GIT_DIR=' "$T/gated.seen" 2>/dev/null \
  || no "without the loop the gate still saw no GIT_DIR, so the loop is not what keeps it out: $(tail -3 "$T/gated.log" 2>/dev/null)"
snapshot gated "$T/gated.after"
! cmp -s "$T/gated.before" "$T/gated.after" \
  || no "without the loop a gate left the repository untouched, so part 3 would pass with or without it"

pushed traced "$M" true
snapshot traced "$T/traced.before"
MJ_1030_SEEN="$T/traced.seen" hook_environment traced bash "$M/scripts/shell-coverage" zz_builds_a_repository > "$T/traced.log" 2>&1 || true
grep -q '^GIT_DIR=' "$T/traced.seen" 2>/dev/null \
  || no "without the loop the traced fixture still saw no GIT_DIR, so the loop is not what keeps it out: $(tail -3 "$T/traced.log" 2>/dev/null)"
snapshot traced "$T/traced.after"
! cmp -s "$T/traced.before" "$T/traced.after" \
  || no "without the loop a traced fixture left the repository untouched, so the tracing half of part 3 would pass with or without it"

echo "    a suite started by a pre-push hook of a linked worktree, a gate of the dispatcher and a case under tracing leave the pushed repository byte-identical; without the loop the same fixture renames the pushed branch to master, marks the repository bare and writes its identity and a tag into it"
