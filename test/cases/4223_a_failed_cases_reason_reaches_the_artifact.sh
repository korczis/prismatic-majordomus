# majordomus-covers: none
# What a failed case said reaches a file CI keeps, not only the job's log (I4223).
#
# Master's validate 40086406859 failed 56_derived_current_gate on suite-shard 3, and the
# shard's artifact, suite.tsv, held "FAIL" and 150 seconds: the case's own reason ("the gate
# took 31s at best of two") was in that job's log alone, which nobody counting failures later
# reads. With MJ_TEST_FAILURES set, test/run.sh writes the output of every case that failed or
# timed out into that file under a "== <name>" line, and validate.yml uploads it per shard.
#
# What it proves:
#   1  a failed case's output is in the file under its name; a passing and a declining case
#      are not, and a passing case's output is not carried along with it
#   2  the file is the run's own: a second run with nothing failing leaves it empty, never
#      the first run's failures read as the second's
#   3  a case runs without the variable, so a case that drives the runner itself cannot
#      write into the outer run's file
#   4  validate.yml sets it for the sharded suite, outside the checkout, and uploads it
. "$ROOT/test/lib.sh"

fail() { printf '    %s\n' "$*"; exit 1; }

H="$T/harness"
mkdir -p "$H/test/cases" "$H/lib"
cp "$ROOT/test/run.sh" "$ROOT/test/lib.sh" "$H/test/"; cp "$ROOT/lib/sha256.sh" "$H/lib/"
cat > "$H/test/cases/01_passes.sh" <<'SH'
. "$ROOT/test/lib.sh"
echo "    a line a passing case prints"
SH
cat > "$H/test/cases/02_declines.sh" <<'SH'
. "$ROOT/test/lib.sh"
skip "a precondition this fixture does not meet"
SH
cat > "$H/test/cases/03_fails.sh" <<'SH'
. "$ROOT/test/lib.sh"
[ -z "${MJ_TEST_FAILURES:-}" ] || { echo "    the case was handed MJ_TEST_FAILURES"; exit 1; }
echo "    the gate took 31s at best of two; it runs on every commit and must not"
exit 1
SH

F="$T/failures.log"
rc=0; MJ_TEST_JOBS=2 MJ_TEST_FAILURES="$F" bash "$H/test/run.sh" > "$T/one.out" 2>&1 || rc=$?
[ "$rc" != 0 ] || { cat "$T/one.out"; fail "a run with a failed case exited 0"; }
[ -f "$F" ] || { cat "$T/one.out"; fail "the run wrote no $F"; }

# 1. the failed case, by name and in its own words; nothing of the others
grep -qx '== 03_fails' "$F" || { cat "$F"; fail "the failures file does not name 03_fails"; }
grep -q 'the gate took 31s at best of two' "$F" || { cat "$F"; fail "the failures file lacks the case's reason"; }
! grep -q 'MJ_TEST_FAILURES' "$F" || { cat "$F"; fail "the case was handed the outer run's file"; }
! grep -q '== 01_passes\|== 02_declines' "$F" || { cat "$F"; fail "a case that did not fail is in the failures file"; }
! grep -q 'a line a passing case prints' "$F" || { cat "$F"; fail "a passing case's output was carried"; }

# 2. a run with nothing failing leaves the file empty
rm "$H/test/cases/03_fails.sh"
rc=0; MJ_TEST_JOBS=2 MJ_TEST_FAILURES="$F" bash "$H/test/run.sh" > "$T/two.out" 2>&1 || rc=$?
[ "$rc" = 0 ] || { cat "$T/two.out"; fail "a run with nothing failed exited $rc"; }
[ -f "$F" ] && [ ! -s "$F" ] || { cat "$F"; fail "a green run left the first run's failures in the file"; }

# 4. the wiring in CI: set for the sharded suite, outside the checkout, uploaded per shard
W="$ROOT/.github/workflows/validate.yml"
grep -q 'MJ_TEST_FAILURES: \${{ runner.temp }}/suite-failures.log' "$W" \
  || fail "validate.yml does not hand the sharded suite MJ_TEST_FAILURES under runner.temp"
grep -q 'name: suite-failures-\${{ matrix.shard }}' "$W" \
  || fail "validate.yml uploads no suite-failures-<shard> artifact"
grep -q 'path: \${{ runner.temp }}/suite-failures.log' "$W" \
  || fail "the suite-failures artifact does not carry the file the suite writes"
