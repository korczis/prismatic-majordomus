# majordomus-covers: none
# A run that ends at its first red says which red that was (I2726).
#
# I2664's self-cancel (case 3019) turns the failing job's conclusion into "cancelled" like every
# other job's and hides its log until the run is over. On 2026-10-15 the grand top's runs
# 38665445251 and 38667725668 were taken for external cancellations twice; the reds were job
# site, step "derived site data in sync", and job interactions, step "build the site". What
# must stay true:
#
#   1  `scripts/ci/first-red job` names the job running on this runner and its failed step, as
#      an error annotation and in the step summary, and never fails the job that asked: a
#      listing it cannot read still names the job GITHUB_JOB gives;
#   2  `scripts/ci/first-red run` names the first step of the run that failed, by completion
#      time, as "ended at its first red: <job> / <step>", and says nothing when none failed;
#      an integration/* top, which is not cancelled (PR #1222), words both as one red of many;
#   3  in validate.yml every self-cancel step is preceded by the step that names the red, on
#      the same condition, and the ci job names the first red before it renders the verdict.
#
# The forge is replaced by --jobs listings in the shape the jobs API answers; nothing reaches
# the network. The wiring is read with awk and each refusal is proved on a mutated copy.
. "$ROOT/test/lib.sh"
FR="$ROOT/scripts/ci/first-red"; W="$ROOT/.github/workflows/validate.yml"
[ -x "$FR" ] || { echo "    scripts/ci/first-red is missing or not executable"; exit 1; }
command -v jq >/dev/null 2>&1 || { echo "    jq is needed to read the listing"; exit 1; }

cat > "$T/jobs.json" <<'EOF'
{"total_count": 3, "jobs": [
  {"name": "structure", "status": "completed", "conclusion": "success", "runner_name": "r1",
   "steps": [{"name": "core-check", "number": 3, "conclusion": "success", "completed_at": "2026-10-15T01:20:00Z"}]},
  {"name": "site", "status": "in_progress", "conclusion": null, "runner_name": "r2",
   "steps": [{"name": "build the site", "number": 4, "conclusion": "success", "completed_at": "2026-10-15T01:21:00Z"},
             {"name": "derived site data in sync", "number": 5, "conclusion": "failure", "completed_at": "2026-10-15T01:25:00Z"},
             {"name": "this job's red is named", "number": 9, "conclusion": null}]},
  {"name": "interactions", "status": "in_progress", "conclusion": null, "runner_name": "r3",
   "steps": [{"name": "build the site", "number": 4, "conclusion": "failure", "completed_at": "2026-10-15T01:22:00Z"}]}
]}
EOF

# ---------------------------------------------------------------- 1. the job names its red
: > "$T/summary.md"
out="$(RUNNER_NAME=r2 GITHUB_JOB=site GITHUB_STEP_SUMMARY="$T/summary.md" "$FR" job --jobs "$T/jobs.json")" \
  || { echo "    first-red job failed the job that asked"; exit 1; }
printf '%s\n' "$out" | grep -qF '::error title=This run ends at its first red::site / derived site data in sync failed' \
  || { echo "    first-red job did not annotate job site's failed step:"; printf '      %s\n' "$out"; exit 1; }
grep -qF '**site / derived site data in sync** failed' "$T/summary.md" \
  || { echo "    first-red job did not put the red at the top of the step summary"; exit 1; }
# another runner is another job
out="$(RUNNER_NAME=r3 GITHUB_STEP_SUMMARY=/dev/null "$FR" job --jobs "$T/jobs.json")"
printf '%s\n' "$out" | grep -qF 'interactions / build the site failed' \
  || { echo "    first-red job on runner r3 did not name job interactions:"; printf '      %s\n' "$out"; exit 1; }
# a listing that cannot be read is said, with the job GITHUB_JOB names, and the job goes on
out="$(RUNNER_NAME=r2 GITHUB_JOB=site GITHUB_STEP_SUMMARY=/dev/null "$FR" job --jobs "$T/absent.json")" \
  || { echo "    first-red job failed the job when the listing could not be read"; exit 1; }
printf '%s\n' "$out" | grep -qE '^::error .*::site / \(its steps could not be read: the listing .*absent.json cannot be read\)' \
  || { echo "    an unreadable listing was not said with the job's name:"; printf '      %s\n' "$out"; exit 1; }

# ---------------------------------------------------------------- 2. the run names its first red
: > "$T/summary.md"
out="$(GITHUB_STEP_SUMMARY="$T/summary.md" "$FR" run --jobs "$T/jobs.json")" \
  || { echo "    first-red run failed"; exit 1; }
[ "$out" = "ended at its first red: interactions / build the site" ] \
  || { echo "    first-red run did not name the earliest failed step (interactions, 01:22):"; printf '      %s\n' "$out"; exit 1; }
grep -qF '**ended at its first red: interactions / build the site**' "$T/summary.md" \
  || { echo "    first-red run did not write the red to the summary"; exit 1; }
jq '.jobs |= map(.steps |= map(.conclusion = "success"))' "$T/jobs.json" > "$T/green.json"
out="$(GITHUB_STEP_SUMMARY=/dev/null "$FR" run --jobs "$T/green.json")"
[ -z "$out" ] || { echo "    first-red run named a red in a run with none: $out"; exit 1; }
rc=0; "$FR" sideways >/dev/null 2>&1 || rc=$?
[ "$rc" = 2 ] || { echo "    an unknown command exited $rc, not 2"; exit 1; }

# an integration top is not cancelled at its first red (PR #1222): it is named as one red of many
out="$(GITHUB_HEAD_REF=integration/next-top RUNNER_NAME=r2 GITHUB_STEP_SUMMARY=/dev/null "$FR" job --jobs "$T/jobs.json")"
printf '%s\n' "$out" | grep -qF "::error title=This job's red::site / derived site data in sync failed; this is an integration top, so the run is not cancelled" \
  || { echo "    on an integration top the job's red was worded as the end of the run:"; printf '      %s\n' "$out"; exit 1; }
out="$(GITHUB_HEAD_REF=integration/next-top GITHUB_STEP_SUMMARY=/dev/null "$FR" run --jobs "$T/jobs.json")"
[ "$out" = "its first red, of every red this integration top reports: interactions / build the site" ] \
  || { echo "    on an integration top the ci job said the run ended at its first red: $out"; exit 1; }

# ---------------------------------------------------------------- 3. the workflow is wired
judge() { # <workflow> -> one finding per line; nothing when it holds
  awk '
    /^  [a-z0-9_-]+:[ \t]*$/ { job = $0; gsub(/[ :\t]/, "", job); prev_name = ""; named = 0 }
    /^      - name: this job.s red is named$/ { named = 1; want_run = 1; next }
    want_run && /^        if: / { cond = $0; sub(/^        if: /, "", cond) }
    want_run && /^        run: / {
      want_run = 0
      if ($0 !~ /run: scripts\/ci\/first-red job$/) printf "job %s names its red with something else: %s\n", job, $0
      named_cond = cond; next }
    /^      - name: a failed branch run ends here$/ {
      if (!named) printf "job %s cancels its run without naming the red first\n", job
      else want_cancel_if = 1
      named = 0; next }
    want_cancel_if && /^        if: / {
      want_cancel_if = 0; c = $0; sub(/^        if: /, "", c)
      if (c != named_cond) printf "job %s names its red on %s but cancels on %s\n", job, named_cond, c }
    /scripts\/ci\/first-red run$/ { if (job == "ci") seen_run = NR }
    /scripts\/ci\/verdict / { if (job == "ci") seen_verdict = NR }
    END {
      if (!seen_run) print "the ci job does not name the first red"
      else if (seen_verdict && seen_run > seen_verdict) print "the ci job names the first red after the verdict"
    }' "$1"
}
out="$(judge "$W")"
[ -z "$out" ] || { echo "    validate.yml does not name the red before it cancels:"; printf '      %s\n' "$out"; exit 1; }
n="$(grep -c "^      - name: this job's red is named\$" "$W")"
[ "$n" -ge 10 ] || { echo "    only $n job(s) name their red; the reader has lost the jobs"; exit 1; }
grep -B6 'scripts/ci/first-red run$' "$W" | grep -q 'GH_TOKEN: \${{ github.token }}' \
  || { echo "    the ci job names the first red without a token that can list the run's jobs"; exit 1; }

mutant() { # <name> <awk program> <expected finding (ERE)>
  awk "$2" "$W" > "$T/$1.yml"
  cmp -s "$W" "$T/$1.yml" && { echo "    mutation $1 changed nothing"; exit 1; }
  out="$(judge "$T/$1.yml")"
  printf '%s\n' "$out" | grep -qE -- "$3" || { echo "    mutation $1 was not refused as /$3/; findings:"; printf '      %s\n' "$out"; exit 1; }
}
# one job loses the naming step: its name line goes, so the cancel follows nothing named
mutant unnamed '!d && /^      - name: this job.s red is named$/ { sub(/this job.s red is named/, "something else"); d = 1 } 1' \
  'cancels its run without naming the red first'
# the ci job no longer names it
mutant no-run '!/scripts\/ci\/first-red run$/' 'the ci job does not name the first red'

echo "    a gate job that ends its run names its job and failed step before it cancels, a listing that cannot be read is said and fails nothing, and the ci job names the run's first red before its verdict"
