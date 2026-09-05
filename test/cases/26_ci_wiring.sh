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

# 1. one canonical validation command, and CI calls exactly that; the site deploys from the
#    same workflow's verified run, so there is no second workflow repeating the suite
grep -q 'bash test/run.sh' "$W" || { echo "    validate.yml does not run bash test/run.sh"; exit 1; }
[ "$(ls "$ROOT"/.github/workflows/*.yml | wc -l | tr -d ' ')" = 1 ] || { echo "    a second workflow exists beside validate.yml; the site deploys from validate.yml's own verified run"; ls "$ROOT"/.github/workflows/; exit 1; }
grep -qE 'actions/deploy-pages|scripts/site-deploy' "$W" || { echo "    validate.yml does not deploy the site"; exit 1; }

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
dispatched=$(grep -oE '^  [a-z|]+\)$' "$ROOT/bin/majordomus" | tr -d ' )' | tr '|' '\n' | sort -u)
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
#    structure job runs scripts/ci/core-check on every plan, and that script runs them
grep -qE '^\s+run: scripts/ci/shell-lint$' "$W" || { echo "    validate.yml does not run scripts/ci/shell-lint"; exit 1; }
grep -qE '^\s+run: (MJ_CI_TIMINGS=[^ ]+ )?scripts/ci/core-check$' "$W" || { echo "    validate.yml does not run scripts/ci/core-check"; exit 1; }
for c in context history handover checkpoint decision question prompt; do
  grep -qE "majordomus $c" "$CORE" || { echo "    core-check never runs majordomus $c"; exit 1; }
done

# 6b. the canonical project model is a blocking gate, and the projection is proved offline.
#     A model that cannot be executed is not a warning, and a projection that only works on a
#     machine with a token is not a projection anyone can trust.
grep -qE '^bin/majordomus plan validate$' "$CORE" || { echo "    core-check does not run plan validate as a blocking step"; exit 1; }
grep -q 'scripts/github-sync --plan' "$CORE" || { echo "    core-check does not check the GitHub projection offline"; exit 1; }
grep -qE 'scripts/github-sync' "$LINT" || { echo "    shell-lint does not shellcheck the GitHub adapter"; exit 1; }
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
#    and the deploy job needs exactly the jobs that guard the site. The model is read here,
#    so a gate added to it without its job fails this case rather than pending forever.
PLAN="$ROOT/scripts/ci-plan"; MODEL="$ROOT/.ai/repo/ci/gates.yaml"
[ -x "$PLAN" ] && [ -f "$MODEL" ] || { echo "    scripts/ci-plan or .ai/repo/ci/gates.yaml is missing"; exit 1; }
expect_exit 0 "$PLAN" --check
grep -q 'scripts/ci-plan ' "$W" || { echo "    validate.yml does not run scripts/ci-plan"; exit 1; }
grep -q 'scripts/ci/verdict ' "$W" || { echo "    validate.yml does not run scripts/ci/verdict"; exit 1; }
"$PLAN" --full "probe" > full.json
jobs="$(grep -oE '^  [a-z]+:$' "$W" | tr -d ' :')"
for j in $(jq -r '.gates[].job' full.json | sort -u); do
  printf '%s\n' "$jobs" | grep -qx "$j" || { echo "    the model names job $j and validate.yml has no such job"; exit 1; }
done
for g in $(jq -r '.gates[] | select(.job != "structure") | .id' full.json); do
  job="$(jq -r --arg g "$g" '.gates[] | select(.id == $g) | .job' full.json)"
  awk -v j="  $job:" '$0 == j {f=1; next} /^  [a-z]+:$/ {f=0} f' "$W" | grep -qE "needs\.plan\.outputs\.$(printf '%s' "$g" | tr '-' '_') == 'true'" \
    || { echo "    job $job is not gated on the plan's output for $g"; exit 1; }
done
ci_needs="$(awk '$0 == "  ci:" {f=1; next} /^  [a-z]+:$/ {f=0} f' "$W" | sed -n 's/^    needs: \[\(.*\)\]$/\1/p' | tr -d ' ' | tr ',' '\n')"
for j in plan $(jq -r '.gates[].job' full.json | sort -u); do
  printf '%s\n' "$ci_needs" | grep -qx "$j" || { echo "    the ci job does not need job $j; a red $j could not turn the required status red"; exit 1; }
done
awk '$0 == "  ci:" {f=1; next} /^  [a-z]+:$/ {f=0} f' "$W" | grep -q '^    if: always()$' || { echo "    the ci job does not always run; a skipped job would leave the required status pending"; exit 1; }
pages_needs="$(awk '$0 == "  pages:" {f=1; next} /^  [a-z]+:$/ {f=0} f' "$W" | sed -n 's/^    needs: \[\(.*\)\]$/\1/p' | tr -d ' ')"
[ "$pages_needs" = "plan,structure,suite,rust,site" ] || { echo "    the pages job needs '$pages_needs', not the jobs that guard the site (plan,structure,suite,rust,site)"; exit 1; }
# a pull request's runs are superseded by the next commit; master's never are
grep -q "cancel-in-progress: \${{ github.event_name == 'pull_request' }}" "$W" || { echo "    validate.yml does not cancel superseded pull-request runs only"; exit 1; }
# no bare push trigger: a branch with a pull request is validated once, as that pull request
awk '/^on:/{f=1} /^permissions:/{f=0} f' "$W" | grep -A1 '^  push:' | grep -q 'branches: \[master\]' || { echo "    validate.yml runs on every push rather than on master and pull requests"; exit 1; }
