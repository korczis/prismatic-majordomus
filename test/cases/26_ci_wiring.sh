# The protection is only real if something runs it. An excellent test that CI never invokes
# is worth nothing, so this case checks the harness and the workflows themselves.
#
# It reads the repository's own files rather than the disposable fixture, because the subject
# is this repository's CI configuration.
. "$ROOT/test/lib.sh"
W="$ROOT/.github/workflows/validate.yml"
P="$ROOT/.github/workflows/pages.yml"
[ -f "$W" ] || { echo "    no validate workflow"; exit 1; }

# 1. one canonical validation command, and CI calls exactly that
grep -q 'bash test/run.sh' "$W" || { echo "    validate.yml does not run bash test/run.sh"; exit 1; }
grep -q 'bash test/run.sh' "$P" || { echo "    pages.yml does not run bash test/run.sh"; exit 1; }

# 2. the runner discovers every case rather than naming a list that can fall behind
grep -q 'test/cases/\*\.sh' "$ROOT/test/run.sh" || { echo "    run.sh does not glob test/cases"; exit 1; }

# 3. the runner's exit status reflects failure, and the workflow does not swallow it
grep -q '^\[ "$fail" = 0 \]' "$ROOT/test/run.sh" || { echo "    run.sh does not exit non-zero on failure"; exit 1; }
for f in "$W" "$P"; do
  # a required step must not end in a construct that discards the exit code
  if grep -nE '^\s+run:.*\|\|\s*true' "$f"; then echo "    ${f##*/}: a run step swallows failure with || true"; exit 1; fi
  if grep -nE 'continue-on-error:\s*true' "$f"; then echo "    ${f##*/}: a step is allowed to fail"; exit 1; fi
  if grep -nE '^\s+set \+e' "$f"; then echo "    ${f##*/}: a step disables errexit"; exit 1; fi
done

# 4. the runner actually fails when a case fails. This is the mutation: a case that cannot
#    pass must turn the whole run red, or every other assertion in this suite is decoration.
tmpcase="$ROOT/test/cases/zz_ci_wiring_probe.sh"
cleanup() { rm -f "$tmpcase"; }
trap cleanup EXIT
printf '# deliberately failing probe, created and removed by 26_ci_wiring\nexit 1\n' > "$tmpcase"
if ( cd "$ROOT" && bash test/run.sh zz_ci_wiring_probe >/dev/null 2>&1 ); then
  echo "    test/run.sh reported success while a case failed"; exit 1
fi
# ... and passes when it passes, so the check above is not vacuous
printf '# probe, created and removed by 26_ci_wiring\nexit 0\n' > "$tmpcase"
( cd "$ROOT" && bash test/run.sh zz_ci_wiring_probe >/dev/null 2>&1 ) || {
  echo "    test/run.sh reported failure while a case passed"; exit 1; }
cleanup; trap - EXIT

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

# 6. the continuity commands are reached by CI in this checkout, not only in fixtures
for c in context history handover checkpoint decision question prompt; do
  grep -qE "majordomus $c" "$W" || { echo "    validate.yml never runs majordomus $c"; exit 1; }
done

# 7. shellcheck covers the tests as well as the tool: an unchecked test is an unchecked gate
grep -q 'shellcheck.*test/cases/\*\.sh' "$W" || { echo "    validate.yml does not shellcheck test/cases"; exit 1; }

# 8. the enforcement the policy declares is the enforcement the hooks run. doctor proves this
#    for the repository; here we prove doctor itself is invoked by both hooks and by CI.
grep -q 'majordomus doctor' "$W" || { echo "    validate.yml does not run doctor"; exit 1; }
grep -q 'majordomus watch' "$W" || { echo "    validate.yml does not run watch"; exit 1; }
grep -q 'majordomus doctor' "$ROOT/.githooks/pre-commit" || { echo "    pre-commit does not run doctor"; exit 1; }
grep -q 'majordomus finish --check' "$ROOT/.githooks/pre-push" || { echo "    pre-push does not run finish --check"; exit 1; }
