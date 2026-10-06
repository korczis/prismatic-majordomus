# majordomus-covers: none
# CI fails on a skip, unless the case says where its subject is read instead.
#
# A skip is recorded as SKIP and never as a pass (#629, case 413), but a run whose cases all
# declined still exited 0: case 102 skipped "no zsh" on every CI run, and nothing turned red.
# `test/run.sh --no-skips` makes a skip a failure. The one honest exception is a case whose
# subject a job other than the one running it can read: 409's rendered half needs the built
# site, which the suite runners do not have when it runs. Such a case declares
#
#   # majordomus-skip-runs-in: <job>
#
# in its header, and `--runs-in <job>` runs exactly the cases that declare that job, with no
# skip tolerated there. So a declared case is excused in one place only because it is held in
# another.
#
# This case drives the runner itself over a fixture repository holding a copy of test/run.sh
# and test/lib.sh and cases written here, one shape each:
#
#   passes      exits 0
#   declines    skips, declaring nothing
#   elsewhere   skips, declaring `majordomus-skip-runs-in: site`
#   held        skips unless HELD_SUBJECT is set, declaring the same job
#
# and asserts what each invocation answers. Without `--no-skips` the runner's behaviour is
# unchanged: a skip is counted and the run passes.
. "$ROOT/test/lib.sh"

S="$(mktemp -d "${TMPDIR:-/tmp}/mj-829.XXXXXX")"
trap 'rm -rf "$S"' EXIT

R="$S/repo"
mkdir -p "$R/test/cases" "$R/bin" "$R/lib"
cp "$ROOT/test/run.sh" "$ROOT/test/lib.sh" "$R/test/"
cp "$ROOT/lib/sha256.sh" "$R/lib/"   # test/lib.sh sources it
: > "$R/bin/majordomus"

cat > "$R/test/cases/passes.sh" <<'CASE'
. "$ROOT/test/lib.sh"
true
CASE
cat > "$R/test/cases/declines.sh" <<'CASE'
. "$ROOT/test/lib.sh"
skip "a tool this fixture pretends is absent"
CASE
cat > "$R/test/cases/elsewhere.sh" <<'CASE'
# majordomus-skip-runs-in: site
. "$ROOT/test/lib.sh"
skip "the built site is read in the site job"
CASE
cat > "$R/test/cases/held.sh" <<'CASE'
# majordomus-skip-runs-in: site
. "$ROOT/test/lib.sh"
[ -n "${HELD_SUBJECT:-}" ] || skip "the subject is absent here"
true
CASE

# run <expected-exit> <description> -- <run.sh arguments...>: prints the output on a mismatch
run() {
  local want="$1" what="$2"; shift 3
  got=0; out="$(cd "$R" && bash test/run.sh "$@" 2>&1)" || got=$?
  [ "$got" = "$want" ] || { echo "    $what: expected exit $want, got $got"; printf '%s\n' "$out" | sed 's/^/      /'; exit 1; }
}

# 1. without --no-skips nothing changes: two cases pass, two decline, the run passes
run 0 "a plain run with a skip" -- passes declines
printf '%s\n' "$out" | grep -q '^tests: 1 passed, 0 failed, 1 skipped$' \
  || { echo "    a plain run did not count the skip as a skip:"; printf '%s\n' "$out" | sed 's/^/      /'; exit 1; }

# 2. --no-skips: an undeclared skip fails the run, and the line names the case
run 1 "--no-skips over an undeclared skip" -- --no-skips passes declines
printf '%s\n' "$out" | grep -q 'declines' && printf '%s\n' "$out" | grep -q -- '--no-skips' \
  || { echo "    the refusal does not name the case and the flag:"; printf '%s\n' "$out" | sed 's/^/      /'; exit 1; }

# 3. --no-skips: a skip declared to run in another job is excused here, and says where
run 0 "--no-skips over a skip declared to run in the site job" -- --no-skips passes elsewhere
printf '%s\n' "$out" | grep -q 'elsewhere.*site' \
  || { echo "    the excused skip does not say which job reads its subject:"; printf '%s\n' "$out" | sed 's/^/      /'; exit 1; }

# 4. --runs-in site selects exactly the cases that declare it, and there no skip is excused
run 1 "--runs-in site where the subject is still absent" -- --no-skips --runs-in site
printf '%s\n' "$out" | grep -q 'passes\|declines' \
  && { echo "    --runs-in site ran a case that does not declare site:"; printf '%s\n' "$out" | sed 's/^/      /'; exit 1; }
got=0; out="$(cd "$R" && HELD_SUBJECT=1 bash test/run.sh --no-skips --runs-in site held 2>&1)" || got=$?
[ "$got" = 0 ] || { echo "    the held case did not pass in its job once its subject was there: exit $got"; printf '%s\n' "$out" | sed 's/^/      /'; exit 1; }

# 5. a job no case declares is a usage error, not an empty success
run 2 "--runs-in a job no case declares" -- --no-skips --runs-in nowhere

echo "    an undeclared skip fails under --no-skips; a declared one is excused only where its job holds it"
