# A case that declined to run is not a case that passed, and the ledger must never read it
# as one. Proves `a-skipped-case-is-not-a-proof`.
#
# The defect this case holds shut is a join of two honest halves. `test/run.sh` wrote one
# word for every case that exited 0, and a case that cannot meet a precondition -- no jq,
# no zola, no built executable -- said so with `echo "    skip: ..."; exit 0`. So the TSV
# the runner hands `majordomus evidence record` carried `ok` for a case that asserted
# nothing, the ledger recorded a pass, and the claim that case names read as supported on
# the strength of a run that measured nothing. The ledger already had the word for this --
# `Outcome::Skip`, which does not prove -- and no runner in this repository could write it.
#
# What it proves, in order:
#   1  a case that calls `skip` is reported as a skip and written into the report as SKIP,
#      while a case that passed is still `ok` and a case that failed is still FAIL
#   2  a skip is not a failure: the run's exit status is 0, and a run that only skipped is
#      still a run rather than "no cases found"
#   3  the parallel phase agrees with the serial one, word for word
#   4  the skip status is not the declaration: a case that ends with 4 because a command
#      failed with 4 (`jq -e` with no result) and never called `skip` is a FAIL and turns
#      the run red, in both phases
#   5  a SKIP recorded into the ledger keeps its own outcome and does NOT prove the claim
#      the case names -- the claim is not `proven`, and the guarantee stays a finding
#   6  the same TSV with `ok` in that one field does prove it, which is what makes 5 an
#      assertion about the word rather than about the fixture
. "$ROOT/test/lib.sh"
command -v jq >/dev/null 2>&1 || skip "no jq"
MJB="$(rust_bin)" || rust_bin_exit $?
export MAJORDOMUS_SHARE="$ROOT/share"

W="$(mktemp -d "${TMPDIR:-/tmp}/mj413.XXXXXX")"; trap 'rm -rf "$W"' EXIT

# ---------------------------------------------------------------- the harness
# The runner under test, over three cases of its own, outside this checkout: driving the
# real test/cases/ would mean running the whole suite to ask one question about one word.
H="$W/harness"
mkdir -p "$H/test/cases"
cp "$ROOT/test/run.sh" "$ROOT/test/lib.sh" "$H/test/"
cat > "$H/test/cases/01_passes.sh" <<'SH'
. "$ROOT/test/lib.sh"
expect_exit 0 true
SH
cat > "$H/test/cases/02_declines.sh" <<'SH'
. "$ROOT/test/lib.sh"
skip "a precondition this fixture does not meet"
echo "    the case continued after skip" >&2
exit 1
SH
cat > "$H/test/cases/03_fails.sh" <<'SH'
. "$ROOT/test/lib.sh"
expect_exit 0 false
SH

run_harness() {  # run_harness <name> [env assignments...] -- the runner, its output into $W
  local name="$1"; shift
  local rc=0
  env "$@" MJ_TEST_REPORT="$W/$name.tsv" bash "$H/test/run.sh" > "$W/$name.out" 2>&1 || rc=$?
  printf '%s' "$rc" > "$W/$name.rc"
  LAST_OUT="$(cat "$W/$name.out")"
}
rc_of() { cat "$W/$1.rc"; }
row() {          # row <name> <case> -- the result field of that case's report line
  awk -F'\t' -v c="$2" '$1 == c { print $2 }' "$W/$1.tsv"
}

# ---------------------------------------------------------------- 1. three words
run_harness serial
expect_grep '^skip 02_declines$'
expect_grep '^ok   01_passes$'
expect_grep '^FAIL 03_fails$'
expect_grep 'skip: a precondition this fixture does not meet'
expect_grep '^tests: 1 passed, 1 failed, 1 skipped$'
# the case stopped at `skip`: everything after it would have failed the case
expect_no_grep 'the case continued after skip'

[ "$(row serial 01_passes)" = "ok" ] \
  || { echo "    the passing case is recorded as '$(row serial 01_passes)', not ok"; exit 1; }
[ "$(row serial 03_fails)" = "FAIL" ] \
  || { echo "    the failing case is recorded as '$(row serial 03_fails)', not FAIL"; exit 1; }
# the assertion this whole case exists for
[ "$(row serial 02_declines)" = "SKIP" ] \
  || { echo "    a case that declined is recorded as '$(row serial 02_declines)', not SKIP"; exit 1; }

# ---------------------------------------------------------------- 2. a skip is not a failure
# and a skip is not "nothing ran" either: the run selected a case, the case answered, and
# reporting exit 2 ("no case matches") would send a reader looking for a name that is there.
rm "$H/test/cases/03_fails.sh"
run_harness passing
[ "$(rc_of passing)" = "0" ] || { echo "    a run whose only non-pass is a skip exited $(rc_of passing)"; exit 1; }
expect_grep '^tests: 1 passed, 0 failed, 1 skipped$'

rc=0
MJ_TEST_REPORT="$W/only.tsv" bash "$H/test/run.sh" 02_declines > "$W/only.out" 2>&1 || rc=$?
LAST_OUT="$(cat "$W/only.out")"
[ "$rc" = 0 ] || { echo "    selecting a case that declines exited $rc:"; cat "$W/only.out"; exit 1; }
expect_grep '^tests: 0 passed, 0 failed, 1 skipped$'
expect_no_grep 'no case matches'
expect_no_grep 'no cases found'
[ "$(awk -F'\t' '$1 == "02_declines" { print $2 }' "$W/only.tsv")" = "SKIP" ] \
  || { echo "    the selected case that declined was not recorded as SKIP"; exit 1; }

# ---------------------------------------------------------------- 3. both phases agree
# The parallel phase renders its verdicts from files the workers wrote, on a second code
# path. A runner that told the truth serially and `ok` in parallel would be worse than one
# that never told it at all, because the suite runs in parallel under CI.
run_harness parallel MJ_TEST_JOBS=2
[ "$(row parallel 02_declines)" = "SKIP" ] \
  || { echo "    the parallel phase recorded '$(row parallel 02_declines)' for a case that declined"; exit 1; }
expect_grep '^skip 02_declines$'
expect_grep '^tests: 1 passed, 0 failed, 1 skipped$'

# ---------------------------------------------------------------- 4. a status is not a declaration
# A case runs under `set -e`, so it ends with the status of whatever command failed, and
# `jq -e` fails with 4 when it produced no result -- on the empty output of a command that
# broke, say. That is the skip status. A runner that read the number alone would record the
# failure as a case that declined and keep the run green, which is the defect this claim
# closes arriving from the other side: a case that asserted and failed, read as one that
# never looked. Only `skip` declares a skip.
cat > "$H/test/cases/04_fails_with_four.sh" <<'SH'
. "$ROOT/test/lib.sh"
: > out.json
jq -e '.schema == 1' out.json >/dev/null
SH
# the fixture must really end with the skip status, or this section asserts nothing
mkdir -p "$W/four"; rc=0
( cd "$W/four" && env -u MJ_SKIP_MARK ROOT="$H" bash -eu "$H/test/cases/04_fails_with_four.sh" ) \
  >/dev/null 2>&1 || rc=$?
[ "$rc" = 4 ] || { echo "    the fixture meant to fail with status 4 exited $rc"; exit 1; }
for jobs in 1 2; do
  run_harness "four$jobs" MJ_TEST_JOBS="$jobs"
  [ "$(rc_of "four$jobs")" != 0 ] \
    || { echo "    a run whose case failed with status 4 exited 0 (MJ_TEST_JOBS=$jobs)"; exit 1; }
  word="$(row "four$jobs" 04_fails_with_four)"
  [ "$word" = "FAIL" ] || {
    echo "    a case that failed with status 4 is recorded as '$word', not FAIL (MJ_TEST_JOBS=$jobs)"
    exit 1; }
  # and the case that did declare it, in the same run, is still a skip
  word="$(row "four$jobs" 02_declines)"
  [ "$word" = "SKIP" ] || {
    echo "    the declared skip beside it is recorded as '$word' (MJ_TEST_JOBS=$jobs)"; exit 1; }
  expect_grep '^FAIL 04_fails_with_four$'
  expect_grep '^tests: 1 passed, 1 failed, 1 skipped$'
done
rm "$H/test/cases/04_fails_with_four.sh"

# ---------------------------------------------------------------- the fixture repository
# A repository with a claims matrix of its own, as 124_evidence builds one: the states
# below are reached by recording runs, and a case that recorded into this checkout would be
# writing into the thing it measures.
"$MJ" init >/dev/null
mkdir -p docs lib test/cases
cat > docs/CLAIMS.yaml <<'YAML'
version: 1
claims:
  - id: alpha-holds
    claim: Alpha holds
    source: docs/ALPHA.md
    implementation: lib/alpha.sh
    test: test/cases/01_alpha.sh
    status: guaranteed
YAML
printf '# Alpha\n' > docs/ALPHA.md
printf '#!/usr/bin/env bash\n# alpha\n' > lib/alpha.sh
printf '# the alpha case\n' > test/cases/01_alpha.sh
git add -A >/dev/null && git commit -qm fixture

ev() {           # ev <name> <args...> -- the JSON answer into $W/<name>.json
  local name="$1"; shift
  run_quiet "$W/$name.err" "$MJB" evidence --repo "$T" --format json "$@" > "$W/$name.json"
}
jqe() {          # jqe <name> <filter> <what broke>
  jq -e "$2" "$W/$1.json" >/dev/null 2>&1 || { printf '    %s\n' "$3"; jq -c . "$W/$1.json" | head -c 2000; echo; return 1; }
}

# ---------------------------------------------------------------- 5. a SKIP proves nothing
# The word the runner now writes, read back by the recorder. It is recorded -- a skip is a
# fact about the run and hiding it would leave the claim reading `not run`, which says less
# than the truth -- and it does not support the guarantee.
printf '01_alpha\tSKIP\t1\tserial\n' > "$W/skip.tsv"
expect_exit 0 "$MJB" evidence --repo "$T" record --suite "$W/skip.tsv"
expect_grep 'recorded +1 execution\(s\), 0 passing'
ev skipped claim alpha-holds
jqe skipped '.execution.outcome == "skip"' \
  "a case the runner reported as SKIP was recorded with some other outcome"
jqe skipped '.state != "proven" and .state != "inputs_unchanged"' \
  "a case that declined to run was read as the proof of the claim it names"
ev skipped_show show
jqe skipped_show '[.findings[].claim] == ["alpha-holds"]' \
  "a guarantee whose only run declined is not reported as unsupported"
expect_exit 10 "$MJB" evidence --repo "$T" show --check

# ---------------------------------------------------------------- 6. the word is the difference
# The same fixture, the same commit, the same report -- one field changed. Without this the
# section above would assert only that the fixture is unprovable.
printf '01_alpha\tok\t1\tserial\n' > "$W/pass.tsv"
expect_exit 0 "$MJB" evidence --repo "$T" record --suite "$W/pass.tsv"
ev passed claim alpha-holds
jqe passed '.execution.outcome == "pass" and .state == "proven"' \
  "the identical report with ok in that field does not prove the claim, so the skip proved nothing about the word"
expect_exit 0 "$MJB" evidence --repo "$T" show --check
exit 0
