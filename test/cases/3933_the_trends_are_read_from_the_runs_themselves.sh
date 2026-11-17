# majordomus-covers: none
# The coverage and CI-cost series the site draws is read from the runs themselves (I3933).
# Coverage and CI cost were measured on every run and kept nowhere past an artifact's
# retention, so a slow regression was seen only by a person reading runs. `scripts/pages
# trends` reads the series at publish time from the forge alone: master's completed validate
# runs, each run's jobs, and each run's own coverage-summary artifact, which is
# scripts/rust-coverage's --summary-json of that run's one measurement. Nothing accumulates
# between publications and no ref is read, so the same answers must give the same bytes, and a
# run whose artifact has expired must say it was not measured rather than that it measured 0.
# Through the real script, against a stand-in `gh` that applies each call's --jq to fixture
# answers with the real jq, read-only:
#
#   0  two runs, listed newest first as the forge lists them: the series is oldest first, in
#      the trends/v1 shape, with each job's queued and run seconds from its own timestamps and
#      each job's median over the runs it ran in
#   1  the older run's artifact has expired: its coverage is null, not zero, and its CI cost
#      is still there
#   2  the newer run's coverage is the fixture summary's crate totals exactly
#   3  the same answers twice give identical bytes
#   4  a forge that cannot be read is exit 12 and leaves no file behind
#   5  the workflow hands rust-coverage its summary file through the variable rust-coverage
#      itself reads, so the artifact this reads is the measurement's own output
#   6  the runs are read several at a time (I4380), and one at a time gives the same bytes
#   7  a call that does not return is stopped at its own bound and is a forge that could not
#      be read (exit 12, no file), not a reading that waits out the step
#   8  a forge that never answers anything, through the trends step's own line of pages.yml
#      run as GitHub runs it (bash -e): the listing is stopped at its bound, the reading is
#      exit 12 with no file, the step says the ::warning and ends 0, so the deploy goes on;
#      and a reading that outlives the line's own `timeout` is exit 124, the same warning,
#      and the same 0 (I4380)
#   9  a call the forge answers with a 5xx or a rate limit is asked once more inside its own
#      bound, and the reading is the same bytes; a refusal is the answer, asked once (I4380)
. "$ROOT/test/lib.sh"

mj_runs python3 || skip "$(mj_runs_why python3)"
command -v unzip >/dev/null 2>&1 || skip "no unzip, which scripts/pages trends reads an artifact with"

F="$T/forge"
mkdir -p "$F" "$T/bin"

# ---------------------------------------------------------------- the forge's answers
# Newest first, as the forge lists runs. 101 is older and its artifact has expired.
cat > "$F/runs.json" <<'J'
{"total_count": 2, "workflow_runs": [
  {"id": 102, "head_sha": "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb", "conclusion": "success",
   "created_at": "2026-10-21T10:00:00Z", "run_started_at": "2026-10-21T10:00:05Z", "updated_at": "2026-10-21T10:40:00Z"},
  {"id": 101, "head_sha": "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa", "conclusion": "failure",
   "created_at": "2026-10-20T09:00:00Z", "run_started_at": "2026-10-20T09:00:00Z", "updated_at": "2026-10-20T09:31:00Z"}
]}
J
# the jobs out of name order, so the series' order is its own
cat > "$F/jobs-101.json" <<'J'
{"total_count": 2, "jobs": [
  {"id": 12, "name": "suite (1)", "conclusion": "failure",
   "created_at": "2026-10-20T09:00:10Z", "started_at": "2026-10-20T09:02:10Z", "completed_at": "2026-10-20T09:30:00Z"},
  {"id": 11, "name": "plan", "conclusion": "success",
   "created_at": "2026-10-20T09:00:00Z", "started_at": "2026-10-20T09:00:30Z", "completed_at": "2026-10-20T09:01:00Z"}
]}
J
cat > "$F/jobs-102.json" <<'J'
{"total_count": 2, "jobs": [
  {"id": 22, "name": "coverage", "conclusion": "success",
   "created_at": "2026-10-21T10:01:00Z", "started_at": "2026-10-21T10:04:00Z", "completed_at": "2026-10-21T10:35:05Z"},
  {"id": 21, "name": "plan", "conclusion": "success",
   "created_at": "2026-10-21T10:00:05Z", "started_at": "2026-10-21T10:00:15Z", "completed_at": "2026-10-21T10:01:00Z"}
]}
J
printf '{"total_count": 1, "artifacts": [{"id": 7, "name": "coverage-summary", "expired": true}]}\n' > "$F/artifacts-101.json"
printf '{"total_count": 1, "artifacts": [{"id": 8, "name": "coverage-summary", "expired": false}]}\n' > "$F/artifacts-102.json"
# the shape scripts/rust-coverage --summary-json writes
cat > "$F/coverage-summary.json" <<'J'
{
  "crate": {
    "functions": {"covered": 4100, "percent": 82.0, "total": 5000},
    "lines": {"covered": 45123, "percent": 90.25, "total": 50000},
    "regions": {"covered": 61000, "percent": 76.25, "total": 80000}
  },
  "domain": {"files": [], "functions": {"covered": 1, "percent": 100.0, "total": 1},
             "lines": {"covered": 1, "percent": 100.0, "total": 1}, "missing": [],
             "regions": {"covered": 1, "percent": 100.0, "total": 1}},
  "files": {},
  "measurement": "scripts/rust-coverage",
  "schema": 1,
  "test_code": "excluded"
}
J
python3 -I -c 'import sys,zipfile; z=zipfile.ZipFile(sys.argv[1],"w"); z.write(sys.argv[2],"coverage-summary.json"); z.close()' \
  "$F/8.zip" "$F/coverage-summary.json" || { echo "    could not zip the fixture summary"; exit 1; }

# A stand-in `gh api`: each URL to its fixture, with the call's --jq applied by the real jq, as
# gh applies it (strings raw, everything else compact JSON). Every call is logged.
cat > "$T/bin/gh" <<EOF
#!/usr/bin/env bash
printf '%s\n' "\$*" >> "$F/calls.log"
jqx=""; url=""
while [ \$# -gt 0 ]; do case "\$1" in
  api|--paginate) shift ;;
  --jq) jqx="\$2"; shift 2 ;;
  *) url="\$1"; shift ;;
esac; done
[ -f "$F/down" ] && exit 1
[ -f "$F/denied" ] && { echo "HTTP 401: Bad credentials (https://api.github.com/x)" >&2; exit 1; }
if [ -f "$F/flaky" ]; then
  rm -f "$F/flaky"; echo "HTTP \$(cat "$F/flaky-answer" 2>/dev/null || echo 502): the forge is busy" >&2; exit 1
fi
[ -f "$F/never" ] && exec sleep 60
case "\$url" in *"/jobs?"*) [ -f "$F/hang" ] && exec sleep 60 ;; esac
case "\$url" in
  *"/actions/workflows/validate.yml/runs?branch=master&status=completed&per_page="*) f=runs.json ;;
  *"/actions/runs/101/jobs?"*) f=jobs-101.json ;;
  *"/actions/runs/102/jobs?"*) f=jobs-102.json ;;
  *"/actions/runs/101/artifacts?name=coverage-summary") f=artifacts-101.json ;;
  *"/actions/runs/102/artifacts?name=coverage-summary") f=artifacts-102.json ;;
  *"/actions/artifacts/8/zip") cat "$F/8.zip"; exit 0 ;;
  *) echo "gh stand-in: no answer for \$url" >&2; exit 1 ;;
esac
if [ -n "\$jqx" ]; then jq -cr "\$jqx" "$F/\$f"; else cat "$F/\$f"; fi
EOF
chmod +x "$T/bin/gh"

# ---------------------------------------------------------------- 0. shape and order
PATH="$T/bin:$PATH" GITHUB_REPOSITORY=example/repo expect_exit 0 \
  "$ROOT/scripts/pages" trends --out "$T/one.json" --runs 2
expect_grep '2 run\(s\) of master.s validation, 1 with the coverage their run measured, 1 whose coverage the forge no longer keeps'
expect_grep 'branch=master&status=completed&per_page=2' "$F/calls.log"
jq -e '
  keys_unsorted == ["schema", "generated_from", "runs", "job_medians"]
  and .schema == "trends/v1"
  and .generated_from == [101, 102]
  and ([.runs[].run_id] == [101, 102])
  and ([.runs[] | keys] | all(. == ["conclusion", "coverage", "head_sha", "jobs", "queued_seconds_total",
                                     "run_id", "started_at", "wall_seconds"]))
  and ([.runs[].jobs[] | keys] | all(. == ["conclusion", "name", "queued_s", "run_s"]))' "$T/one.json" >/dev/null \
  || { echo "    trends.json is not the trends/v1 series, oldest first"; jq . "$T/one.json"; exit 1; }
# the older run: plan queued 30 s and ran 30 s, suite (1) queued 120 s and ran 1670 s, the run
# ended when its last job did, 1800 s after it started
jq -e '.runs[0] | .head_sha == "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"
  and .started_at == "2026-10-20T09:00:00Z"
  and .jobs == [{name: "plan", queued_s: 30, run_s: 30, conclusion: "success"},
                {name: "suite (1)", queued_s: 120, run_s: 1670, conclusion: "failure"}]
  and .queued_seconds_total == 150 and .wall_seconds == 1800' "$T/one.json" >/dev/null \
  || { echo "    the older run's CI cost is not its jobs' own timestamps"; jq '.runs[0]' "$T/one.json"; exit 1; }
jq -e '.runs[1] | .jobs[0].name == "coverage" and .jobs[0].queued_s == 180 and .jobs[0].run_s == 1865
  and .queued_seconds_total == 190 and .wall_seconds == 2100' "$T/one.json" >/dev/null \
  || { echo "    the newer run's CI cost is not its jobs' own timestamps"; jq '.runs[1]' "$T/one.json"; exit 1; }

# each job's median over the runs it ran in, slowest first: plan ran 30 s and 45 s
jq -e '.job_medians == [
  {name: "coverage", runs: 1, median_run_s: 1865, median_queued_s: 180},
  {name: "suite (1)", runs: 1, median_run_s: 1670, median_queued_s: 120},
  {name: "plan", runs: 2, median_run_s: 37.5, median_queued_s: 20}]' "$T/one.json" >/dev/null \
  || { echo "    the job medians are not the runs' own"; jq .job_medians "$T/one.json"; exit 1; }

# ---------------------------------------------------------------- 1. an expired artifact is null
jq -e '.runs[0].coverage | type == "null"' "$T/one.json" >/dev/null \
  || { echo "    the run whose artifact expired carries a coverage reading (it must be null, never 0)"; jq '.runs[0].coverage' "$T/one.json"; exit 1; }
if grep -q '"artifacts/7/zip' "$F/calls.log"; then echo "    the expired artifact was asked for"; exit 1; fi

# ---------------------------------------------------------------- 2. the totals are the summary's
want="$(jq -cS '.crate | map_values({covered, total})' "$F/coverage-summary.json")"
got="$(jq -cS '.runs[1].coverage' "$T/one.json")"
[ "$got" = "$want" ] || { echo "    the coverage totals are not the run's own summary: got $got, want $want"; exit 1; }

# ---------------------------------------------------------------- 3. deterministic
PATH="$T/bin:$PATH" GITHUB_REPOSITORY=example/repo expect_exit 0 \
  "$ROOT/scripts/pages" trends --out "$T/two.json" --runs 2
cmp -s "$T/one.json" "$T/two.json" || { echo "    two readings of the same answers differ"; diff "$T/one.json" "$T/two.json" | head -20; exit 1; }

# ---------------------------------------------------------------- 4. an unreadable forge
cp "$T/one.json" "$T/three.json"
: > "$F/down"
PATH="$T/bin:$PATH" GITHUB_REPOSITORY=example/repo expect_exit 12 \
  "$ROOT/scripts/pages" trends --out "$T/three.json"
expect_grep 'could not be listed'
[ ! -e "$T/three.json" ] || { echo "    an older reading was left at --out when the forge could not be read"; exit 1; }
rm -f "$F/down"
expect_exit 2 "$ROOT/scripts/pages" trends --runs 0

# ---------------------------------------------------------------- 5. the summary is the measurement's
grep -q 'SUMMARY_JSON="\$2"; export SUMMARY_JSON' "$ROOT/scripts/rust-coverage" \
  && grep -q 'os.environ.get("SUMMARY_JSON"' "$ROOT/scripts/rust-coverage" \
  || { echo "    scripts/rust-coverage no longer reads its summary path from SUMMARY_JSON, which validate.yml sets"; exit 1; }
awk '/^  coverage:$/ { f = 1; next } /^  [a-z-]+:$/ { f = 0 } f' "$ROOT/.github/workflows/validate.yml" > "$T/coverage-job.yml"
grep -q 'SUMMARY_JSON: \${{ github.workspace }}/coverage-summary.json' "$T/coverage-job.yml" \
  && grep -q 'name: coverage-summary$' "$T/coverage-job.yml" \
  && grep -q 'path: coverage-summary.json$' "$T/coverage-job.yml" \
  || { echo "    the coverage job does not keep rust-coverage's summary as the coverage-summary artifact"; exit 1; }
grep -q 'scripts/pages trends' "$ROOT/.github/workflows/pages.yml" \
  || { echo "    the pages publication does not read the series"; exit 1; }

# ---------------------------------------------------------------- 6. read in parallel, same bytes
PATH="$T/bin:$PATH" GITHUB_REPOSITORY=example/repo MJ_TRENDS_PARALLEL=1 expect_exit 0 \
  "$ROOT/scripts/pages" trends --out "$T/serial.json" --runs 2
cmp -s "$T/one.json" "$T/serial.json" || { echo "    one run at a time reads other bytes than several"; diff "$T/one.json" "$T/serial.json" | head -20; exit 1; }
MJ_TRENDS_PARALLEL=0 expect_exit 2 "$ROOT/scripts/pages" trends --runs 2

# ---------------------------------------------------------------- 7. a call that hangs is bounded
if command -v timeout >/dev/null 2>&1; then
  : > "$F/hang"; t0="$(date +%s)"
  PATH="$T/bin:$PATH" GITHUB_REPOSITORY=example/repo MJ_TRENDS_CALL_SECONDS=2 expect_exit 12 \
    "$ROOT/scripts/pages" trends --out "$T/hung.json" --runs 2
  took=$(( $(date +%s) - t0 ))
  expect_grep 'the jobs of run 10[12] could not be read'
  [ ! -e "$T/hung.json" ] || { echo "    a reading stopped at its bound left a file behind"; exit 1; }
  [ "$took" -lt 30 ] || { echo "    a hanging call was not stopped at its 2 s bound: the reading took $took s"; exit 1; }
  rm -f "$F/hang"
else
  echo "    (no timeout(1) here: the per-call bound is not exercised)"
fi

# ---------------------------------------------------------------- 8. a forge that never answers
if command -v timeout >/dev/null 2>&1; then
  step="$(awk '/- name: trends$/ { f = 1 } f && /^        run: / { sub(/^        run: /, ""); print; exit }' "$ROOT/.github/workflows/pages.yml")"
  case "$step" in
    'timeout --kill-after=10 200 scripts/pages trends || echo "::warning title=No trends::'*) ;;
    *) echo "    the trends step of pages.yml is not the bounded, degrading line this runs: $step"; exit 1 ;;
  esac
  # the step's own line, with the series written to the scratch dir instead of the tree
  line="${step/scripts\/pages trends/scripts/pages trends --out $T/never.json}"
  run_step() {
    (cd "$ROOT" && PATH="$T/bin:$PATH" GITHUB_REPOSITORY=example/repo bash -e -c "$1") > "$T/step.out" 2>&1
  }
  : > "$F/never"; : > "$F/calls.log"
  t0="$(date +%s)"
  MJ_TRENDS_CALL_SECONDS=2 run_step "$line" \
    || { echo "    the trends step did not end 0 when the forge never answered: the deploy would stop"; cat "$T/step.out"; exit 1; }
  took=$(( $(date +%s) - t0 ))
  expect_grep 'pages trends: the validate runs of example/repo could not be listed' "$T/step.out"
  expect_grep '^::warning title=No trends::.*\(exit 12\)' "$T/step.out"
  [ ! -e "$T/never.json" ] || { echo "    a forge that never answered left a trends.json"; exit 1; }
  [ "$took" -lt 30 ] || { echo "    the listing was not stopped at its 2 s bound: the step took $took s"; exit 1; }
  [ "$(grep -c . "$F/calls.log")" = 1 ] || { echo "    more was asked of a forge whose listing never answered"; cat "$F/calls.log"; exit 1; }
  # the line's own bound, 3 s here for the 200 of pages.yml, outlived by calls bounded at 8:
  # a call's own timeout(1) is a process group of its own, which the line's bound does not
  # reach, so the stand-in's sleep is left to that 8 s and not to the stand-in's 60
  short="${line/timeout --kill-after=10 200 /timeout --kill-after=2 3 }"
  [ "$short" != "$line" ] || { echo "    the step's own bound could not be shortened for the test"; exit 1; }
  t0="$(date +%s)"
  MJ_TRENDS_CALL_SECONDS=8 run_step "$short" \
    || { echo "    the trends step did not end 0 when its own bound stopped the reading"; cat "$T/step.out"; exit 1; }
  took=$(( $(date +%s) - t0 ))
  expect_grep '^::warning title=No trends::.*\(exit 124\)' "$T/step.out"
  [ ! -e "$T/never.json" ] || { echo "    a reading stopped at the step's bound left a trends.json"; exit 1; }
  [ "$took" -lt 30 ] || { echo "    the step's own 3 s bound did not stop the reading: it took $took s"; exit 1; }
  rm -f "$F/never"
else
  echo "    (no timeout(1) here: the trends step's bounds are not exercised)"
fi

# ---------------------------------------------------------------- 9. an outage is asked again once
for answer in 502 "403: API rate limit exceeded for installation"; do
  printf '%s\n' "$answer" > "$F/flaky-answer"; : > "$F/flaky"; : > "$F/calls.log"
  PATH="$T/bin:$PATH" GITHUB_REPOSITORY=example/repo MJ_TRENDS_PARALLEL=1 expect_exit 0 \
    "$ROOT/scripts/pages" trends --out "$T/retried.json" --runs 2
  [ "$(grep -c 'actions/workflows/validate.yml/runs' "$F/calls.log")" = 2 ] \
    || { echo "    the listing answered HTTP $answer was not asked again exactly once"; cat "$F/calls.log"; exit 1; }
  cmp -s "$T/one.json" "$T/retried.json" || { echo "    a retried listing ($answer) reads other bytes"; exit 1; }
done
rm -f "$F/flaky" "$F/flaky-answer"
: > "$F/denied"; : > "$F/calls.log"
PATH="$T/bin:$PATH" GITHUB_REPOSITORY=example/repo expect_exit 12 \
  "$ROOT/scripts/pages" trends --out "$T/denied.json" --runs 2
[ "$(grep -c . "$F/calls.log")" = 1 ] || { echo "    a refusal was asked again"; cat "$F/calls.log"; exit 1; }
rm -f "$F/denied"

echo "    the series is master's runs oldest first, each with its own jobs' cost and its own measurement's totals, null where the forge no longer keeps one"
