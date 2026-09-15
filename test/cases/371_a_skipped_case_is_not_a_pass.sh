# majordomus-covers: none
# A skipped case is not a pass.
#
# 57 cases used to print "jq absent; skipping" and then exit 0, and test/run.sh counted
# exit 0 as a pass. On a machine or a CI runner without jq or zola, the required behavioural
# evidence of those cases became green without one assertion running. The rule it breaks is
# project.never-reported-is-not-green: a check that has not spoken is unknown, not passing.
#
# The contract this case holds (test/lib.sh skip_case, test/run.sh, docs/CI.md):
#   - a case skips through skip_case "<reason>", which exits 77;
#   - the runner reports it as SKIP, never ok, counts and names it, and exits 0 locally;
#   - with --no-skips (CI's invocation) the same case is a FAIL and the run exits non-zero;
#   - both in the serial runner and in the parallel one, whose verdicts come from files;
#   - no case in this repository skips with exit 0 any more.
#
# The probe cases live in a private copy of the harness, never in this checkout's test/cases:
# the suite may be running other cases beside this one, and they read that directory.
. "$ROOT/test/lib.sh"

H="$T/harness"; mkdir -p "$H/test/cases" "$H/bin"
cp "$ROOT/test/run.sh" "$H/test/run.sh"; cp "$ROOT/test/lib.sh" "$H/test/lib.sh"
probe() { # name, body
  printf '. "$ROOT/test/lib.sh"\n%s\n' "$2" > "$H/test/cases/$1.sh"
}
runner() { # expected exit, runner arguments... -> the output in LAST_OUT
  expect_exit "$@"
}

# ---------------------------------------------------------------- 1. serial, without the flag
probe zz_skips 'command -v zz-no-such-tool-anywhere >/dev/null 2>&1 || skip_case "zz-no-such-tool-anywhere absent"
echo "    an assertion that must never run"; exit 1'
runner 0 bash "$H/test/run.sh" zz_skips
expect_grep '^SKIP zz_skips$'
expect_no_grep '^ok +zz_skips'
expect_grep 'SKIP: zz-no-such-tool-anywhere absent'
expect_no_grep 'an assertion that must never run'
expect_grep '^tests: 0 passed, 0 failed, 1 skipped$'
expect_grep '^skipped: zz_skips$'
expect_grep 'did not run and proved nothing'

# ---------------------------------------------------------------- 2. serial, --no-skips
runner 1 bash "$H/test/run.sh" --no-skips zz_skips
expect_grep '^FAIL zz_skips \(skipped under --no-skips: a skipped case is not evidence\)$'
expect_no_grep '^SKIP zz_skips$'
expect_grep '^tests: 0 passed, 1 failed, 1 skipped$'
expect_grep '^failed: zz_skips$'
# the flag after the name means the same thing
runner 1 bash "$H/test/run.sh" zz_skips --no-skips
expect_grep '^FAIL zz_skips \(skipped under --no-skips'

# ---------------------------------------------------------------- 3. the parallel runner
probe zz_passes 'true'
runner 0 env MJ_TEST_JOBS=2 MJ_TEST_REPORT="$T/report.tsv" bash "$H/test/run.sh"
expect_grep '^SKIP zz_skips$'
expect_grep '^ok +zz_passes$'
expect_grep '^tests: 1 passed, 0 failed, 1 skipped$'
grep -qE "^zz_skips	SKIP	" "$T/report.tsv" || { echo "    the report does not record the skip as SKIP:"; cat "$T/report.tsv"; exit 1; }
runner 1 env MJ_TEST_JOBS=2 bash "$H/test/run.sh" --no-skips
expect_grep '^FAIL zz_skips \(skipped under --no-skips'
expect_grep '^failed: zz_skips$'

# ---------------------------------------------------------------- 4. not vacuous
# --no-skips does not fail a run in which nothing skipped
rm -f "$H/test/cases/zz_skips.sh"
runner 0 bash "$H/test/run.sh" --no-skips
expect_grep '^tests: 1 passed, 0 failed, 0 skipped$'
runner 0 env MJ_TEST_JOBS=2 bash "$H/test/run.sh" --no-skips
# only 77 is a skip: a case that dies with another status is a failure, whatever the number
probe zz_dies 'exit 3'
runner 1 bash "$H/test/run.sh" zz_dies
expect_grep '^FAIL zz_dies$'
expect_no_grep '^(SKIP|TIMEOUT) zz_dies'
rm -f "$H/test/cases/zz_dies.sh"
# a usage error is not a flag silently ignored
runner 2 bash "$H/test/run.sh" --no-skip
expect_grep "unknown option '--no-skip'"

# ---------------------------------------------------------------- 5. the Rust cases' skip
# rust_bin returns 3 without cargo and without MAJORDOMUS_BIN; rust_bin_exit turns that into
# the skip status, and every other failure of rust_bin into a failure
rc=0; ( rust_bin_exit 3 ) >/dev/null || rc=$?
[ "$rc" = 77 ] || { echo "    rust_bin_exit 3 exited $rc, not the skip status 77"; exit 1; }
rc=0; ( rust_bin_exit 1 ) >/dev/null || rc=$?
[ "$rc" = 1 ] || { echo "    rust_bin_exit 1 exited $rc, not a failure"; exit 1; }

# ---------------------------------------------------------------- 6. no case skips with exit 0
# The shape that hid the evidence: a message about skipping or an absent tool, then exit 0.
# The four named below are held by fix/the-trunk-is-green-again, which rewrites them; each
# migrates to skip_case when that lands, and leaves this list.
legacy=' 12_site_build.sh 95_executable_reference.sh 130_context_compiler.sh 192_entry_across_checkouts_and_clients.sh '
found=""
for f in "$ROOT"/test/cases/*.sh; do
  n="${f##*/}"; case "$legacy" in *" $n "*) continue ;; esac
  if grep -nEi 'echo "[^"]*(skip|absent|not installed)[^"]*"; *exit 0' "$f" >/dev/null; then found="$found $n"; fi
done
[ -z "$found" ] || { echo "    these cases still skip with exit 0, which the runner counts as a pass:$found"; exit 1; }
