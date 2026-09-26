# majordomus-covers: none
# claims: evidence-recorded-in-ci
# Proves claim evidence-recorded-in-ci (docs/CLAIMS.yaml), whose `test:` names this case.
#
# CI's evidence is kept where the run happened and published with the commit it proves (ADR 68,
# as ADR 0087 amends it). This case drives each half through the real executable and the real
# scripts, in fixtures of its own, because the behaviours that matter are about which run,
# which commit and which tree:
#
#   1  a `ci` recording inside a GitHub Actions environment names the run: id, attempt, job and
#      the address the provider gave, exactly
#   2  the same recording as `local`, or as `ci` outside a run, names no run, and the ledger row
#      carries no `run` member at all
#   3  scripts/ci/evidence-collect gathers a run: the manifest names the commit and the run,
#      counts the outcomes of that run only, keeps a failure as a failure, names what was
#      absent, and summarises coverage through scripts/rust-coverage. Beyond that:
#      a  a crate report that yields no test binary is named as absent, never read as nothing
#         to say
#      b  the head a pull request was built from is carried beside the commit that ran, and
#         never replaces it
#      c  the working tree is the one the producing jobs measured where they ran: clean only
#         when every recorded runner measured a clean tree, dirty when any measured a dirty
#         one, unknown when one was not measured; the rows' own stamp is kept apart
#      d  a report whose job measured another commit is refused: it is not recorded, and it is
#         named as absent and refused; when every report is refused, nothing is gathered
#      e  the report is the tracked ledger's: derived before the run's rows are recorded, on a
#         checkout measured first, so a claim only the run proved still reads `not_run` in it
#   4  nothing to gather is refused, not written as an empty success
#   5  scripts/pages evidence publishes `current` only when the run is of the published commit,
#      measured clean trees, derived its report on a clean checkout, and nothing was absent,
#      refused or failed; `unknown` with its reasons when the run cannot confirm the report,
#      `stale` for a failure or an ancestor's run, with its distance; a manifest from outside
#      the history, or none at all, as unavailable with a reason — never as current
#   6  validate.yml is wired for all of it: cargo writes no colour codes, the reports and the
#      gathered directory live outside the checkouts, each producing job measures the tree it
#      ran in excluding only the outputs its run names, and every reader of the suite's report
#      reads the file the suite wrote
. "$ROOT/test/lib.sh"
MJB="$(rust_bin)" || rust_bin_exit $?
export MAJORDOMUS_SHARE="$ROOT/share"

# Reports, answers and gathered directories live outside every repository under test.
W="$(mktemp -d "${TMPDIR:-/tmp}/mj357.XXXXXX")"; trap 'rm -rf "$W"' EXIT

actions() {      # actions <command...> — run inside a described GitHub Actions run
  env GITHUB_ACTIONS=true GITHUB_SERVER_URL=https://forge.example GITHUB_REPOSITORY=owner/repo \
    GITHUB_RUN_ID=77 GITHUB_RUN_ATTEMPT=3 GITHUB_WORKFLOW=validate GITHUB_JOB=evidence \
    GITHUB_EVENT_NAME=push "$@"
}
row() {          # row <test> — that test's ledger row, compact
  jq -c --arg t "$1" '.executions[] | select(.test == $t)' "$T/.ai/repo/evidence/ledger.json"
}

# A repository with a layer of its own: the recorder refuses a directory that is not one.
( cd "$T" && "$MJ" init >/dev/null )
mkdir -p "$T/test/cases"
printf 'echo holds\n' > "$T/test/cases/01_holds.sh"
printf 'echo breaks\n' > "$T/test/cases/02_breaks.sh"
git -C "$T" add -A >/dev/null
git -C "$T" commit -qm fixture
FIXTURE="$(git -C "$T" rev-parse HEAD)"
printf '01_holds\tok\t1\tparallel\n02_breaks\tFAIL\t2\tparallel\n' > "$W/suite.tsv"

# ---------------------------------------------------------------- 1. a ci recording names its run
run_quiet "$W/r1.err" actions "$MJB" evidence --repo "$T" record --suite "$W/suite.tsv" --origin ci > /dev/null
r="$(row suite:01_holds)"
[ "$(printf '%s' "$r" | jq -r '.run.url')" = "https://forge.example/owner/repo/actions/runs/77/attempts/3" ] \
  || { echo "    a ci recording did not name the run it happened in: $r"; exit 1; }
printf '%s' "$r" | jq -e '.run.id == "77" and .run.attempt == 3 and .run.job == "evidence" and .run.workflow == "validate" and .origin == "ci"' >/dev/null \
  || { echo "    the run named is not the run described: $r"; exit 1; }
row suite:02_breaks | jq -e '.outcome == "fail" and .run.id == "77"' >/dev/null \
  || { echo "    a failing case lost its outcome or its run: $(row suite:02_breaks)"; exit 1; }

# ---------------------------------------------------------------- 2. no run where there is none
run_quiet "$W/r2.err" actions "$MJB" evidence --repo "$T" record --suite "$W/suite.tsv" --origin local > /dev/null
row suite:01_holds | jq -e 'has("run") | not' >/dev/null \
  || { echo "    a local recording inside a CI shell claimed that run: $(row suite:01_holds)"; exit 1; }
run_quiet "$W/r3.err" env -u GITHUB_ACTIONS "$MJB" evidence --repo "$T" record --suite "$W/suite.tsv" --origin ci > /dev/null
row suite:01_holds | jq -e '(has("run") | not) and .origin == "ci"' >/dev/null \
  || { echo "    a ci recording outside any run invented one: $(row suite:01_holds)"; exit 1; }

# ---------------------------------------------------------------- 3. a run gathered
# The collector and the coverage script resolve the repository from where they live, so they
# are placed in the fixture: run from the checkout, they would record into this repository.
# The fixture also gains a claims matrix of its own, one guaranteed claim that 01_holds proves,
# and the crate test the cargo report below names.
mkdir -p "$T/scripts/ci" "$T/apps/majordomus-cli/src" "$T/apps/majordomus-cli/tests" "$T/docs"
cp "$ROOT/scripts/ci/evidence-collect" "$T/scripts/ci/evidence-collect"
cp "$ROOT/scripts/rust-coverage" "$T/scripts/rust-coverage"
cp "$ROOT/test/fixtures/coverage/sample.rs" "$T/apps/majordomus-cli/src/sample.rs"
chmod +x "$T/scripts/ci/evidence-collect" "$T/scripts/rust-coverage"
printf '#[test]\nfn holds() {}\n' > "$T/apps/majordomus-cli/tests/holds.rs"
printf '# Holds\n\nThe fixture holds.\n' > "$T/docs/HOLDS.md"
cat > "$T/docs/CLAIMS.yaml" <<'YAML'
version: 1
claims:
  - id: fixture-holds
    claim: The fixture holds
    source: docs/HOLDS.md
    implementation: test/cases/01_holds.sh
    test: test/cases/01_holds.sh
    status: guaranteed
YAML
# The ledger committed with the collector is the one the fixture commit holds. Sections 1 and 2
# recorded 01_holds into the working ledger; committing that would put a run's rows into the
# authority, and the report could no longer tell the two apart.
L=".ai/repo/evidence/ledger.json"
if git -C "$T" cat-file -e "$FIXTURE:$L" 2>/dev/null; then git -C "$T" show "$FIXTURE:$L" > "$T/$L"; else rm -f "$T/$L"; fi
git -C "$T" add -A >/dev/null
git -C "$T" commit -qm "the collector"
HEAD_T="$(git -C "$T" rev-parse HEAD)"
T="$T" python3 - "$ROOT/test/fixtures/coverage/export.json" "$W/export.json" <<'PY'
import json, os, sys
data = json.load(open(sys.argv[1]))
target = os.path.join(os.environ["T"], "apps/majordomus-cli/src/sample.rs")
for f in data["data"][0].get("files", []):
    if f["filename"] == "SAMPLE_ABS":
        f["filename"] = target
for fn in data["data"][0].get("functions", []):
    fn["filenames"] = [target if n == "SAMPLE_ABS" else n for n in fn.get("filenames", [])]
json.dump(data, open(sys.argv[2], "w"))
PY

fresh() {        # fresh — the fixture's working ledger back to its commit's, so a gather starts clean
  if git -C "$T" cat-file -e "HEAD:$L" 2>/dev/null; then git -C "$T" show "HEAD:$L" > "$T/$L"; else rm -f "$T/$L"; fi
  [ -z "$(git -C "$T" status --porcelain --untracked-files=all)" ] \
    || { echo "    the fixture is not clean before a gather:"; git -C "$T" status --porcelain --untracked-files=all; exit 1; }
}
gather() {       # gather <name> <collector options...> — gathered into $W/<name>, its summary in $W/<name>.out
  local name="$1"; shift
  fresh
  run_quiet "$W/$name.err" actions env -u MJ_RUN_HEAD_SHA MAJORDOMUS_BIN="$MJB" \
    "$T/scripts/ci/evidence-collect" --out "$W/$name" "$@" > "$W/$name.out"
}
tree() {         # tree <file> <commit> <working tree> — a producing job's measurement of its tree
  jq -n --arg c "$2" --arg wt "$3" '{commit: $c, working_tree: $wt, excluded: []}' > "$1"
}
state_of() {     # state_of <evidence show json> — the fixture claim's state in it
  jq -r '.claims[] | select(.id == "fixture-holds") | .state' "$1"
}
printf '     Running tests/holds.rs (target/debug/deps/holds-1a2b3c)\n\nrunning 1 test\ntest holds ... ok\n\ntest result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.01s\n\n' > "$W/cargo-test.txt"

gather ev --suite "$W/suite.tsv" --crate-output "$W/no-such-output.txt" --coverage "$W/export.json"
M="$W/ev/manifest.json"
jq -e --arg c "$HEAD_T" '.commit == $c and .run.id == "77"' "$M" >/dev/null \
  || { echo "    the manifest does not name the commit and the run gathered"; jq -c . "$M"; exit 1; }
jq -e '.totals.executions == 2 and .totals.outcomes.pass == 1 and .totals.outcomes.fail == 1' "$M" >/dev/null \
  || { echo "    the manifest does not count this run's outcomes, failure included"; jq -c .totals "$M"; exit 1; }
jq -e '.absent == ["crate"] and .refused == []' "$M" >/dev/null \
  || { echo "    what the run did not produce is not named as absent, or something was refused"; jq -c '{absent, refused}' "$M"; exit 1; }
jq -e '[.panels[].label] == ["evidence", "coverage"] and (.panels | all(.exit | type == "number"))' "$M" >/dev/null \
  || { echo "    the manifest does not describe both recorded commands"; jq -c .panels "$M"; exit 1; }
jq -e '.crate.lines.total > 0 and .crate.lines.covered < .crate.lines.total and (.files | has("apps/majordomus-cli/src/sample.rs"))' "$W/ev/coverage.json" >/dev/null \
  || { echo "    the coverage summary does not carry the measured sample, uncovered line included"; cat "$W/ev/coverage.json"; exit 1; }
[ -s "$W/ev/report.txt" ] && [ -s "$W/ev/ledger.json" ] || { echo "    the report or the ledger was not kept beside the manifest"; exit 1; }
# b: no head named, so the head is the commit that ran
jq -e '.head_sha == .commit and .event == "push"' "$M" >/dev/null \
  || { echo "    with no pull request head named, the head is not the commit that ran, or the event is lost"; jq -c '{commit, head_sha, event}' "$M"; exit 1; }
# c: the suite was recorded and nothing measured its tree, so the tree is unknown, whatever the
#    recorder stamped into the rows
rows_wt="$(row suite:01_holds | jq -r .working_tree)"
jq -e --arg w "$rows_wt" '.working_tree == "unknown" and .producers.suite == null and .rows_working_tree == $w' "$M" >/dev/null \
  || { echo "    an unmeasured tree was not published as unknown, or the rows' own stamp ($rows_wt) was not kept apart"; jq -c '{working_tree, rows_working_tree, producers}' "$M"; exit 1; }
# e: the report is the tracked ledger's. The committed ledger has no row for 01_holds, so the
#    claim reads not_run in the gathered report, while the working ledger, which now holds the
#    run's rows, answers otherwise
[ "$(state_of "$W/ev/report.json")" = not_run ] \
  || { echo "    the gathered report is not the tracked ledger's: the fixture claim reads '$(state_of "$W/ev/report.json")', not not_run"; exit 1; }
run_quiet "$W/after.err" "$MJB" evidence --repo "$T" show --format json > "$W/after.json"
after="$(state_of "$W/after.json")"
[ -n "$after" ] && [ "$after" != not_run ] \
  || { echo "    the run's rows did not reach the working ledger, so the report proves nothing about the order: '$after'"; exit 1; }
jq -e '.report_tree == "clean"' "$M" >/dev/null \
  || { echo "    the report was not derived on a checkout measured clean"; jq -c '{report_tree}' "$M"; exit 1; }

# a: a crate report with no test binary in it is absent, not an empty success
printf 'error: could not compile `majordomus-cli` (lib) due to 1 previous error\n' > "$W/cargo-broken.txt"
gather broken --suite "$W/suite.tsv" --crate-output "$W/cargo-broken.txt"
jq -e '.absent | any(.[]; . == "crate")' "$W/broken/manifest.json" >/dev/null \
  || { echo "    a crate report that yielded no test binary was not named as absent"; jq -c '{absent, totals}' "$W/broken/manifest.json"; exit 1; }
grep -q 'absent: .*crate' "$W/broken.out" \
  || { echo "    the summary line does not name the crate as absent:"; cat "$W/broken.out"; exit 1; }

# b: a pull request's head is carried beside the commit that ran, never in its place
PR_HEAD="0123456789abcdef0123456789abcdef01234567"
fresh
run_quiet "$W/head.err" actions env MJ_RUN_HEAD_SHA="$PR_HEAD" MAJORDOMUS_BIN="$MJB" \
  "$T/scripts/ci/evidence-collect" --out "$W/head" --suite "$W/suite.tsv" > /dev/null
jq -e --arg h "$PR_HEAD" --arg c "$HEAD_T" '.head_sha == $h and .commit == $c' "$W/head/manifest.json" >/dev/null \
  || { echo "    the pull request head is not carried beside the commit that ran"; jq -c '{commit, head_sha}' "$W/head/manifest.json"; exit 1; }

# c: the tree is the one the producing jobs measured
tree "$W/suite-clean.json" "$HEAD_T" clean
tree "$W/suite-dirty.json" "$HEAD_T" dirty
tree "$W/crate-clean.json" "$HEAD_T" clean
gather clean --suite "$W/suite.tsv" --suite-tree "$W/suite-clean.json" --crate-output "$W/cargo-test.txt" --crate-tree "$W/crate-clean.json"
jq -e '.working_tree == "clean" and .producers.suite.working_tree == "clean" and .producers.crate.working_tree == "clean" and .totals.runners.crate == 1 and .refused == []' "$W/clean/manifest.json" >/dev/null \
  || { echo "    two clean measurements did not give a clean tree, or the crate recorded nothing"; jq -c '{working_tree, producers, totals, refused}' "$W/clean/manifest.json"; exit 1; }
# the recorder's own checkout is clean and so are the rows it stamps: only the suite's
# measurement says dirty, and a script that copied the rows' tree would say clean
gather dirty --suite "$W/suite.tsv" --suite-tree "$W/suite-dirty.json" --crate-output "$W/cargo-test.txt" --crate-tree "$W/crate-clean.json"
jq -e '.working_tree == "dirty" and .rows_working_tree == "clean" and .report_tree == "clean"' "$W/dirty/manifest.json" >/dev/null \
  || { echo "    a producer's dirty tree was not published as dirty over a clean recorder"; jq -c '{working_tree, rows_working_tree, report_tree}' "$W/dirty/manifest.json"; exit 1; }

# d: a report whose job measured another commit is not recorded against this one
tree "$W/suite-foreign.json" "$FIXTURE" clean
gather refused --suite "$W/suite.tsv" --suite-tree "$W/suite-foreign.json" --crate-output "$W/cargo-test.txt" --crate-tree "$W/crate-clean.json"
jq -e '(.absent | any(.[]; . == "suite")) and (.refused | length == 1 and .[0].runner == "suite" and (.[0].reason | length > 0))
       and (.totals.runners | has("suite") | not) and .totals.runners.crate == 1' "$W/refused/manifest.json" >/dev/null \
  || { echo "    a report measured at another commit was not refused and named"; jq -c '{absent, refused, totals}' "$W/refused/manifest.json"; exit 1; }
jq -e --arg c "$HEAD_T" '[.executions[] | select(.runner == "suite" and .commit == $c)] | length == 0' "$T/$L" >/dev/null \
  || { echo "    the refused suite report was recorded against the commit that ran anyway"; exit 1; }
grep -q 'refused: suite' "$W/refused.out" \
  || { echo "    the summary line does not name the refusal:"; cat "$W/refused.out"; exit 1; }
fresh
expect_exit 12 actions env MAJORDOMUS_BIN="$MJB" "$T/scripts/ci/evidence-collect" --out "$W/all-refused" \
  --suite "$W/suite.tsv" --suite-tree "$W/suite-foreign.json"
expect_grep "every report was refused: suite"
[ ! -f "$T/$L" ] || jq -e '[.executions[] | select(.runner == "suite")] | length == 0' "$T/$L" >/dev/null \
  || { echo "    a run whose every report was refused recorded one anyway"; exit 1; }

# e: the checkout the report is derived in is measured, and a file left in it makes it dirty
fresh
printf 'left behind\n' > "$T/left-behind.txt"
run_quiet "$W/untracked.err" actions env -u MJ_RUN_HEAD_SHA MAJORDOMUS_BIN="$MJB" \
  "$T/scripts/ci/evidence-collect" --out "$W/untracked" --suite "$W/suite.tsv" > /dev/null
rm -f "$T/left-behind.txt"
jq -e '.report_tree == "dirty"' "$W/untracked/manifest.json" >/dev/null \
  || { echo "    a report derived in a checkout holding an untracked file was not marked dirty"; jq -c '{report_tree}' "$W/untracked/manifest.json"; exit 1; }

# ---------------------------------------------------------------- 4. nothing to gather
expect_exit 12 env MAJORDOMUS_BIN="$MJB" "$T/scripts/ci/evidence-collect" --out "$W/empty" --suite "$W/absent.tsv"
expect_grep "no report to record"

# ---------------------------------------------------------------- 5. published with its distance
# Asked of this repository's own history, read-only: the commit being published, and its parent.
HERE="$(git -C "$ROOT" rev-parse HEAD)"
PARENT="$(git -C "$ROOT" rev-parse HEAD^ 2>/dev/null)" || { echo "    this checkout has no parent commit to publish evidence of"; exit 1; }
publish() {      # publish <name> <evidence commit> [<jq filter>] — a gathered directory claiming that commit
  mkdir -p "$W/$1"
  cp "$W/ev/report.txt" "$W/ev/coverage.txt" "$W/ev/coverage.json" "$W/$1/"
  jq --arg c "$2" "(${3:-.}) | .commit = \$c" "$M" > "$W/$1/manifest.json"
  run_quiet "$W/$1.err" "$ROOT/scripts/pages" evidence --commit "$HERE" --from "$W/$1" --out "$W/$1.json" > /dev/null
}
CONFIRMED='.working_tree = "clean" | .report_tree = "clean" | .absent = [] | .refused = [] | .totals.outcomes = {pass: 2}'
publish current "$HERE" "$CONFIRMED"
jq -e '.available and .current and .state == "current" and .unconfirmed == [] and .behind == 0 and .run.id == "77"' "$W/current.json" >/dev/null \
  || { echo "    a confirmed run of the published commit is not current"; jq -c 'del(.panels)' "$W/current.json"; exit 1; }
jq -e --rawfile r "$W/ev/report.txt" '.panels[0].output == $r and ([.panels[0].links[].href] | index("https://forge.example/owner/repo/actions/runs/77/attempts/3") != null)' "$W/current.json" >/dev/null \
  || { echo "    the published panel is not the recorded report, or does not link the run"; exit 1; }
jq -e '.coverage.crate.lines.total > 0' "$W/current.json" >/dev/null \
  || { echo "    the published evidence dropped the coverage summary"; exit 1; }

# The same commit, with one thing the run cannot confirm: never current, and the reason named.
n=0
for spoil in '.working_tree = "dirty"' 'del(.working_tree)' '.report_tree = "dirty"' '.absent = ["crate"]' \
  '.refused = [{runner: "suite", reason: "x"}]'; do
  n=$((n + 1))
  publish "unconfirmed$n" "$HERE" "$CONFIRMED | $spoil"
  jq -e '.available and (.current | not) and .state == "unknown" and (.unconfirmed | length > 0)' "$W/unconfirmed$n.json" >/dev/null \
    || { echo "    a run of the published commit that cannot confirm its report ($spoil) was not published as unknown with a reason"; jq -c 'del(.panels)' "$W/unconfirmed$n.json"; exit 1; }
done
[ "$n" = 5 ] || { echo "    the unconfirmed cases did not all run ($n)"; exit 1; }
publish failed "$HERE" "$CONFIRMED | .totals.outcomes = {pass: 1, fail: 1}"
jq -e '.available and (.current | not) and .state == "stale" and (.unconfirmed | length > 0)' "$W/failed.json" >/dev/null \
  || { echo "    a run of the published commit that recorded a failure was not published as stale"; jq -c 'del(.panels)' "$W/failed.json"; exit 1; }

publish stale "$PARENT"
jq -e '.available and (.current | not) and .state == "stale" and .behind >= 1' "$W/stale.json" >/dev/null \
  || { echo "    evidence of an ancestor was published as current, or without its distance"; jq -c 'del(.panels)' "$W/stale.json"; exit 1; }
# an ancestor's run is stale however well it confirms its own report
publish stale-confirmed "$PARENT" "$CONFIRMED"
jq -e '.available and (.current | not) and .state == "stale" and .behind >= 1' "$W/stale-confirmed.json" >/dev/null \
  || { echo "    a confirmed run of an ancestor was not published as stale"; jq -c 'del(.panels)' "$W/stale-confirmed.json"; exit 1; }

publish foreign "0000000000000000000000000000000000000001"
jq -e '(.available | not) and (.reason | test("not in the history"))' "$W/foreign.json" >/dev/null \
  || { echo "    evidence from outside the published history was not refused"; jq -c . "$W/foreign.json"; exit 1; }

mkdir -p "$W/nothing"
run_quiet "$W/nothing.err" "$ROOT/scripts/pages" evidence --commit "$HERE" --from "$W/nothing" --out "$W/nothing.json" > /dev/null
jq -e '(.available | not) and (.reason | length > 0) and (has("current") | not)' "$W/nothing.json" >/dev/null \
  || { echo "    no evidence at all was published as something other than unavailable"; jq -c . "$W/nothing.json"; exit 1; }

# ---------------------------------------------------------------- 6. the workflow as wired
# This repository's own validate.yml, read job by job the way 26_ci_wiring reads it.
WF="$ROOT/.github/workflows/validate.yml"
job() {          # job <name> — that job's block
  awk -v j="  $1:" '$0 == j {f=1; next} /^  [a-z]+:$/ {f=0} f' "$WF"
}
step_with() {    # step_with <regex> — from the job block on stdin, every step whose text matches
  awk -v p="$1" '
    /^      - / { if (s ~ p) printf "%s", s; s = "" }
    { s = s $0 "\n" }
    END { if (s ~ p) printf "%s", s }'
}
suite_job="$(job suite)"; rust_job="$(job rust)"; evidence_job="$(job evidence)"
[ -n "$suite_job" ] && [ -n "$rust_job" ] && [ -n "$evidence_job" ] \
  || { echo "    validate.yml has no suite, rust or evidence job to read"; exit 1; }

# the crate's output is plain text, and it is written outside the checkout
check="$(printf '%s\n' "$rust_job" | step_with '- name: rust-check')"
printf '%s\n' "$check" | grep -qE '^ +CARGO_TERM_COLOR: never$' \
  || { echo "    the rust-check step does not turn cargo's colour off, so its output reaches the recorder unreadable"; exit 1; }
printf '%s\n' "$check" | grep -qE '^ +MJ_CARGO_TEST_OUTPUT: \$\{\{ runner\.temp \}\}/' \
  || { echo "    the rust-check step writes cargo's output into the checkout it measures"; exit 1; }

# each producing job measures the tree it ran in, excluding exactly the outputs its run names
report="$(printf '%s\n' "$suite_job" | sed -n 's/^ *MJ_TEST_REPORT: *//p' | head -1)"
[ -n "$report" ] || { echo "    the suite job names no MJ_TEST_REPORT"; exit 1; }
measure="$(printf '%s\n' "$suite_job" | step_with 'suite-tree\.json')"
printf '%s\n' "$measure" | grep -q 'status --porcelain --untracked-files=all' \
  && printf '%s\n' "$measure" | grep -qF "':(exclude)$report'" \
  && [ "$(printf '%s\n' "$measure" | grep -o ':(exclude)' | wc -l | tr -d ' ')" = 1 ] \
  || { echo "    the suite job has no step measuring its tree with only its report ($report) excluded"; exit 1; }
measure="$(printf '%s\n' "$rust_job" | step_with 'crate-tree\.json')"
printf '%s\n' "$measure" | grep -q 'status --porcelain --untracked-files=all' \
  && printf '%s\n' "$measure" | grep -qF "':(exclude)timings.tsv'" \
  && printf '%s\n' "$measure" | grep -qF "':(exclude)dist'" \
  && [ "$(printf '%s\n' "$measure" | grep -o ':(exclude)' | wc -l | tr -d ' ')" = 2 ] \
  || { echo "    the rust job has no step measuring its tree with only timings.tsv and dist excluded"; exit 1; }
# ... and before the plan it downloads into the checkout
at_tree="$(printf '%s\n' "$rust_job" | grep -n 'crate-tree\.json' | head -1 | cut -d: -f1)"
at_plan="$(printf '%s\n' "$rust_job" | grep -n 'name: ci-plan$' | head -1 | cut -d: -f1)"
[ -n "$at_tree" ] && [ -n "$at_plan" ] && [ "$at_tree" -lt "$at_plan" ] \
  || { echo "    the rust job measures its tree after downloading the plan into it"; exit 1; }
printf '%s\n' "$rust_job" | step_with 'name: ci-evidence-crate' | grep -qF '${{ runner.temp }}/crate-tree.json' \
  || { echo "    the rust job does not upload its measurement beside the crate's output"; exit 1; }

# every reader of the suite's report reads the file the suite wrote
readers=0
for f in $(printf '%s\n' "$suite_job" | grep -oE -- '--report [^ ]+' | awk '{print $2}'); do
  readers=$((readers + 1))
  [ "$f" = "$report" ] || { echo "    a step of the suite job reads --report $f, and the suite wrote $report"; exit 1; }
done
[ "$readers" -gt 0 ] || { echo "    no step of the suite job reads the suite's report"; exit 1; }

# the evidence job reads the measurements, and neither its downloads nor its output are in its checkout
fetch="$(printf '%s\n' "$evidence_job" | step_with 'download-artifact')"
printf '%s\n' "$fetch" | grep -q 'ci-evidence-suite-tree' \
  || { echo "    the evidence job does not download the suite's measurement"; exit 1; }
printf '%s\n' "$fetch" | grep -qE '^ +path: \$\{\{ runner\.temp \}\}/' \
  || { echo "    the evidence job downloads the reports into its own checkout"; exit 1; }
printf '%s\n' "$evidence_job" | step_with 'evidence-collect --out' | grep -qE -- '--out "?(\$RUNNER_TEMP|\$\{\{ runner\.temp \}\})/' \
  || { echo "    the evidence job gathers into its own checkout"; exit 1; }
printf '%s\n' "$evidence_job" | grep -qE '^ +MJ_RUN_HEAD_SHA: ' \
  || { echo "    the evidence job does not name the head a pull request was built from"; exit 1; }
