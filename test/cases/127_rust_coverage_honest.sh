# majordomus-covers: none
#
# The gate that measures coverage over the code the crate ships, exercised against fixture
# coverage exports rather than against a compiled crate.
#
# The subject is a JSON document, not Rust: the check reads llvm-cov's export and classifies
# each function record, so a fixture is a handful of files and the case runs in under a
# second. Building a real instrumented crate here would take twenty minutes and would prove
# less, because it could not produce the shapes that matter — a test module under an
# unexpected name, a file that improved, a baseline that is out of date.
#
# What is proved here: an in-file `tests::` function is not counted; a function under
# `tests/` is not counted; a function of another crate is not counted; a production function
# is counted and its uncovered lines are reported; the ratchet fails a rise above the
# accepted count and names the file; it passes and reports a fall; `--strict` ignores the
# baseline in both directions; and a missing baseline is unusable rather than clean.
. "$ROOT/test/lib.sh"

CHECK="$ROOT/scripts/ci/rust-coverage-check"
[ -x "$CHECK" ] || { echo "    scripts/ci/rust-coverage-check is missing or not executable"; exit 1; }
command -v python3 >/dev/null 2>&1 || { echo "    skip: no python3"; exit 0; }

F="$PWD/fixture"
mkdir -p "$F/apps/majordomus-cli/src/web" "$F/.ai/repo"
printf '[package]\nname = "majordomus-cli"\n' > "$F/apps/majordomus-cli/Cargo.toml"
BASE="$F/.ai/repo/rust-coverage-baseline.txt"

# The fixture's cfg(test) geography. Test code is classified out by DECLARATION, not by
# name — llvm-cov's export carries mangled symbols, and this crate has a production module
# called `tests` — so the check reads the source for `#[cfg(test)]` spans and the fixture
# has to carry a real one at a known line. The export below declares its unit test at line
# 90, inside the span this file opens at 89; its two production functions are at 10 and 20,
# outside it.
{
  awk 'BEGIN { for (i = 1; i <= 9; i++) print "// filler" }'
  printf 'pub fn routes() -> u8 {\n    7\n}\n'                     # lines 10-12
  awk 'BEGIN { for (i = 13; i <= 19; i++) print "// filler" }'
  printf 'pub fn never_called() -> u8 {\n    0\n}\n'               # lines 20-22
  awk 'BEGIN { for (i = 23; i <= 88; i++) print "// filler" }'
  printf '#[cfg(test)]\nmod tests {\n'                             # lines 89-90
  awk 'BEGIN { for (i = 91; i <= 98; i++) print "    // a test" }'
  printf '}\n'                                                     # line 99
} > "$F/apps/majordomus-cli/src/web/mod.rs"

# ---- a fixture export.
# One production function that ran, one that did not, plus three records that must all be
# classified out: an in-file test module, an integration test target, and another crate.
# $1 is how many lines the uncovered production function spans, which is the number the
# ratchet compares.
export_json() {
  end=$((20 + $1 - 1))
  cat > "$2" <<JSON
{"data":[{"functions":[
 {"name":"majordomus_cli::web::routes","count":7,
  "filenames":["/x/apps/majordomus-cli/src/web/mod.rs"],
  "regions":[[10,1,12,2,7,0,0,0]]},
 {"name":"majordomus_cli::web::never_called","count":0,
  "filenames":["/x/apps/majordomus-cli/src/web/mod.rs"],
  "regions":[[20,1,$end,2,0,0,0,0]]},
 {"name":"majordomus_cli::web::tests::a_unit_test","count":3,
  "filenames":["/x/apps/majordomus-cli/src/web/mod.rs"],
  "regions":[[90,1,99,2,3,0,0,0]]},
 {"name":"web::an_integration_test","count":1,
  "filenames":["/x/apps/majordomus-cli/tests/web.rs"],
  "regions":[[1,1,50,2,1,0,0,0]]},
 {"name":"serde_json::from_str","count":9,
  "filenames":["/registry/serde_json-1.0/src/de.rs"],
  "regions":[[1,1,9,2,9,0,0,0]]}
],"totals":{}}]}
JSON
}

# a fully covered export: the same production functions, both reached
covered_json() {
  cat > "$1" <<'JSON'
{"data":[{"functions":[
 {"name":"majordomus_cli::web::routes","count":7,
  "filenames":["/x/apps/majordomus-cli/src/web/mod.rs"],
  "regions":[[10,1,12,2,7,0,0,0]]},
 {"name":"majordomus_cli::web::never_called","count":4,
  "filenames":["/x/apps/majordomus-cli/src/web/mod.rs"],
  "regions":[[20,1,22,2,4,0,0,0]]},
 {"name":"majordomus_cli::web::tests::a_unit_test","count":3,
  "filenames":["/x/apps/majordomus-cli/src/web/mod.rs"],
  "regions":[[90,1,99,2,3,0,0,0]]}
],"totals":{}}]}
JSON
}

# The check cd's to MJ_ROOT, so the export has to be named absolutely; a relative path
# would resolve against the fixture rather than against the case's own directory.
run() { MJ_ROOT="$F" "$CHECK" --from "$PWD/$1" ${2:+$2} > out.txt 2>&1; echo $?; }

# ---------------------------------------------------------------- classification
export_json 3 three.json
code="$(run three.json --write-baseline)"
[ "$code" = 0 ] || { echo "    --write-baseline exited $code, not 0"; cat out.txt; exit 1; }

# 3 production lines covered (10-12), 3 uncovered (20-22): 50% of six, and the ten lines of
# the in-file test module, the fifty of the integration test and the nine of serde_json are
# in neither half.
# the production denominator is six lines, not sixty-nine
expect_grep '50.0000%' out.txt
# three of six production lines are covered
expect_grep '3/6' out.txt
# the test records are classified out and counted
expect_grep 'excluded 1 in-file test functions and 1 under tests/ or benches/' out.txt
# another crate is not part of this crate.s coverage
expect_no_grep 'serde_json' out.txt

# the baseline records a count against a crate-relative path, not a line number
# the baseline is a count and a path
expect_grep '^3	src/web/mod.rs$' "$BASE"
# a test target is never recorded as debt
expect_no_grep 'tests/web.rs' "$BASE"

# ---------------------------------------------------------------- the ratchet holds
code="$(run three.json)"
[ "$code" = 0 ] || { echo "    unchanged debt exited $code, not 0"; cat out.txt; exit 1; }
# unchanged debt passes
expect_grep 'no file is less covered than the baseline accepts' out.txt

# ---------------------------------------------------------------- a rise fails
export_json 5 five.json
code="$(run five.json)"
[ "$code" = 10 ] || { echo "    new debt exited $code, not 10"; cat out.txt; exit 1; }
# the finding names the file and both counts
expect_grep 'src/web/mod.rs: 3 accepted, 5 now' out.txt
# the finding says how to record debt that was already there
expect_grep 'write-baseline' out.txt

# ---------------------------------------------------------------- a fall is reported, not failed
export_json 1 one.json
code="$(run one.json)"
[ "$code" = 0 ] || { echo "    an improvement exited $code, not 0"; cat out.txt; exit 1; }
# an improvement is reported
expect_grep 'improved since the baseline' out.txt
# the improvement names both counts
expect_grep '3 -> 1 uncovered' out.txt

# ---------------------------------------------------------------- --strict ignores the baseline
code="$(run three.json --strict)"
[ "$code" = 10 ] || { echo "    --strict over accepted debt exited $code, not 10"; cat out.txt; exit 1; }
# strict counts every uncovered line
expect_grep '3 uncovered production line' out.txt

covered_json clean.json
code="$(run clean.json --strict)"
[ "$code" = 0 ] || { echo "    --strict over a covered crate exited $code, not 0"; cat out.txt; exit 1; }
# strict passes only when nothing is uncovered
expect_grep 'every production line' out.txt
# a covered crate measures 100%
expect_grep '100.0000%' out.txt

# ---------------------------------------------------------------- unusable is not clean
rm -f "$BASE"
code="$(run three.json)"
[ "$code" = 12 ] || { echo "    a missing baseline exited $code, not 12"; cat out.txt; exit 1; }
# a missing baseline is unusable and says so
expect_grep 'nothing to compare against' out.txt

# a document that is not a coverage export is unusable, not empty
printf '{"data":[]}\n' > empty.json
code="$(run empty.json --strict)"
[ "$code" = 12 ] || { echo "    an export with no data exited $code, not 12"; cat out.txt; exit 1; }
