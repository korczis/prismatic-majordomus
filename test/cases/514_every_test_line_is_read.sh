# majordomus-covers: none
# Every test line is read (I1957, ADR 0087 decision 2). The recorder reads each runner through
# one adapter, and every line the runner printed is either read into a case result or listed
# as dropped, with its number, its text and why. The web report's test page reads the same
# adapters. This case drives both through the Rust executable in a fixture repository of its
# own, over real output of `cargo test` and of `test/run.sh` kept under
# test/fixtures/evidence-adapters/.
#
# What it proves, in order:
#   1  a recorded cargo run counts its output's lines in the run record, and the lines it read
#      and the lines it listed add up to the output; each listed line is the output's line at
#      its number, with a reason
#   2  each binary's execution carries its tests as cases, a failing test's captured output as
#      that case's detail, and the result line of a binary that did not pass as its detail;
#      the unit tests and the doctests, which no claim names, are listed with their cases
#   3  the ledger keeps one line's worth per test: the cases that did not pass, by name, and
#      no output; a green binary's row is what it always was
#   4  a suite report is read row by row, and a blank row is listed rather than skipped
#   5  the web report's test page reads the same adapters: its cases are the report's rows
#      and its crate totals are the sums of every result line, a coloured one included
#
# The reports and the run records live outside the repository under test, and the recorder
# writes its rows into the local ledger under .ai/local/, which init ignores.
. "$ROOT/test/lib.sh"
command -v jq >/dev/null 2>&1 || skip "no jq: the run record is read with jq"
MJB="$(rust_bin)" || rust_bin_exit $?
export MAJORDOMUS_SHARE="$ROOT/share"

W="$(mktemp -d "${TMPDIR:-/tmp}/mj514.XXXXXX")"; trap 'rm -rf "$W"' EXIT
FX="$ROOT/test/fixtures/evidence-adapters"

ev() {           # ev <name> <args...> — the JSON answer of `evidence` into $W/<name>.json
  local name="$1"; shift
  run_quiet "$W/$name.err" "$MJB" evidence --repo "$T" --format json "$@" > "$W/$name.json"
}
jqe() {          # jqe <file> <filter> <what broke> [jq args...]
  local file="$1" filter="$2" what="$3"; shift 3
  jq -e "$@" "$filter" "$file" >/dev/null 2>&1 || {
    printf '    %s\n' "$what"; jq -c . "$file" | head -c 2000; echo; return 1; }
}
accounted() {    # accounted <run record> <producer> <stream> <file> — every line read or listed
  local run="$1" producer="$2" stream="$3" file="$4" held n text
  held="$(wc -l < "$file" | tr -d ' ')"
  jqe "$run" '[.lines[] | select(.producer == $p and .stream == $s)] as $c
      | ($c | length) == 1 and $c[0].lines == ($n | tonumber)
      and $c[0].read + ([.dropped_lines[] | select(.producer == $p and .stream == $s)] | length)
        == ($n | tonumber)' \
    "the $producer $stream's $held lines are not each read or listed" \
    --arg p "$producer" --arg s "$stream" --arg n "$held"
  jq -r --arg p "$producer" --arg s "$stream" \
    '.dropped_lines[] | select(.producer == $p and .stream == $s) | "\(.line)\t\(.reason)"' "$run" \
    > "$W/listed.tsv"
  while IFS="$(printf '\t')" read -r n reason; do
    [ -n "$reason" ] || { echo "    line $n of the $producer $stream is listed without a reason"; exit 1; }
    text="$(jq -r --argjson n "$n" --arg p "$producer" --arg s "$stream" \
      '.dropped_lines[] | select(.producer == $p and .stream == $s and .line == $n) | .text' "$run")"
    [ "$(sed -n "${n}p" "$file")" = "$text" ] \
      || { echo "    line $n of the $producer $stream is listed as '$text'"; exit 1; }
  done < "$W/listed.tsv"
}

# ---------------------------------------------------------------- the fixture
# The binaries the cargo fixture ran and the cases the suite fixture ran, so none is unknown.
"$MJ" init >/dev/null
mkdir -p apps/majordomus-cli/tests test/cases
for b in empty green red; do printf '// %s\n' "$b" > "apps/majordomus-cli/tests/$b.rs"; done
for c in 01_passes 02_fails 03_declines 04_exclusive 05_hangs; do
  printf '# majordomus-%s: %s\ntrue\n' covers none > "test/cases/$c.sh"
done
git add -A >/dev/null && git -c core.hooksPath=/dev/null commit -qm fixture

# ---------------------------------------------------------------- 1. every line read or listed
ev crate record --crate-output "$FX/cargo-test.txt" --ledger local --run-record "$W/crate-run.json"
jqe "$W/crate-run.json" '. == $answer[0].run_record' "the run record written is not the one answered" \
  --slurpfile answer "$W/crate.json"
accounted "$W/crate-run.json" crate output "$FX/cargo-test.txt"
jqe "$W/crate-run.json" 'any(.dropped_lines[]; .text | test("to rerun pass `--test red`"))' \
  "cargo's own summary line is neither read nor listed"
jqe "$W/crate-run.json" '[.dropped_lines[].text | select(test("^test "))] == []' \
  "a test line was listed rather than read"

# ---------------------------------------------------------------- 2. cases and detail
jqe "$W/crate-run.json" '[.executions[] | select(.test == "crate:red") | .cases[] | [.name, .outcome]]
    == [["ignored_plain","skip"],["passes","pass"],["fails_too","fail"],["fails_with_output","fail"]]' \
  "the failing binary's tests are not its cases"
jqe "$W/crate-run.json" '.executions[] | select(.test == "crate:red")
    | (.detail | startswith("test result: FAILED."))
      and (.cases[3].detail | contains("arithmetic is broken"))
      and (.cases[2].detail | contains("second failure"))' \
  "a failure's reason did not reach its case, or the binary's result line its detail"
jqe "$W/crate-run.json" '.executions[] | select(.test == "crate:green") | has("detail") | not' \
  "a green binary carries a detail"
jqe "$W/crate-run.json" '[.dropped[] | [.what, (.cases | length)]]
    == [["unittests src/lib.rs", 2], ["doc-tests fixture", 3]]' \
  "the unit tests and the doctests are not listed with their cases"

# ---------------------------------------------------------------- 3. the ledger keeps a line's worth
L=.ai/local/evidence/ledger.json
jqe "$L" '.executions[] | select(.test == "crate:red")
    | ([.cases[].name] == ["ignored_plain","fails_too","fails_with_output"])
      and (has("detail") | not) and ([.cases[] | has("detail")] | any | not)' \
  "the ledger keeps a passing case or a case's output"
jqe "$L" '.executions[] | select(.test == "crate:green") | has("cases") | not' \
  "a green row is not what it always was"

# ---------------------------------------------------------------- 4. a suite report, row by row
printf '01_passes\tok\t1\tparallel\n\n02_fails\tFAIL\t2\texclusive\n' > "$W/suite.tsv"
ev suite record --suite "$W/suite.tsv" --ledger local --run-record "$W/suite-run.json"
accounted "$W/suite-run.json" suite report "$W/suite.tsv"
jqe "$W/suite-run.json" '[.dropped_lines[] | .line] == [2]' "the blank row is not listed"
jqe "$W/suite-run.json" '[.executions[] | .cases[0] | [.name, .phase]]
    == [["01_passes","parallel"],["02_fails","exclusive"]]' \
  "a suite case is not its own case, with its phase"
printf '01_passes\tok\t12\n' > "$W/short.tsv"
expect_exit 13 "$MJB" evidence --repo "$T" record --suite "$W/short.tsv" --ledger local
expect_grep 'field\(s\)'

# ---------------------------------------------------------------- 5. the web report reads the same adapters
expect_exit 0 "$MJB" web --repo "$T" report tests --suite "$FX/suite-serial.tsv" \
  --crate-output "$FX/cargo-test.txt"
R=target/web/tests/results.json
expect_file "$R"
jqe "$R" '.crate_tests == {passed: 7, failed: 2, ignored: 4}' \
  "the page's crate totals are not the sums of the result lines the adapter read"
jqe "$R" '[.cases[] | [.name, .result, .phase]] == [["01_passes","ok","serial"],
    ["02_fails","FAIL","serial"],["03_declines","SKIP","serial"],["04_exclusive","ok","serial"]]' \
  "the page's cases are not the report's rows"
expect_exit 0 "$MJB" web --repo "$T" report tests --suite "$FX/suite-serial.tsv" \
  --crate-output "$FX/cargo-test-color.txt"
jqe "$R" '.crate_tests == {passed: 3, failed: 0, ignored: 0}' \
  "a coloured result line is not summed as the plain one"
