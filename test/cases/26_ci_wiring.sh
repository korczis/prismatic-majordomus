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
H="$T/harness"; mkdir -p "$H/test/cases" "$H/bin" "$H/lib"
cp "$ROOT/test/run.sh" "$H/test/run.sh"; cp "$ROOT/test/lib.sh" "$H/test/lib.sh"
# test/lib.sh sources the tool's own lib/sha256.sh
cp "$ROOT/lib/sha256.sh" "$H/lib/sha256.sh"
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
# The lint reads a batched list of files: the cases must be in that list, and the list must be
# what shellcheck is handed
grep -q 'test/cases/\*\.sh > "\$lint_dir/files"' "$LINT" \
  || { echo "    shell-lint does not list test/cases for shellcheck"; exit 1; }
grep -q 'xargs -0 shellcheck' "$LINT" \
  || { echo "    shell-lint does not shellcheck the files it listed"; exit 1; }

# 8. the enforcement the policy declares is the enforcement the hooks run. doctor proves this
#    for the repository; here we prove doctor itself is invoked by both hooks and by CI.
grep -q 'majordomus doctor' "$CORE" || { echo "    core-check does not run doctor"; exit 1; }
grep -q 'majordomus watch' "$CORE" || { echo "    core-check does not run watch"; exit 1; }
grep -q 'majordomus doctor' "$ROOT/.githooks/pre-commit" || { echo "    pre-commit does not run doctor"; exit 1; }
grep -q 'majordomus finish --check' "$ROOT/.githooks/pre-push" || { echo "    pre-push does not run finish --check"; exit 1; }
# and a push that changes the crate builds its rustdoc with warnings denied first: the one
# gate of the crate nothing else stands in for, and the job the Pages deploy starts with. It
# reports and CI's rustdoc job refuses (I3830, case 3948 drives both verdicts)
grep -qE '^[[:space:]]+scripts/rust-check --doc$' "$ROOT/.githooks/pre-push" \
  || { echo "    pre-push does not build the crate's rustdoc before a push that changes the crate"; exit 1; }
grep -q 'scripts/case-numbers check' "$ROOT/.githooks/pre-push" \
  || { echo "    pre-push does not check that no two cases share a number"; exit 1; }
grep -q 'git diff --quiet "$base" "$head" -- apps/majordomus-cli/src' "$ROOT/.githooks/pre-push" \
  || { echo "    pre-push does not ask the rustdoc gate only of a push that changes the crate"; exit 1; }

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
jobs="$(grep -oE '^  [a-z][a-z-]*:$' "$W" | tr -d ' :')"
for j in $(jq -r '.gates[].job' full.json | LC_ALL=C sort -u); do
  printf '%s\n' "$jobs" | grep -qx "$j" || { echo "    the model names job $j and validate.yml has no such job"; exit 1; }
done
for g in $(jq -r '.gates[] | select(.job != "structure") | .id' full.json); do
  job="$(jq -r --arg g "$g" '.gates[] | select(.id == $g) | .job' full.json)"
  awk -v j="  $job:" '$0 == j {f=1; next} /^  [a-z][a-z-]*:$/ {f=0} f' "$W" | grep -qE "needs\.plan\.outputs\.$(printf '%s' "$g" | tr '-' '_') == 'true'" \
    || { echo "    job $job is not gated on the plan's output for $g"; exit 1; }
done
# A job's own `if:` is enough when the job carries one gate: the job runs when the gate is
# planned and the gate is what the job does. When a job carries several, the job-level `if:`
# satisfies a per-gate search while saying nothing about whether any step runs the gate — so
# each gate needs its own step-level `if:`, or the job must run scripts/ci/run-plan, which
# executes every planned gate of a job by itself. coverage-differential had neither: the
# coverage job named it in its `if:` and ran only the floor, and the gate of a guaranteed
# claim executed nowhere.
for job in $(jq -r '.gates[] | select(.job != "structure") | .job' full.json | LC_ALL=C sort -u); do
  n="$(jq -r --arg j "$job" '[.gates[] | select(.job == $j)] | length' full.json)"
  [ "$n" -gt 1 ] || continue
  blk="$(awk -v j="  $job:" '$0 == j {f=1; next} /^  [a-z][a-z_-]*:$/ {f=0} f' "$W")"
  printf '%s\n' "$blk" | grep -q "run-plan .*--job $job" && continue
  for g in $(jq -r --arg j "$job" '.gates[] | select(.job == $j) | .id' full.json); do
    o="$(printf '%s' "$g" | tr '-' '_')"
    printf '%s\n' "$blk" | grep -E '^      - if:|^        if:' | grep -q "needs\.plan\.outputs\.$o" \
      || { echo "    job $job carries $n gates and no step of it is gated on $g; the job-level if would run the job without ever running this gate"; exit 1; }
  done
done

# The step that writes the plan emits one output per gate, generated from the model; the
# job's `outputs:` map that publishes them to the other jobs is written by hand. A gate
# missing from that map is produced and never exposed: every `needs.plan.outputs.<gate>`
# reading it is the empty string, the job it gates never runs, and nothing says so — which is
# how coverage-differential, a gate of a guaranteed claim, sat wired at both ends and dead in
# the middle. The hand-written half must cover the generated one.
plan_outputs="$(awk '$0 == "  plan:" {f=1} f && /^    steps:/ {exit} f' "$W" | sed -n 's/^      \([a-z_]*\):.*/\1/p')"
for g in $(jq -r '.gates[] | select(.job != "structure") | .id' full.json); do
  printf '%s\n' "$plan_outputs" | grep -qx "$(printf '%s' "$g" | tr '-' '_')" \
    || { echo "    the plan job publishes no output for gate $g; every job gated on it would read the empty string"; exit 1; }
done

ci_needs="$(awk '$0 == "  ci:" {f=1; next} /^  [a-z][a-z-]*:$/ {f=0} f' "$W" | sed -n 's/^    needs: \[\(.*\)\]$/\1/p' | tr -d ' ' | tr ',' '\n')"
for j in plan $(jq -r '.gates[].job' full.json | LC_ALL=C sort -u); do
  printf '%s\n' "$ci_needs" | grep -qx "$j" || { echo "    the ci job does not need job $j; a red $j could not turn the required status red"; exit 1; }
done
awk '$0 == "  ci:" {f=1; next} /^  [a-z][a-z-]*:$/ {f=0} f' "$W" | grep -q '^    if: always()$' || { echo "    the ci job does not always run; a skipped job would leave the required status pending"; exit 1; }
# Every job that carries a gate leaves its receipts, and the verdict is asked for them: a
# job's `success` says no step failed, not that the gate's step ran, so scripts/ci/verdict
# refuses a selected gate with no receipt (I2416). A gate job without MJ_CI_RECEIPTS writes
# none and one that does not upload them sends none; either turns every gate of that job red
# on the first run that plans it — this says so before that run.
for j in $(jq -r '.gates[].job' full.json | LC_ALL=C sort -u); do
  blk="$(awk -v j="  $j:" '$0 == j {f=1; next} /^  [a-z][a-z_-]*:$/ {f=0} f' "$W")"
  printf '%s\n' "$blk" | grep -q '^      MJ_CI_RECEIPTS: \${{ github.workspace }}/receipts.tsv$' \
    || { echo "    job $j carries gates and sets no MJ_CI_RECEIPTS; its gates would leave no receipt"; exit 1; }
  printf '%s\n' "$blk" | grep -qE '^          name: ci-receipts-' \
    || { echo "    job $j does not upload its receipts (ci-receipts-*); the verdict would find none"; exit 1; }
done
ci_blk="$(awk '$0 == "  ci:" {f=1; next} /^  [a-z][a-z-]*:$/ {f=0} f' "$W")"
printf '%s\n' "$ci_blk" | grep -q 'pattern: ci-receipts-\*' || { echo "    the ci job does not download the receipts"; exit 1; }
printf '%s\n' "$ci_blk" | grep -q 'scripts/ci/verdict .*--receipts receipts' || { echo "    the ci verdict is not asked for the receipts"; exit 1; }
# a started run reaches its verdict: a pull request's runs queue per pull request, and a
# newer commit never cancels the run in progress (a two-hour suite under frequent pushes
# never finished); master's runs are each their own group
grep -q "group: validate-\${{ github.event_name == 'pull_request' && github.event.pull_request.number || github.run_id }}" "$W" || { echo "    validate.yml does not group runs per pull request"; exit 1; }
grep -q "^  cancel-in-progress: false$" "$W" || { echo "    validate.yml cancels a run in progress; a superseding push would again leave no verdict"; exit 1; }
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
  # The schedule has an arm of its own, and so has the dispatch: a dispatch with no input is
  # the person's "every gate", and one with plan=change is the release pipeline asking for
  # the record's verdict as a pull request would have had it (case 996). What must not
  # happen is the default arm — a push — asking for the on-demand gates.
  grep -qE '^[[:space:]]*schedule\) scripts/ci-plan --full "\$EVENT on \$REF" --on-demand' "$W" \
    || { echo "    validate.yml does not give the schedule an arm of its own that asks for the on-demand gates"; exit 1; }
  grep -qE '^[[:space:]]*workflow_dispatch\)$' "$W" \
    || { echo "    validate.yml does not tell the dispatch apart from a push, so a push would ask for the on-demand gates"; exit 1; }
  grep -E '^[[:space:]]*\*\) scripts/ci-plan --full "\$EVENT on \$REF"' "$W" | grep -q -- '--on-demand' \
    && { echo "    the default arm of validate.yml asks for the on-demand gates, so a push would wait for their runners"; exit 1; }
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
# the deploy job's own steps: another job of the workflow has its own bound and its own sum,
# and counting its steps here made the deploy job look overrun the day one gained a timeout
step_to="$(awk '$0 == "  deploy:" {f=1; next} /^  [a-z-]+:$/ {f=0} f && /^        timeout-minutes: / {s += $2} END {print s+0}' "$P")"
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

# 12. a label that changes nothing starts no second verdict (I4347). `labeled` starts validate
#     for every label; on 2026-10-28 six of eight trains (#2631, #2630, #2593, #2592, #2591,
#     #2501) were opened with `--label train` and ran their head twice. Only ci:full and hold
#     change what a run does. Any other label on a head that already carries a verdict gives
#     the runners back in the plan job, and the ci job writes that verdict again — never a
#     skip, which GitHub counts as passing. scripts/ci/carried-verdict decides from the runs
#     GitHub lists; this section feeds it those lists through a stub `gh`, offline.
CV="$ROOT/scripts/ci/carried-verdict"
[ -x "$CV" ] || { echo "    scripts/ci/carried-verdict is missing or not executable"; exit 1; }
# the wiring: the plan job asks after its checkout and before the release path, on a pull
# request alone; the ci job asks first, and every step that reads the gates' artifacts or
# decides from them waits on its answer
plan_steps="$(awk '/^  plan:$/ {f=1; next} /^  [a-z-]+:$/ {f=0} f' "$W")"
yield_at="$(printf '%s\n' "$plan_steps" | grep -n 'scripts/ci/carried-verdict yield' | head -1 | cut -d: -f1)"
co_at="$(printf '%s\n' "$plan_steps" | grep -n 'uses: actions/checkout@' | head -1 | cut -d: -f1)"
rp_at="$(printf '%s\n' "$plan_steps" | grep -n 'scripts/ci/release-path yield' | head -1 | cut -d: -f1)"
[ -n "$yield_at" ] || { echo "    the plan job does not ask scripts/ci/carried-verdict yield"; exit 1; }
[ -n "$co_at" ] && [ "$co_at" -lt "$yield_at" ] || { echo "    the plan job asks carried-verdict before its checkout"; exit 1; }
[ -n "$rp_at" ] && [ "$yield_at" -lt "$rp_at" ] || { echo "    the plan job asks the release path before carried-verdict, so a redundant run waits for a release"; exit 1; }
printf '%s\n' "$plan_steps" | grep -B8 'scripts/ci/carried-verdict yield' | grep -q "if: github.event_name == 'pull_request'" \
  || { echo "    the plan job's carried-verdict step is not limited to a pull request"; exit 1; }
for v in 'ACTION: ${{ github.event.action }}' 'LABEL: ${{ github.event.label.name }}' 'HEAD_SHA: ${{ github.event.pull_request.head.sha }}'; do
  [ "$(grep -cF "$v" "$W")" -ge 2 ] || { echo "    validate.yml does not pass $v to both carried-verdict steps through the environment"; exit 1; }
done
grep -E 'carried-verdict (yield|verdict)' "$W" | grep -q '\${{' \
  && { echo "    a carried-verdict step interpolates an expression into its command; a label name is the pull request author's text"; exit 1; }
ci_steps="$(awk '/^  ci:$/ {f=1; next} /^  [a-z-]+:$/ {f=0} f' "$W")"
printf '%s\n' "$ci_steps" | grep -q '^      - id: carried$' || { echo "    the ci job has no step with id carried"; exit 1; }
printf '%s\n' "$ci_steps" | grep -q 'scripts/ci/carried-verdict verdict' || { echo "    the ci job does not ask scripts/ci/carried-verdict verdict"; exit 1; }
first_dl="$(printf '%s\n' "$ci_steps" | grep -n 'actions/download-artifact@' | head -1 | cut -d: -f1)"
cv_at="$(printf '%s\n' "$ci_steps" | grep -n 'scripts/ci/carried-verdict verdict' | head -1 | cut -d: -f1)"
[ -n "$first_dl" ] && [ "$cv_at" -lt "$first_dl" ] || { echo "    the ci job downloads the plan before it asks whether the verdict is carried"; exit 1; }
unguarded="$(printf '%s\n' "$ci_steps" | awk '
  /^      - / { if (step != "" && reads && !guard) print step; step=$0; reads=0; guard=0 }
  /actions\/download-artifact@|scripts\/ci\/verdict|actions\/upload-artifact@/ { reads=1 }
  /steps\.carried\.outputs\.carried != .true./ { guard=1 }
  END { if (step != "" && reads && !guard) print step }')"
[ -z "$unguarded" ] || { echo "    a ci step that reads or decides from the gates runs on a carried verdict:"; printf '      %s\n' "$unguarded"; exit 1; }
# the labels the script calls plan-changing are exactly the labels validate.yml reads
read_labels="$( { grep -oE "contains\(github\.event\.pull_request\.labels\.\*\.name, '[^']+'\)" "$W" | sed -E "s/.*, '([^']+)'\)/\1/"
                  grep -oE '\*,[a-z:-]+,\*\)' "$W" | sed -E 's/\*,([^,]+),\*\)/\1/'; } | sort -u | tr '\n' ' ')"
script_labels="$(sed -n 's/^PLAN_LABELS="\(.*\)"$/\1/p' "$CV" | tr ' ' '\n' | sort -u | tr '\n' ' ')"
[ -n "$read_labels" ] && [ "$read_labels" = "$script_labels" ] \
  || { echo "    validate.yml reads the labels '$read_labels' and carried-verdict treats '$script_labels' as plan-changing"; exit 1; }

# the behaviour, against a stub forge
CVX="$T/cv"; CVLOG="$CVX/gh.log"; mkdir -p "$CVX/bin" "$CVX/fx"
cat > "$CVX/bin/gh" <<'STUB'
#!/usr/bin/env bash
printf '%s\n' "$*" >> "$GH_STUB_LOG"
[ ! -f "$GH_STUB_FX/down" ] || { echo "HTTP 503: the forge is down" >&2; exit 1; }
[ "$1" = api ] || { echo "stub gh: only api" >&2; exit 1; }
shift
case "$*" in
  "-X POST "*) echo '{}' ;;
  *"/actions/workflows/validate.yml/runs?"*)
    sha="${*#*head_sha=}"; sha="${sha%%&*}"
    if [ -f "$GH_STUB_FX/vruns-$sha.json" ]; then cat "$GH_STUB_FX/vruns-$sha.json"; else echo '{"workflow_runs":[]}'; fi ;;
  *"/actions/runs/"*"/jobs?"*)
    id="${*#*/actions/runs/}"; id="${id%%/*}"; cat "$GH_STUB_FX/jobs-$id.json" ;;
  *) echo "stub gh: no answer for: $*" >&2; exit 1 ;;
esac
STUB
chmod +x "$CVX/bin/gh"
cv() { # <command> <action> <label> <sha>
  : > "$CVLOG"; : > "$CVX/out"
  env -u GITHUB_ACTIONS -u GITHUB_STEP_SUMMARY -u GITHUB_REPOSITORY \
    PATH="$CVX/bin:$PATH" GH_STUB_LOG="$CVLOG" GH_STUB_FX="$CVX/fx" GITHUB_OUTPUT="$CVX/out" \
    MJ_CARRIED_VERDICT_CANCEL_WAIT=0 \
    "$CV" "$1" --action "$2" --label "$3" --sha "$4" --self 900 --repo o/r
}
# a completed run of a head, and its jobs' conclusions: <sha> <id> <created> <plan> <ci>
cv_run() {
  local f="$CVX/fx/vruns-$1.json"
  [ -f "$f" ] || echo '{"workflow_runs":[]}' > "$f"
  jq --argjson id "$2" --arg at "$3" '.workflow_runs += [{id: $id, created_at: $at, status: "completed"}]' "$f" > "$f.n" && mv "$f.n" "$f"
  printf '{"jobs":[{"name":"plan","conclusion":"%s"},{"name":"build","conclusion":"success"},{"name":"ci","conclusion":"%s"}]}' "$4" "$5" > "$CVX/fx/jobs-$2.json"
}
cv_reset() { rm -f "$CVX/fx"/*; }
cv_says() { [ "$LAST_OUT" = "$1" ] || { echo "    carried-verdict decided '$LAST_OUT', not '$1' ($2)"; exit 1; }; }
cv_posts() { [ "$(grep -c -- '^api -X POST' "$CVLOG" || true)" = "$1" ] || { echo "    carried-verdict wrote to the forge $1 time(s) expected ($2):"; cat "$CVLOG"; exit 1; }; }

# a. opened, synchronize and reopened plan their change, whatever the head carries, and ask nothing
cv_reset; cv_run aaa 101 2026-10-28T10:00:00Z success success
for a in opened synchronize reopened; do
  expect_exit 0 cv decide "$a" "" aaa || exit 1
  case "$LAST_OUT" in "run a $a event plans its change") ;; *) echo "    a $a event was not planned: $LAST_OUT"; exit 1 ;; esac
  [ ! -s "$CVLOG" ] || { echo "    a $a event read the forge"; exit 1; }
done
# b. ci:full and hold change what the run does: never carried, even on a green head
for l in ci:full hold; do
  expect_exit 0 cv decide labeled "$l" aaa || exit 1
  cv_says "run the label $l changes what this run does" "labeled $l"
done
# c. a train label on a green head: carried; the plan job cancels its own run and nothing else;
#    the ci job writes the success again, and says so in its output
expect_exit 0 cv decide labeled train aaa || exit 1
cv_says "carry 101 success" "labeled train on a green head"
expect_exit 0 cv yield labeled train aaa || exit 1
cv_posts 1 "yield on a carried verdict"
grep -q '^api -X POST repos/o/r/actions/runs/900/cancel$' "$CVLOG" || { echo "    yield did not cancel its own run, or cancelled another:"; cat "$CVLOG"; exit 1; }
expect_exit 0 cv verdict labeled train aaa || exit 1
grep -qx 'carried=true' "$CVX/out" || { echo "    a carried success did not write carried=true"; exit 1; }
cv_posts 0 "verdict never writes to the forge"
# d. a red head keeps its red: a run that ended at its first red (plan success, ci failure)
#    is a verdict, and the carried ci fails
cv_reset; cv_run bbb 201 2026-10-28T10:00:00Z success failure
expect_exit 0 cv decide labeled priority:p0 bbb || exit 1
cv_says "carry 201 failure" "labeled on a red head"
expect_exit 1 cv verdict labeled priority:p0 bbb || { echo "    a carried failure did not fail the ci job"; exit 1; }
grep -qx 'carried=true' "$CVX/out" || { echo "    a carried failure did not write carried=true"; exit 1; }
# e. a run that yielded (plan cancelled, ci failure for want of a plan) is no verdict: the
#    older real run is carried, and a head whose only runs yielded plans
cv_reset; cv_run ccc 301 2026-10-28T10:00:00Z success success; cv_run ccc 302 2026-10-28T11:00:00Z cancelled failure
expect_exit 0 cv decide labeled train ccc || exit 1
cv_says "carry 301 success" "a yielded newer run over an older verdict"
cv_reset; cv_run ddd 401 2026-10-28T10:00:00Z cancelled failure; cv_run ddd 402 2026-10-28T11:00:00Z success cancelled
expect_exit 0 cv decide labeled train ddd || exit 1
cv_says "run head ddd carries no verdict yet" "only yielded or unfinished runs"
expect_exit 0 cv yield labeled train ddd || exit 1
cv_posts 0 "yield with no verdict to carry"
expect_exit 0 cv verdict labeled train ddd || exit 1
grep -qx 'carried=false' "$CVX/out" || { echo "    a run with nothing to carry did not write carried=false"; exit 1; }
# f. the newest verdict wins, and the run itself is never its own source
cv_reset; cv_run eee 501 2026-10-28T10:00:00Z success failure; cv_run eee 502 2026-10-28T12:00:00Z success success
expect_exit 0 cv decide labeled train eee || exit 1
cv_says "carry 502 success" "two verdicts of one head"
cv_reset; cv_run fff 900 2026-10-28T10:00:00Z success success
expect_exit 0 cv decide labeled train fff || exit 1
cv_says "run head fff carries no verdict yet" "the run's own earlier attempt"
# g. a forge that cannot be read decides run, in every command, and nothing is cancelled
cv_reset; cv_run ggg 701 2026-10-28T10:00:00Z success success; touch "$CVX/fx/down"
expect_exit 0 cv yield labeled train ggg || exit 1
cv_posts 0 "yield on a forge that cannot be read"
expect_exit 0 cv verdict labeled train ggg || exit 1
grep -qx 'carried=false' "$CVX/out" || { echo "    an unreadable forge carried a verdict"; exit 1; }
# h. the replay of 2026-10-28: each train opened with --label train is one opened event (it
#    plans and runs its gates, and writes the head's verdict) and one labeled event (it
#    carries). One gate-running run per head.
cv_reset; n=0
for pr in 2631 2630 2593 2592 2591 2501; do
  n=$((n + 1)); sha="replay$pr"
  expect_exit 0 cv decide opened "" "$sha" || exit 1
  case "$LAST_OUT" in run*) ;; *) echo "    #$pr: the opened event did not plan: $LAST_OUT"; exit 1 ;; esac
  cv_run "$sha" "$((1000 + n))" 2026-10-28T10:00:00Z success success   # the opened run's verdict
  expect_exit 0 cv decide labeled train "$sha" || exit 1
  cv_says "carry $((1000 + n)) success" "#$pr: the labeled event of a train"
done
