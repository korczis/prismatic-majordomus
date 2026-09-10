# The protection is only real if something runs it. An excellent test that CI never invokes
# is worth nothing, so this case checks the harness and the workflows themselves.
#
# It reads the repository's own files rather than the disposable fixture, because the subject
# is this repository's CI configuration.
. "$ROOT/test/lib.sh"
W="$ROOT/.github/workflows/validate.yml"
CORE="$ROOT/scripts/ci/core-check"; LINT="$ROOT/scripts/ci/shell-lint"
[ -f "$W" ] || { echo "    no validate workflow"; exit 1; }
[ -x "$CORE" ] && [ -x "$LINT" ] || { echo "    scripts/ci/core-check or scripts/ci/shell-lint is missing or not executable"; exit 1; }

# 1. one canonical validation command, and CI calls exactly that. This workflow decides
#    whether a change may merge, .github/workflows/pages.yml decides what the public site
#    shows, .github/workflows/release.yml decides what a tag publishes, and none of them
#    repeats another's work. Every workflow has a model under .ai/repo/ci/ — that is the
#    invariant, not how many there are; the shapes of the other two are held by
#    test/cases/97_pages_fast_path.sh and test/cases/87_release_pipeline.sh.
grep -q 'bash test/run.sh' "$W" || { echo "    validate.yml does not run bash test/run.sh"; exit 1; }
for w in "$ROOT"/.github/workflows/*.yml; do
  n="$(basename "$w" .yml)"; [ "$n" = validate ] && n=gates    # validate.yml's model is gates.yaml
  [ -f "$ROOT/.ai/repo/ci/$n.yaml" ] || { echo "    .github/workflows/$(basename "$w") has no model under .ai/repo/ci/"; exit 1; }
done
[ -f "$ROOT/.github/workflows/pages.yml" ] || { echo "    no pages.yml; nothing publishes the site"; exit 1; }
grep -q 'bash test/run.sh' "$ROOT/.github/workflows/pages.yml" && { echo "    pages.yml repeats the validation workflow's suite"; exit 1; }
grep -qE 'actions/deploy-pages|scripts/site-deploy' "$ROOT/.github/workflows/pages.yml" || { echo "    pages.yml does not deploy the site"; exit 1; }

# 2. the runner discovers every case rather than naming a list that can fall behind
grep -q 'test/cases/\*\.sh' "$ROOT/test/run.sh" || { echo "    run.sh does not glob test/cases"; exit 1; }

# 3. the runner's exit status reflects failure, and neither the workflow nor the gate scripts
#    swallow it
grep -q '^\[ "$fail" = 0 \]' "$ROOT/test/run.sh" || { echo "    run.sh does not exit non-zero on failure"; exit 1; }
for f in "$W" "$CORE" "$LINT" "$ROOT"/scripts/ci/timed "$ROOT"/scripts/rust-check; do
  # a required step must not end in a construct that discards the exit code
  if grep -nE '^\s+run:.*\|\|\s*true' "$f"; then echo "    ${f##*/}: a run step swallows failure with || true"; exit 1; fi
  if grep -nE 'continue-on-error:\s*true' "$f"; then echo "    ${f##*/}: a step is allowed to fail"; exit 1; fi
  if grep -nE '^\s+set \+e' "$f"; then echo "    ${f##*/}: a step disables errexit"; exit 1; fi
done
for f in "$CORE" "$LINT"; do grep -q '^set -eu' "$f" || { echo "    ${f##*/} does not run under set -eu"; exit 1; }; done

# 4. the runner actually fails when a case fails. This is the mutation: a case that cannot
#    pass must turn the whole run red, or every other assertion in this suite is decoration.
#    The probe cases live in a private copy of the harness (the runner and its library, in a
#    scratch tree of this case), never in this checkout's test/cases: the suite may be
#    running other cases beside this one, and they read that directory.
H="$T/harness"; mkdir -p "$H/test/cases" "$H/bin"
cp "$ROOT/test/run.sh" "$H/test/run.sh"; cp "$ROOT/test/lib.sh" "$H/test/lib.sh"
tmpcase="$H/test/cases/zz_ci_wiring_probe.sh"
printf '# deliberately failing probe\nexit 1\n' > "$tmpcase"
if bash "$H/test/run.sh" zz_ci_wiring_probe >/dev/null 2>&1; then
  echo "    test/run.sh reported success while a case failed"; exit 1
fi
# ... and passes when it passes, so the check above is not vacuous
printf '# probe\nexit 0\n' > "$tmpcase"
bash "$H/test/run.sh" zz_ci_wiring_probe >/dev/null 2>&1 || {
  echo "    test/run.sh reported failure while a case passed"; exit 1; }
# the same two facts through the parallel runner, which renders from per-case logs
printf '# deliberately failing probe\nexit 1\n' > "$tmpcase"
if MJ_TEST_JOBS=2 bash "$H/test/run.sh" >/dev/null 2>&1; then
  echo "    MJ_TEST_JOBS=2 test/run.sh reported success while a case failed"; exit 1
fi
printf '# probe\nexit 0\n' > "$tmpcase"
MJ_TEST_JOBS=2 bash "$H/test/run.sh" >/dev/null 2>&1 || {
  echo "    MJ_TEST_JOBS=2 test/run.sh reported failure while a case passed"; exit 1; }
rm -f "$tmpcase"

# 4b. a filter that matches nothing is a usage error, not an empty success. `run.sh <name>`
#     on a case that does not exist printed "0 passed, 0 failed" and exited 0, and that zero
#     was read as "it passed" — the same shape as a green CI that never ran the suite.
if ( cd "$ROOT" && bash test/run.sh zz_no_such_case_exists >/dev/null 2>&1 ); then
  echo "    test/run.sh reported success for a case that does not exist"; exit 1
fi
if ( cd "$ROOT" && MJ_TEST_JOBS=2 bash test/run.sh zz_no_such_case_exists >/dev/null 2>&1 ); then
  echo "    MJ_TEST_JOBS=2 test/run.sh reported success for a case that does not exist"; exit 1
fi
# 4c. and an empty case directory is a usage error in both modes
if bash "$H/test/run.sh" >/dev/null 2>&1 || MJ_TEST_JOBS=2 bash "$H/test/run.sh" >/dev/null 2>&1; then
  echo "    test/run.sh reported success with no cases at all"; exit 1
fi

# 5. every command the CLI dispatches has a behavioural case somewhere in the suite, so a
#    new command cannot ship with no coverage at all. The command list is read from the
#    dispatcher itself, so adding a command to bin/majordomus adds it to this check.
dispatched=$(grep -oE '^  [a-z|]+\)$' "$ROOT/bin/majordomus" | tr -d ' )' | tr '|' '\n' | LC_ALL=C sort -u)
[ "$(printf '%s\n' "$dispatched" | wc -w | tr -d ' ')" -ge 15 ] || {
  echo "    could not read the command list from the dispatcher (got: $dispatched)"; exit 1; }
for c in $dispatched; do
  grep -rqE "MJ\" (--json )?$c( |$)|MJ' $c " "$ROOT"/test/cases/*.sh || {
    echo "    no test case invokes: majordomus $c"; exit 1; }
done
# the loop above is not vacuous: a command with no case is detected
if grep -rqE "MJ\" nosuchcommand( |$)" "$ROOT"/test/cases/*.sh; then
  echo "    the coverage check matches a command that no case invokes"; exit 1
fi

# 6. the continuity commands are reached by CI in this checkout, not only in fixtures: the
#    structure job dispatches every gate the model assigns to it from the written plan,
#    the model assigns shell-lint and core-check to it on every plan (`always: true`), and
#    core-check runs them. The workflow names no gate itself: a job that listed its steps
#    was a second declaration of the model, and four gates it did not list never ran.
grep -qE '^\s+run: (MJ_CI_TIMINGS=[^ ]+ )?scripts/ci/run-plan --plan plan.json --job structure$' "$W" \
  || { echo "    validate.yml's structure job does not dispatch its gates from the plan (scripts/ci/run-plan --plan plan.json --job structure)"; exit 1; }
G="$ROOT/.ai/repo/ci/gates.yaml"
for g in shell-lint core-check; do
  awk -v g="$g" '$0 ~ "^  - id: " g "$" {f=1; next} f && /^  - id: / {f=0} f' "$G" | grep -qE '^\s+job: structure$' \
    || { echo "    gates.yaml does not assign $g to the structure job"; exit 1; }
  awk -v g="$g" '$0 ~ "^  - id: " g "$" {f=1; next} f && /^  - id: / {f=0} f' "$G" | grep -qE '^\s+always: true$' \
    || { echo "    gates.yaml does not mark $g as run on every plan (always: true)"; exit 1; }
done
grep -qE '^\s+runs: scripts/ci/shell-lint$' "$G" || { echo "    gates.yaml does not run scripts/ci/shell-lint"; exit 1; }
grep -qE '^\s+runs: scripts/ci/core-check$' "$G" || { echo "    gates.yaml does not run scripts/ci/core-check"; exit 1; }
for c in context history handover checkpoint decision question prompt; do
  grep -qE "majordomus $c" "$CORE" || { echo "    core-check never runs majordomus $c"; exit 1; }
done

# 6b. the canonical project model is a blocking gate, and the projection is proved offline.
#     A model that cannot be executed is not a warning, and a projection that only works on a
#     machine with a token is not a projection anyone can trust.
grep -qE '^bin/majordomus plan validate$' "$CORE" || { echo "    core-check does not run plan validate as a blocking step"; exit 1; }
grep -q 'scripts/github-sync --plan' "$CORE" || { echo "    core-check does not check the GitHub projection offline"; exit 1; }
# shell-lint discovers what it lints rather than listing it, so the assertion is over what
# it would actually lint, not over a name written inside it
"$LINT" --list > "$T/linted"
for f in scripts/github-sync bin/majordomus lib/common.sh scripts/generate-site-data; do
  grep -qx "$f" "$T/linted" || { echo "    shell-lint would not lint $f"; exit 1; }
done
# the gate scripts must need no credential: a gate that only runs where a token exists is a
# gate that does not run on a fork
grep -qiE 'secrets\.|GITHUB_TOKEN|gh auth' "$CORE" "$LINT" && { echo "    a gate script reaches for a credential"; exit 1; }

# 7. shellcheck covers the tests as well as the tool: an unchecked test is an unchecked gate
grep -q 'shellcheck.*test/cases/\*\.sh' "$LINT" || { echo "    shell-lint does not shellcheck test/cases"; exit 1; }

# 8. the enforcement the policy declares is the enforcement the hooks run. doctor proves this
#    for the repository; here we prove doctor itself is invoked by both hooks and by CI.
grep -q 'majordomus doctor' "$CORE" || { echo "    core-check does not run doctor"; exit 1; }
grep -q 'majordomus watch' "$CORE" || { echo "    core-check does not run watch"; exit 1; }
grep -q 'majordomus doctor' "$ROOT/.githooks/pre-commit" || { echo "    pre-commit does not run doctor"; exit 1; }
grep -q 'majordomus finish --check' "$ROOT/.githooks/pre-push" || { echo "    pre-push does not run finish --check"; exit 1; }

# 9. the workflow is an adapter over the CI model, not a second copy of it: every gate the
#    model declares is carried by a job the workflow has, every job that a plan can skip is
#    gated on the plan's output for its gate, the verdict job needs every job and always runs,
#    and no job of this workflow publishes. The model is read here, so a gate added to it
#    without its job fails this case rather than pending forever.
PLAN="$ROOT/scripts/ci-plan"; MODEL="$ROOT/.ai/repo/ci/gates.yaml"
[ -x "$PLAN" ] && [ -f "$MODEL" ] || { echo "    scripts/ci-plan or .ai/repo/ci/gates.yaml is missing"; exit 1; }
expect_exit 0 "$PLAN" --check
grep -q 'scripts/ci-plan ' "$W" || { echo "    validate.yml does not run scripts/ci-plan"; exit 1; }
grep -q 'scripts/ci/verdict ' "$W" || { echo "    validate.yml does not run scripts/ci/verdict"; exit 1; }
"$PLAN" --full "probe" > full.json
jobs="$(grep -oE '^  [a-z]+:$' "$W" | tr -d ' :')"
for j in $(jq -r '.gates[].job' full.json | LC_ALL=C sort -u); do
  printf '%s\n' "$jobs" | grep -qx "$j" || { echo "    the model names job $j and validate.yml has no such job"; exit 1; }
done
for g in $(jq -r '.gates[] | select(.job != "structure") | .id' full.json); do
  job="$(jq -r --arg g "$g" '.gates[] | select(.id == $g) | .job' full.json)"
  awk -v j="  $job:" '$0 == j {f=1; next} /^  [a-z]+:$/ {f=0} f' "$W" | grep -qE "needs\.plan\.outputs\.$(printf '%s' "$g" | tr '-' '_') == 'true'" \
    || { echo "    job $job is not gated on the plan's output for $g"; exit 1; }
done
ci_needs="$(awk '$0 == "  ci:" {f=1; next} /^  [a-z]+:$/ {f=0} f' "$W" | sed -n 's/^    needs: \[\(.*\)\]$/\1/p' | tr -d ' ' | tr ',' '\n')"
for j in plan $(jq -r '.gates[].job' full.json | LC_ALL=C sort -u); do
  printf '%s\n' "$ci_needs" | grep -qx "$j" || { echo "    the ci job does not need job $j; a red $j could not turn the required status red"; exit 1; }
done
awk '$0 == "  ci:" {f=1; next} /^  [a-z]+:$/ {f=0} f' "$W" | grep -q '^    if: always()$' || { echo "    the ci job does not always run; a skipped job would leave the required status pending"; exit 1; }
# a pull request's runs are superseded by the next commit; master's never are
grep -q "cancel-in-progress: \${{ github.event_name == 'pull_request' }}" "$W" || { echo "    validate.yml does not cancel superseded pull-request runs only"; exit 1; }
# no bare push trigger: a branch with a pull request is validated once, as that pull request
awk '/^on:/{f=1} /^permissions:/{f=0} f' "$W" | grep -A1 '^  push:' | grep -q 'branches: \[master\]' || { echo "    validate.yml runs on every push rather than on master and pull requests"; exit 1; }

# 10. a verdict that does not arrive is not a verdict. The model marks the gates whose runner
#     this repository cannot get on demand (the macOS ones); the workflow asks for them where
#     they can be afforded — the schedule, a dispatch, a pull request labelled ci:full — and
#     nowhere else. On 2026-09-10 four master runs had every Linux job finished, five of them
#     red, and their `ci` job had never started, because it needs three macOS jobs that had
#     been queued for hours; master carried the defects overnight looking green. So: if the
#     model marks a gate on-demand, something must still run it, and a routine push must not
#     wait for it.
ondemand="$(awk '/^  - id: /{id=$3} /^    on-demand: true$/{print id}' "$G" | tr '\n' ' ')"
if [ -n "$ondemand" ]; then
  grep -qE '^  schedule:$' "$W" \
    || { echo "    the model marks gate(s) on-demand ($ondemand) and validate.yml has no schedule to run them"; exit 1; }
  grep -qE 'ci-plan --full "\$EVENT on \$REF" --on-demand' "$W" \
    || { echo "    the schedule and the dispatch do not ask the planner for the on-demand gates (--on-demand)"; exit 1; }
  grep -qE 'schedule\|workflow_dispatch\)' "$W" \
    || { echo "    validate.yml does not tell the schedule and the dispatch apart from a push, so a push would ask for the on-demand gates"; exit 1; }
  grep -qE 'ci-plan --full "pull request labelled ci:full" --on-demand' "$W" \
    || { echo "    the ci:full label does not ask for the on-demand gates, so a reviewer cannot request them"; exit 1; }
  grep -qE 'ci-plan --base HEAD\^1 --head HEAD.*--on-demand' "$W" \
    && { echo "    an ordinary pull request asks for the on-demand gates; its verdict would queue behind them"; exit 1; }
  grep -qE '\*\) scripts/ci-plan --full "\$EVENT on \$REF" > plan\.json ;;' "$W" \
    || { echo "    a push does not plan without the on-demand gates; the verdict it needs would never arrive"; exit 1; }
  # and every on-demand gate is still a gate: its job exists and the verdict still needs it
  for g in $ondemand; do
    job="$(awk -v g="$g" '$0 ~ "^  - id: " g "$" {f=1; next} f && /^  - id: / {f=0} f' "$G" | sed -n 's/^    job: //p' | head -1)"
    [ -n "$job" ] || { echo "    the on-demand gate $g names no job"; exit 1; }
    printf '%s\n' "$ci_needs" | grep -qx "$job" || { echo "    the ci job does not need $job, the job of the on-demand gate $g"; exit 1; }
  done
fi

# 11. the publication probe of pages.yml survives, and which bound fires is arithmetic rather
#     than luck. A step killed by its own timeout is a failed step — and the probe is
#     continue-on-error, so a reported one; a job killed by the *job's* timeout is `cancelled`
#     over every step at once, the ones that had already succeeded included. On 2026-09-10 a
#     five-minute job bound killed the deploy of 67ec7c2aa mid-wait at 13:56:12 with the
#     publication already done at 13:52:20, and a deployment that had succeeded was reported
#     as a failure. So the job's bound must strictly exceed the sum of its steps', and the
#     probe's own --timeout must fit inside the step that runs it, or the diagnostic a reader
#     needs — which commit the site is still serving — is replaced by a kill.
P="$ROOT/.github/workflows/pages.yml"
grep -q 'scripts/pages verify --commit' "$P" \
  || { echo "    pages.yml no longer verifies that the site serves the commit it published"; exit 1; }
job_to="$(awk '$0 == "  deploy:" {f=1; next} /^  [a-z-]+:$/ {f=0} f && /^    timeout-minutes: / {print $2; exit}' "$P")"
step_to="$(awk '/^        timeout-minutes: / {s += $2} END {print s+0}' "$P")"
[ -n "$job_to" ] || { echo "    the pages deploy job carries no timeout; a hang holds a runner for GitHub's default six hours"; exit 1; }
[ "$step_to" -gt 0 ] || { echo "    no step of the pages deploy job carries a timeout, so only the job's bound can fire and every overrun reads as cancelled"; exit 1; }
[ "$job_to" -gt "$step_to" ] \
  || { echo "    the pages deploy job is bounded at $job_to min and its steps at $step_to min together; the job's bound fires first, and a finished deployment then reports cancelled"; exit 1; }
probe_to="$(grep 'scripts/pages verify --commit' "$P" | sed -n 's/.*--timeout \([0-9][0-9]*\).*/\1/p' | head -1)"
probe_step="$(awk '/^      - id: public$/ {f=1} f && /^        timeout-minutes: / {print $2; exit}' "$P")"
[ -n "$probe_to" ] && [ -n "$probe_step" ] \
  || { echo "    the publication probe's wait or its step bound could not be read from pages.yml"; exit 1; }
[ "$probe_to" -lt "$(( probe_step * 60 ))" ] \
  || { echo "    the publication probe waits ${probe_to}s inside a step bounded at ${probe_step}min; the step kills it before it can say what the site is serving"; exit 1; }
