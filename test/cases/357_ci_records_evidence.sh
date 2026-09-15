# majordomus-covers: none
# Proves claim evidence-recorded-in-ci (docs/CLAIMS.yaml), whose `test:` names this case.
#
# CI's evidence is kept where the run happened and published with the commit it proves (ADR 68).
# This case drives each half through the real executable and the real scripts, in fixtures of
# its own, because the behaviours that matter are about which run and which commit:
#
#   1  a `ci` recording inside a GitHub Actions environment names the run: id, attempt, job and
#      the address the provider gave, exactly
#   2  the same recording as `local`, or as `ci` outside a run, names no run, and the ledger row
#      carries no `run` member at all
#   3  scripts/ci/evidence-collect gathers a run: the manifest names the commit and the run,
#      counts the outcomes of that run only, keeps a failure as a failure, names what was
#      absent, and summarises coverage through scripts/rust-coverage
#   4  nothing to gather is refused, not written as an empty success
#   5  scripts/pages evidence publishes evidence of the published commit as current, evidence
#      of an ancestor as stale with its distance, and a manifest from outside the history, or
#      none at all, as unavailable with a reason — never as current
. "$ROOT/test/lib.sh"
MJB="$(rust_bin)" || rust_bin_exit $?
export MAJORDOMUS_SHARE="$ROOT/share"

# Reports, answers and gathered directories live outside every repository under test.
W="$(mktemp -d "${TMPDIR:-/tmp}/mj357.XXXXXX")"; trap 'rm -rf "$W"' EXIT

actions() {      # actions <command...> — run inside a described GitHub Actions run
  env GITHUB_ACTIONS=true GITHUB_SERVER_URL=https://forge.example GITHUB_REPOSITORY=owner/repo \
    GITHUB_RUN_ID=77 GITHUB_RUN_ATTEMPT=3 GITHUB_WORKFLOW=validate GITHUB_JOB=evidence "$@"
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
mkdir -p "$T/scripts/ci" "$T/apps/majordomus-cli/src"
cp "$ROOT/scripts/ci/evidence-collect" "$T/scripts/ci/evidence-collect"
cp "$ROOT/scripts/rust-coverage" "$T/scripts/rust-coverage"
cp "$ROOT/test/fixtures/coverage/sample.rs" "$T/apps/majordomus-cli/src/sample.rs"
chmod +x "$T/scripts/ci/evidence-collect" "$T/scripts/rust-coverage"
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

run_quiet "$W/c1.err" actions env MAJORDOMUS_BIN="$MJB" "$T/scripts/ci/evidence-collect" \
  --out "$W/ev" --suite "$W/suite.tsv" --crate-output "$W/no-such-output.txt" --coverage "$W/export.json" > /dev/null
M="$W/ev/manifest.json"
jq -e --arg c "$HEAD_T" '.commit == $c and .run.id == "77"' "$M" >/dev/null \
  || { echo "    the manifest does not name the commit and the run gathered"; jq -c . "$M"; exit 1; }
jq -e '.totals.executions == 2 and .totals.outcomes.pass == 1 and .totals.outcomes.fail == 1' "$M" >/dev/null \
  || { echo "    the manifest does not count this run's outcomes, failure included"; jq -c .totals "$M"; exit 1; }
jq -e '.absent == ["crate"]' "$M" >/dev/null \
  || { echo "    what the run did not produce is not named as absent"; jq -c .absent "$M"; exit 1; }
jq -e '[.panels[].label] == ["evidence", "coverage"] and (.panels | all(.exit | type == "number"))' "$M" >/dev/null \
  || { echo "    the manifest does not describe both recorded commands"; jq -c .panels "$M"; exit 1; }
jq -e '.crate.lines.total > 0 and .crate.lines.covered < .crate.lines.total and (.files | has("apps/majordomus-cli/src/sample.rs"))' "$W/ev/coverage.json" >/dev/null \
  || { echo "    the coverage summary does not carry the measured sample, uncovered line included"; cat "$W/ev/coverage.json"; exit 1; }
[ -s "$W/ev/report.txt" ] && [ -s "$W/ev/ledger.json" ] || { echo "    the report or the ledger was not kept beside the manifest"; exit 1; }

# ---------------------------------------------------------------- 4. nothing to gather
expect_exit 12 env MAJORDOMUS_BIN="$MJB" "$T/scripts/ci/evidence-collect" --out "$W/empty" --suite "$W/absent.tsv"
expect_grep "no report to record"

# ---------------------------------------------------------------- 5. published with its distance
# Asked of this repository's own history, read-only: the commit being published, and its parent.
HERE="$(git -C "$ROOT" rev-parse HEAD)"
PARENT="$(git -C "$ROOT" rev-parse HEAD^ 2>/dev/null)" || { echo "    this checkout has no parent commit to publish evidence of"; exit 1; }
publish() {      # publish <name> <evidence commit> — a gathered directory claiming that commit
  mkdir -p "$W/$1"
  cp "$W/ev/report.txt" "$W/ev/coverage.txt" "$W/ev/coverage.json" "$W/$1/"
  jq --arg c "$2" '.commit = $c' "$M" > "$W/$1/manifest.json"
  run_quiet "$W/$1.err" "$ROOT/scripts/pages" evidence --commit "$HERE" --from "$W/$1" --out "$W/$1.json" > /dev/null
}
publish current "$HERE"
jq -e '.available and .current and .behind == 0 and .run.id == "77"' "$W/current.json" >/dev/null \
  || { echo "    evidence of the published commit is not current"; jq -c 'del(.panels)' "$W/current.json"; exit 1; }
jq -e --rawfile r "$W/ev/report.txt" '.panels[0].output == $r and ([.panels[0].links[].href] | index("https://forge.example/owner/repo/actions/runs/77/attempts/3") != null)' "$W/current.json" >/dev/null \
  || { echo "    the published panel is not the recorded report, or does not link the run"; exit 1; }
jq -e '.coverage.crate.lines.total > 0' "$W/current.json" >/dev/null \
  || { echo "    the published evidence dropped the coverage summary"; exit 1; }

publish stale "$PARENT"
jq -e '.available and (.current | not) and .behind >= 1' "$W/stale.json" >/dev/null \
  || { echo "    evidence of an ancestor was published as current, or without its distance"; jq -c 'del(.panels)' "$W/stale.json"; exit 1; }

publish foreign "0000000000000000000000000000000000000001"
jq -e '(.available | not) and (.reason | test("not in the history"))' "$W/foreign.json" >/dev/null \
  || { echo "    evidence from outside the published history was not refused"; jq -c . "$W/foreign.json"; exit 1; }

mkdir -p "$W/nothing"
run_quiet "$W/nothing.err" "$ROOT/scripts/pages" evidence --commit "$HERE" --from "$W/nothing" --out "$W/nothing.json" > /dev/null
jq -e '(.available | not) and (.reason | length > 0) and (has("current") | not)' "$W/nothing.json" >/dev/null \
  || { echo "    no evidence at all was published as something other than unavailable"; jq -c . "$W/nothing.json"; exit 1; }
