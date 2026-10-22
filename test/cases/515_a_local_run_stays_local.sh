# majordomus-covers: none
# A local run stays local (I1957, ADR 0087 decision 2). `evidence record --run suite` runs the
# repository's own suite runner, stamps the checkout the run left, reads everything the runner
# wrote and printed through the suite adapter, and records the run into the local ledger and
# the local run index under .ai/local/, which is bounded. It never reaches the tracked ledger.
# This case drives it through the Rust executable in a fixture repository of its own, whose
# runner is this repository's test/run.sh.
#
# What it proves, in order:
#   1  a run of the fixture's suite is recorded into the local ledger and kept in the local
#      run index, stamped by the run itself: the suite's measurement, of a clean tree, naming
#      its report
#   2  every line the runner wrote and printed is read or listed: each case's log is its
#      execution's detail, a skip's reason among them, and only the runner's own totals are
#      listed; each case carries its phase
#   3  nothing tracked changed and the tracked ledger was never written; the index is what
#      `evidence coverage` reads
#   4  the cases named with --case are the ones run, and the text rendering names the local
#      run and how much of the output was read, while the runner's own output streams aside
#   5  a local run pointed at the tracked ledger, given a report or a measurement, given
#      another origin, or a case named without --run suite is refused and writes nothing
#   6  the index keeps the newest 50 runs and drops the oldest
. "$ROOT/test/lib.sh"
command -v jq >/dev/null 2>&1 || skip "no jq: the run record is read with jq"
MJB="$(rust_bin)" || rust_bin_exit $?
export MAJORDOMUS_SHARE="$ROOT/share"
unset MJ_TEST_JOBS

W="$(mktemp -d "${TMPDIR:-/tmp}/mj515.XXXXXX")"; trap 'rm -rf "$W"' EXIT

ev() {           # ev <name> <args...> — the JSON answer of `evidence` into $W/<name>.json
  local name="$1"; shift
  run_quiet "$W/$name.err" "$MJB" evidence --repo "$T" --format json "$@" > "$W/$name.json"
}
jqe() {          # jqe <name> <filter> <what broke> [jq args...]
  local name="$1" filter="$2" what="$3"; shift 3
  jq -e "$@" "$filter" "$W/$name.json" >/dev/null 2>&1 || {
    printf '    %s\n' "$what"; jq -c . "$W/$name.json" | head -c 2000; echo; return 1; }
}
RUNS=.ai/local/evidence/runs
LEDGER=.ai/repo/evidence/ledger.json
runs() { find "$RUNS" -maxdepth 1 -name 'run-*.json' 2>/dev/null | wc -l | tr -d ' '; }

# ---------------------------------------------------------------- the fixture
# This repository's own runner, and three cases of the fixture's: one passes and prints, one
# fails and says why, one declines and says why.
"$MJ" init >/dev/null
git check-ignore -q "$RUNS/x.json" \
  || { echo "    init does not ignore .ai/local/, which the local run index relies on"; exit 1; }
mkdir -p test/cases
cp "$ROOT/test/run.sh" test/run.sh
printf 'echo "    a line it printed"\n' > test/cases/01_passes.sh
printf 'echo "    expected /proven/ in output:"\necho "not run"\nexit 1\n' > test/cases/02_fails.sh
printf 'printf "no zola\\n" > "$MJ_SKIP_MARK"\necho "    skip: no zola"\nexit 4\n' \
  > test/cases/03_declines.sh
git add -A >/dev/null && git -c core.hooksPath=/dev/null commit -qm fixture
before="$(git status --porcelain --untracked-files=all)"

# ---------------------------------------------------------------- 1. a run, stamped by itself
ev run record --run suite
jqe run '.recorded == 3 and .passed == 1 and .ledger == ".ai/local/evidence/ledger.json"' \
  "the run was not recorded into the local ledger"
jqe run '.local_run | startswith(".ai/local/evidence/runs/run-local-")' \
  "the run is not kept in the local run index"
jqe run '.run_record | .ledger == "local" and .origin == "local" and .working_tree == "clean"
    and (.provenance | length) == 1 and .provenance[0].producer == "suite"
    and .provenance[0].working_tree == "clean" and .provenance[0].report.path == "suite.tsv"' \
  "the run did not stamp itself: the suite's measurement of a clean tree, naming its report"

# ---------------------------------------------------------------- 2. every line read or listed
jqe run '.run_record | [.lines[] | [.producer, .stream]] == [["suite","report"],["suite","output"]]' \
  "the report and the output are not both counted"
jqe run '.run_record as $r | all($r.lines[]; . as $c
    | $c.read + ([$r.dropped_lines[] | select(.stream == $c.stream)] | length) == $c.lines)' \
  "a line the runner wrote or printed is neither read nor listed"
jqe run '.run_record.lines[0] | .lines == 3 and .read == 3' "a report row is not read"
jqe run '[.run_record.dropped_lines[].reason] | all(test("^the runner.s (totals|list)"))' \
  "something other than the runner's own totals is listed"
jqe run '.run_record.executions | map({key: .test, value: .detail}) | from_entries
    | .["suite:01_passes"] == "    a line it printed"
      and .["suite:02_fails"] == "    expected /proven/ in output:\nnot run"
      and .["suite:03_declines"] == "    skip: no zola"' \
  "a case's log is not its execution's detail"
jqe run '[.run_record.executions[] | [.outcome, .cases[0].phase]]
    == [["pass","serial"],["fail","serial"],["skip","serial"]]' \
  "a case does not carry its outcome and its phase"

# ---------------------------------------------------------------- 3. only the ignored layer moved
[ ! -e "$LEDGER" ] || { echo "    a local run wrote the tracked ledger"; exit 1; }
[ "$(git status --porcelain --untracked-files=all)" = "$before" ] \
  || { echo "    a local run changed the checkout"; git status --porcelain; exit 1; }
[ "$(runs)" = 1 ] || { echo "    the index holds $(runs) runs, not 1"; exit 1; }
local_run="$(jq -r .local_run "$W/run.json")"
[ -f "$local_run" ] || { echo "    the index has no $local_run"; exit 1; }
jq -e --slurpfile kept "$local_run" '.run_record == $kept[0]' "$W/run.json" >/dev/null \
  || { echo "    the index keeps another record than the one answered"; exit 1; }
ev cov coverage
jqe cov '.source == ".ai/local/evidence/runs" and .runs == 1' \
  "the local run index is not what evidence coverage reads"

# ---------------------------------------------------------------- 4. named cases, and the text
ev one record --run suite --case 01_passes
jqe one '.recorded == 1 and .passed == 1' "--case did not run only the case named"
expect_exit 0 "$MJB" evidence --repo "$T" record --run suite --case 02_fails
expect_grep '^local run    \.ai/local/evidence/runs/run-local-'
expect_grep '^lines        suite report \(1 of 1 read\), suite output \('
expect_grep '^not read     1 line\(s\): the runner.s totals'
expect_grep '^FAIL 02_fails'
[ "$(runs)" = 3 ] || { echo "    the index holds $(runs) runs, not 3"; exit 1; }

# ---------------------------------------------------------------- 5. refused, and nothing written
expect_exit 13 "$MJB" evidence --repo "$T" record --run suite --ledger repo
expect_grep 'never reaches the tracked ledger'
expect_exit 13 "$MJB" evidence --repo "$T" record --run suite --suite "$W/x.tsv"
expect_grep 'reads no report'
expect_exit 13 "$MJB" evidence --repo "$T" record --run suite --provenance "$W/p.json"
expect_grep 'reads no report'
expect_exit 13 "$MJB" evidence --repo "$T" record --run suite --origin ci
expect_grep 'is a local run'
expect_exit 13 "$MJB" evidence --repo "$T" record --case 01_passes
expect_grep 'give --run suite'
expect_exit 13 "$MJB" evidence --repo "$T" record --run suite --case 99_none
expect_grep 'exited with status 2'
expect_exit 2 "$MJB" evidence --repo "$T" record --run crate
[ ! -e "$LEDGER" ] || { echo "    a refused run wrote the tracked ledger"; exit 1; }
[ "$(runs)" = 3 ] || { echo "    a refused run changed the index: $(runs) runs"; exit 1; }

# ---------------------------------------------------------------- 6. bounded at the newest 50
# 49 older runs beside the three real ones: 52, of which the newest 50 stay
for n in $(seq 10 58); do
  jq --arg at "2000-01-01T00:00:${n}Z" --arg id "local:old:$n" \
    '.run_record | .recorded_at = $at | .id = $id' "$W/run.json" > "$RUNS/run-old-$n.json"
done
ev last record --run suite --case 01_passes
[ "$(runs)" = 50 ] || { echo "    the index holds $(runs) runs, not the newest 50"; exit 1; }
for gone in 10 11 12; do
  [ ! -e "$RUNS/run-old-$gone.json" ] || { echo "    the oldest run $gone was kept"; exit 1; }
done
[ -e "$RUNS/run-old-13.json" ] || { echo "    a run newer than the three oldest was dropped"; exit 1; }
[ -f "$(jq -r .local_run "$W/last.json")" ] || { echo "    the newest run was dropped"; exit 1; }
[ "$(git status --porcelain --untracked-files=all)" = "$before" ] \
  || { echo "    the index left the ignored layer"; git status --porcelain; exit 1; }
