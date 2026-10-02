# majordomus-covers: none
# claims: evidence-tree-is-measured-by-the-run, evidence-a-recording-is-a-typed-run, evidence-local-recording-stays-local
# A recording carries what its run measured. The recorder does not measure its own checkout:
# `evidence stamp` measures the checkout as the run left it, naming the report, and
# `evidence record --provenance` carries that measurement into the report's executions. This
# case drives both through the Rust executable in a fixture repository of its own.
#
# What it proves, in order:
#   1  a stamp names the commit, the tree (the run's own untracked outputs excluded by path
#      segment, a tracked change never hidden), the report without a machine path and its
#      digest; an exclude that is a pattern, names the checkout, holds tracked files or is not
#      a repository path is refused; a stamp changes nothing in the checkout
#   2  (evidence-tree-is-measured-by-the-run) the recorded tree is the stamp's, not the
#      recorder's, either way round, and the digest of a clean run is taken from the commit
#   3  a local report without a measurement carries the tree `unknown` and never reads proven;
#      a CI report without one, and a local measurement that names no report, are refused and
#      write nothing
#   4  a measurement of another commit, of another report, for a report not given, or keyed
#      to another producer is refused before anything is written; a keyless measurement takes
#      the only report's producer and is refused among several
#   5  a CI recording accepts a job's minimal measurement, and its runs carry the pull
#      request's head and the event beside the commit, never in its place
#   6  (evidence-a-recording-is-a-typed-run) every recording returns a run record, the one it
#      writes with --run-record, with its commit, tree, totals, absences and unknowns; a
#      coverage summary joins it bound to its commit, its tree and the floors the threshold
#      files state, and one of another schema is refused
#   7  (evidence-local-recording-stays-local) --ledger local writes only the ignored local
#      ledger, changes no tracked file and moves no verdict
. "$ROOT/test/lib.sh"
MJB="$(rust_bin)" || rust_bin_exit $?
export MAJORDOMUS_SHARE="$ROOT/share"

# Reports and answers live outside the repository under test, except where a row puts one
# inside it on purpose.
W="$(mktemp -d "${TMPDIR:-/tmp}/mj505.XXXXXX")"; trap 'rm -rf "$W"' EXIT

ev() {           # ev <name> <args...> — the JSON answer of `evidence` into $W/<name>.json
  local name="$1"; shift
  run_quiet "$W/$name.err" "$MJB" evidence --repo "$T" --format json "$@" > "$W/$name.json"
}
jqe() {          # jqe <name> <filter> <what broke>
  jq -e "$2" "$W/$1.json" >/dev/null 2>&1 || { printf '    %s\n' "$3"; jq -c . "$W/$1.json" | head -c 2000; echo; return 1; }
}
stamp() {        # stamp <out> <stamp options...> — a measurement into $W/<out>.json
  local out="$1"; shift
  run_quiet "$W/$out.err" "$MJB" evidence --repo "$T" stamp --out "$W/$out.json" "$@" > /dev/null
}
row() {          # row <test> <filter> — a jq filter over that test's row of the tracked ledger
  jq -r --arg t "$1" ".executions[] | select(.test == \$t) | $2" "$LEDGER"
}
state() {        # state <claim> — the claim's proof state
  "$MJB" evidence --repo "$T" --format json claim "$1" 2>/dev/null | jq -r .state
}
unchanged() {    # unchanged <digest before> <what> — the tracked ledger was not written
  [ "$(sha256_of_file "$LEDGER")" = "$1" ] || { echo "    $2 changed the ledger"; exit 1; }
}
LEDGER=".ai/repo/evidence/ledger.json"

# ---------------------------------------------------------------- the fixture
"$MJ" init >/dev/null
git check-ignore -q .ai/local/evidence/ledger.json \
  || { echo "    init does not ignore .ai/local/, which the local ledger relies on"; exit 1; }
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
  - id: beta-holds
    claim: Beta holds
    source: docs/BETA.md
    implementation: lib/beta.sh
    test: test/cases/02_beta.sh
    status: advisory
YAML
printf '# Alpha\n' > docs/ALPHA.md
printf '# Beta\n' > docs/BETA.md
printf '# alpha\n' > lib/alpha.sh
printf '# beta\n' > lib/beta.sh
printf '# the alpha case\n' > test/cases/01_alpha.sh
printf '# the beta case\n' > test/cases/02_beta.sh
# an empty tracked ledger, so that every refusal below can show the ledger was not written
mkdir -p .ai/repo/evidence
printf '{\n  "version": 1,\n  "executions": []\n}\n' > "$LEDGER"
git add -A >/dev/null && git commit -qm fixture
HEAD_C="$(git rev-parse HEAD)"
printf '01_alpha\tok\t1\tparallel\n02_beta\tok\t1\tparallel\n' > "$W/all.tsv"
printf '01_alpha\tok\t9\tparallel\n' > "$W/other.tsv"
printf '01_alpha\tok\t1\tparallel\n02_beta\tFAIL\t1\tparallel\n99_ghost\tok\t1\tparallel\n' > "$W/mixed.tsv"
printf '01_alpha\tok\t5\tparallel\n' > "$W/one.tsv"

# ---------------------------------------------------------------- 1. a stamp
# Proves `evidence-tree-is-measured-by-the-run`, its measuring half.
stamp p --producer suite --report "$W/all.tsv"
jq -e --arg c "$HEAD_C" --arg d "sha256:$(sha256_of_file "$W/all.tsv")" '
    .producer == "suite" and .commit == $c and .working_tree == "clean" and .excluded == []
    and .report.path == "all.tsv" and .report.digest == $d
    and (.recorder | length > 0) and (.host.os | length > 0)' "$W/p.json" >/dev/null \
  || { echo "    the stamp does not name the commit, a clean tree and the report without a machine path"; cat "$W/p.json"; exit 1; }

# the run's own untracked outputs are excluded, a directory with everything under it
mkdir -p dist && printf 'x' > dist/out.bin && printf 'x' > timings.tsv
stamp ex2 --exclude dist --exclude timings.tsv
stamp ex1 --exclude timings.tsv
stamp ex0
for pair in ex2:clean ex1:dirty ex0:dirty; do
  got="$(jq -r .working_tree "$W/${pair%%:*}.json")"
  [ "$got" = "${pair#*:}" ] || { echo "    stamp ${pair%%:*} read '$got', not '${pair#*:}'"; exit 1; }
done
rm -rf dist timings.tsv

# an exclude never hides a tracked change: the checkout, a pattern, a tracked directory and a
# tracked file are refused, and without them the edit is seen
printf 'edited\n' >> test/cases/01_alpha.sh
for e in . '*' test test/cases/01_alpha.sh; do
  expect_exit 13 "$MJB" evidence --repo "$T" stamp --exclude "$e"
  expect_grep "\`$(printf '%s' "$e" | sed 's/[.*]/\\&/g')\`"
done
stamp edited
[ "$(jq -r .working_tree "$W/edited.json")" = dirty ] || { echo "    a tracked edit did not make the stamp dirty"; exit 1; }
git checkout -- test/cases/01_alpha.sh
for e in /tmp ../x :x; do
  expect_exit 13 "$MJB" evidence --repo "$T" stamp --exclude "$e"
  expect_grep 'is not a repository path'
done

# a report inside the checkout is one of the run's own outputs
cp "$W/all.tsv" suite.tsv
stamp inside --producer suite --report suite.tsv
jq -e '.working_tree == "clean" and .excluded == ["suite.tsv"] and .report.path == "suite.tsv"' "$W/inside.json" >/dev/null \
  || { echo "    a report inside the checkout was not excluded as the run's output"; cat "$W/inside.json"; exit 1; }
rm -f suite.tsv

# a stamp changes nothing in the checkout
before_status="$(git status --porcelain --untracked-files=all)"
stamp quiet --producer suite --report "$W/all.tsv"
[ "$(git status --porcelain --untracked-files=all)" = "$before_status" ] || { echo "    a stamp changed the checkout"; exit 1; }

# ---------------------------------------------------------------- 2. the run's tree, carried
# Proves `evidence-tree-is-measured-by-the-run`: the recorded tree is the one the stamp stated.
# A dirty stamp over a clean recorder is dirty...
printf 'pending\n' > pending.txt
stamp p-dirty --producer suite --report "$W/all.tsv"
rm -f pending.txt
expect_exit 0 "$MJB" evidence --repo "$T" record --suite "$W/all.tsv" --provenance "suite=$W/p-dirty.json"
[ "$(row suite:01_alpha .working_tree)" = dirty ] || { echo "    the recorder measured its own clean checkout over the run's dirty one"; exit 1; }
[ "$(state alpha-holds)" = inputs_unchanged ] || { echo "    a run measured dirty reads $(state alpha-holds)"; exit 1; }
# ...and a clean stamp over a dirty recorder is clean
stamp p-clean --producer suite --report "$W/all.tsv"
printf 'pending\n' > pending.txt
expect_exit 0 "$MJB" evidence --repo "$T" record --suite "$W/all.tsv" --provenance "suite=$W/p-clean.json"
rm -f pending.txt
[ "$(row suite:01_alpha .working_tree)" = clean ] || { echo "    the recorder measured its own dirty checkout over the run's clean one"; exit 1; }
[ "$(state alpha-holds)" = proven ] || { echo "    a run measured clean at HEAD reads $(state alpha-holds)"; exit 1; }
# the digest of a clean run is the commit's bytes, the ones the run executed
git show HEAD:test/cases/01_alpha.sh > "$W/committed.sh"
committed="sha256:$(sha256_of_file "$W/committed.sh")"
stamp p-clean2 --producer suite --report "$W/all.tsv"
printf 'edited after the run\n' >> test/cases/01_alpha.sh
edited="sha256:$(sha256_of_file test/cases/01_alpha.sh)"
expect_exit 0 "$MJB" evidence --repo "$T" record --suite "$W/all.tsv" --provenance "suite=$W/p-clean2.json"
git checkout -- test/cases/01_alpha.sh
[ "$(row suite:01_alpha .digest)" = "$committed" ] && [ "$committed" != "$edited" ] \
  || { echo "    the digest is not the committed test's: $(row suite:01_alpha .digest)"; exit 1; }
[ "$(state alpha-holds)" = proven ] || { echo "    the restored checkout does not read proven: $(state alpha-holds)"; exit 1; }

# ---------------------------------------------------------------- 3. no measurement
# A local report without a measurement is recorded with its tree unknown, never proven.
expect_exit 0 "$MJB" evidence --repo "$T" record --suite "$W/all.tsv"
expect_grep '^measured +nothing: the tree is recorded as unknown'
[ "$(row suite:01_alpha .working_tree)" = unknown ] || { echo "    an unmeasured run was stamped $(row suite:01_alpha .working_tree)"; exit 1; }
[ "$(state alpha-holds)" = inputs_unchanged ] || { echo "    an unmeasured run reads $(state alpha-holds)"; exit 1; }
# a CI report without a measurement is refused, and nothing is written
before="$(sha256_of_file "$LEDGER")"
expect_exit 13 "$MJB" evidence --repo "$T" record --suite "$W/all.tsv" --origin ci
expect_grep '--provenance'
unchanged "$before" "a refused CI recording"
# a local measurement that names no report cannot say which run it measured
stamp bare --producer suite
expect_exit 13 "$MJB" evidence --repo "$T" record --suite "$W/all.tsv" --provenance "suite=$W/bare.json"
expect_grep '--report'
unchanged "$before" "a refused report-less measurement"

# ---------------------------------------------------------------- 4. a measurement of something else
# another commit: refused before the ledger or the run record is written
stamp p-old --producer suite --report "$W/all.tsv"
printf 'later\n' > later.txt && git add later.txt >/dev/null && git commit -qm later
HEAD_C="$(git rev-parse HEAD)"
before="$(sha256_of_file "$LEDGER")"
expect_exit 13 "$MJB" evidence --repo "$T" record --suite "$W/all.tsv" --provenance "suite=$W/p-old.json" --run-record "$W/r4a.json"
expect_grep "$(jq -r '.commit[0:12]' "$W/p-old.json")"
expect_grep "${HEAD_C:0:12}"
unchanged "$before" "a measurement of another commit"
[ ! -e "$W/r4a.json" ] || { echo "    a refused recording wrote its run record"; exit 1; }
# another report
stamp p-other --producer suite --report "$W/other.tsv"
expect_exit 13 "$MJB" evidence --repo "$T" record --suite "$W/all.tsv" --provenance "suite=$W/p-other.json"
expect_grep 'another report'
unchanged "$before" "a measurement of another report"
# a report not given, and a key that disagrees with the file
stamp p-now --producer suite --report "$W/all.tsv"
expect_exit 13 "$MJB" evidence --repo "$T" record --suite "$W/all.tsv" --provenance "crate=$W/p-now.json"
stamp p-crate --producer crate --report "$W/all.tsv"
expect_exit 13 "$MJB" evidence --repo "$T" record --suite "$W/all.tsv" --provenance "suite=$W/p-crate.json"
expect_grep 'measures the crate report, not the suite one'
unchanged "$before" "a measurement keyed to another producer"
# a keyless measurement with no producer: the only report's, and refused among several
stamp np --report "$W/all.tsv"
expect_exit 0 "$MJB" evidence --repo "$T" record --suite "$W/all.tsv" --provenance "$W/np.json"
[ "$(row suite:01_alpha .working_tree)" = clean ] || { echo "    a keyless measurement did not reach the only report"; exit 1; }
printf 'error: could not compile `majordomus-cli` (lib) due to 1 previous error\n' > "$W/none.txt"
expect_exit 13 "$MJB" evidence --repo "$T" record --suite "$W/all.tsv" --crate-output "$W/none.txt" --provenance "$W/np.json"
expect_grep '<producer>='

# ---------------------------------------------------------------- 5. a CI job's measurement
PR_HEAD="0123456789abcdef0123456789abcdef01234567"
printf '{"commit":"%s","working_tree":"clean","excluded":["timings.tsv"]}' "$HEAD_C" > "$W/suite-tree.json"
run_quiet "$W/ci.err" env GITHUB_ACTIONS=true GITHUB_SERVER_URL=https://forge.example \
  GITHUB_REPOSITORY=owner/repo GITHUB_RUN_ID=77 GITHUB_RUN_ATTEMPT=3 GITHUB_WORKFLOW=validate \
  GITHUB_JOB=evidence GITHUB_EVENT_NAME=pull_request MJ_RUN_HEAD_SHA="$PR_HEAD" \
  "$MJB" evidence --repo "$T" record --suite "$W/all.tsv" --provenance "suite=$W/suite-tree.json" \
  --origin ci --run-record "$W/ci.json" > /dev/null
[ "$(row suite:01_alpha .working_tree)" = clean ] && [ "$(row suite:01_alpha .commit)" = "$HEAD_C" ] \
  || { echo "    the job's minimal measurement was not carried, or the commit is not the one that ran"; row suite:01_alpha .; exit 1; }
[ "$(row suite:01_alpha .run.head_sha)" = "$PR_HEAD" ] && [ "$(row suite:01_alpha .run.event)" = pull_request ] \
  || { echo "    the run does not carry the pull request's head and the event"; row suite:01_alpha .run; exit 1; }
[ "$(jq -r .id "$W/ci.json")" = "ci:github_actions:77:3" ] || { echo "    the CI run record is not named for its run"; exit 1; }

# ---------------------------------------------------------------- 6. the run record
# Proves `evidence-a-recording-is-a-typed-run`: the record returned is the record written.
stamp p6 --producer suite --report "$W/mixed.tsv"
run_quiet "$W/rec6.err" "$MJB" evidence --repo "$T" --format json record --suite "$W/mixed.tsv" \
  --provenance "suite=$W/p6.json" --run-record "$W/run.json" > "$W/rec6.json"
[ "$(jq -S .run_record "$W/rec6.json")" = "$(jq -S . "$W/run.json")" ] \
  || { echo "    the run record written is not the run record returned"; exit 1; }
jq -e --arg c "$HEAD_C" '
    (.id | test("^local:[0-9a-f]{12}:[0-9]{8}T[0-9]{6}Z$")) and .commit == $c
    and .working_tree == "clean" and .ledger == "repo" and .schema == 1
    and .totals == {"executions": 2, "outcomes": {"fail": 1, "pass": 1}, "runners": {"suite": 2}}
    and .unknown == ["suite:99_ghost"] and ([.absent[].producer] == ["crate", "coverage"])
    and ([.provenance[].producer] == ["suite"])
    and ([.executions[].test] == ["suite:01_alpha", "suite:02_beta"])' "$W/run.json" >/dev/null \
  || { echo "    the run record does not carry its commit, tree, totals, absences and unknowns"; jq -c 'del(.executions)' "$W/run.json"; exit 1; }

# coverage, from the real producer's summary, bound to the commit it was measured on
mkdir -p scripts apps/majordomus-cli/src
cp "$ROOT/scripts/rust-coverage" scripts/rust-coverage && chmod +x scripts/rust-coverage
cp "$ROOT/test/fixtures/coverage/sample.rs" apps/majordomus-cli/src/sample.rs
FLOOR=42.5
printf '%s\n' "$FLOOR" > scripts/rust-coverage-threshold
git add -A >/dev/null && git commit -qm coverage
HEAD_C="$(git rev-parse HEAD)"
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
scripts/rust-coverage --report --from "$W/export.json" --summary-json "$W/coverage.json" > "$W/coverage.txt" 2>&1 || true
[ -s "$W/coverage.json" ] || { echo "    the coverage producer wrote no summary"; cat "$W/coverage.txt"; exit 1; }
before="$(sha256_of_file "$LEDGER")"
stamp pcov --producer coverage --report "$W/coverage.json"
ev cov record --coverage "$W/coverage.json" --provenance "coverage=$W/pcov.json"
jqe cov ".run_record.coverage.commit == \"$HEAD_C\" and .run_record.coverage.working_tree == \"clean\"
    and .run_record.coverage.crate.lines.total > 0 and .run_record.coverage.floors.crate_lines == $FLOOR
    and (.run_record.coverage.floors | has(\"domain_lines\") | not) and .run_record.totals.executions == 0" \
  "the coverage record is not bound to its commit and clean tree, or its floors are not the threshold files'"
unchanged "$before" "a coverage-only recording"
ev cov-bare record --coverage "$W/coverage.json"
jqe cov-bare '.run_record.coverage.working_tree == "unknown"' "coverage without its measurement is not of an unknown tree"
jq '.schema = 2' "$W/coverage.json" > "$W/coverage2.json"
expect_exit 13 "$MJB" evidence --repo "$T" record --coverage "$W/coverage2.json"
expect_grep 'schema 2'

# ---------------------------------------------------------------- 7. a local recording stays local
# Proves `evidence-local-recording-stays-local`.
ev before_local claim alpha-holds
before="$(sha256_of_file "$LEDGER")"
before_status="$(git status --porcelain --untracked-files=all)"
stamp p7 --producer suite --report "$W/one.tsv"
ev local record --suite "$W/one.tsv" --provenance "suite=$W/p7.json" --ledger local
jqe local '.ledger == ".ai/local/evidence/ledger.json" and .run_record.ledger == "local"' \
  "the local recording does not say it wrote the local ledger"
jq -e '[.executions[] | select(.test == "suite:01_alpha")][0].seconds == 5' .ai/local/evidence/ledger.json >/dev/null \
  || { echo "    the local ledger does not hold the local run"; exit 1; }
unchanged "$before" "a local recording"
[ "$(git status --porcelain --untracked-files=all)" = "$before_status" ] || { echo "    a local recording changed a tracked file"; git status --porcelain; exit 1; }
ev after_local claim alpha-holds
[ "$(jq -S . "$W/before_local.json")" = "$(jq -S . "$W/after_local.json")" ] \
  || { echo "    a local recording moved the verdict"; jq -c '{state, execution}' "$W/after_local.json"; exit 1; }
jqe after_local '.execution.seconds != 5' "the claim shows the local run's seconds"
