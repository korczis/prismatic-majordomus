# majordomus-covers: none
# A plan owes a test report only for the lanes it selected, and the evidence job says so rather
# than failing or vanishing (I3013, I3668). A plan of records alone builds the executable and
# runs the registry checks (rust_integration) and no test; when the job's condition keyed on the
# rust job's success it ran with nothing to record and failed at evidence-collect's exit 12, and
# #1745, #1746 and #1749 were red for that alone (validate run 39483394648). I3013 skipped the job
# there; I3668 runs it on every decided plan and tells the collector what the plan owed, so a
# records plan leaves a receipt saying not_required and why.
#
# The case reads validate.yml and checks two things:
#
#   1  the evidence job's `if`, translated into jq and evaluated against each plan, runs the job
#      whenever the plan decided and the run was not cancelled (records, crate, suite plans),
#      and not when the plan failed or the run was cancelled
#   2  the gather passes each producer's obligation from the plan output that selects it
#      (suite <- shell_suite, crate <- rust_check, coverage <- rust_coverage) with the result of
#      the job that produces it (suite, rust, coverage), and the receipt is kept on every run
#
# A condition the translation cannot read is a failure, never a pass.
. "$ROOT/test/lib.sh"
command -v jq >/dev/null 2>&1 || skip "no jq"
WF="$ROOT/.github/workflows/validate.yml"

job_of() { awk -v j="  $1:" '$0 == j {f=1; next} f && /^  [a-z][a-z-]*:$/{exit} f' "$WF"; }
EVIDENCE="$(job_of evidence)"
cond="$(printf '%s\n' "$EVIDENCE" | sed -n 's/^    if: //p' | head -n 1)"
[ -n "$cond" ] || { echo "    FAIL: validate.yml's evidence job has no if"; exit 1; }

# ---------------------------------------------------------------- 1. when the job runs
# GitHub's expression, as jq: the ${{ }} wrapper goes, needs.<job>.result and
# needs.<job>.outputs.<name> become paths into the needs object, !cancelled() reads the run's
# cancellation, && and || become and/or, always() is true and strings take double quotes.
to_jq() {
  printf '%s\n' "$1" | sed \
    -e 's/^\${{ *//' -e 's/ *}}$//' \
    -e 's/!cancelled()/(.cancelled | not)/g' \
    -e 's/always()/true/g' \
    -e 's/needs\.\([a-z_-]*\)\.outputs\.\([a-z_]*\)/.needs["\1"].outputs["\2"]/g' \
    -e 's/needs\.\([a-z_-]*\)\.result/.needs["\1"].result/g' \
    -e 's/&&/ and /g' -e 's/||/ or /g' -e "s/'/\"/g"
}
known() { # the translation holds only paths, comparisons, and/or/not, true and string literals
  local rest
  rest="$(printf '%s\n' "$1" | sed -e 's/\.needs\["[a-z_-]*"\]\.outputs\["[a-z_]*"\]//g' \
    -e 's/\.needs\["[a-z_-]*"\]\.result//g' -e 's/(\.cancelled | not)//g' -e 's/"[a-z]*"//g' \
    -e 's/==//g' -e 's/ and //g' -e 's/ or //g' -e 's/true//g' -e 's/[() ]//g')"
  [ -z "$rest" ]
}
expr="$(to_jq "$cond")"
known "$expr" || { echo "    FAIL: the evidence job's if holds a construct this case cannot evaluate: $cond"; exit 1; }

needs() { # plan-result cancelled shell_suite rust_check suite-result rust-result
  jq -n --arg p "$1" --argjson c "$2" --arg ss "$3" --arg rc "$4" --arg s "$5" --arg r "$6" \
    '{cancelled: $c,
      needs: {plan: {result: $p, outputs: {shell_suite: $ss, rust_check: $rc, rust_integration: "true"}},
              suite: {result: $s}, rust: {result: $r}, coverage: {result: "skipped"}}}'
}
check() { # want name needs...
  local want="$1" name="$2"; shift 2
  if needs "$@" | jq -e "$expr" >/dev/null; then got=run; else got=skip; fi
  [ "$got" = "$want" ] || { echo "    FAIL: the $name plan makes the evidence job $got, not $want"; echo "    if: $cond"; exit 1; }
}
check run  records     success false false false skipped success
check run  crate       success false false true  skipped success
check run  crate-red   success false false true  skipped failure
check run  suite       success false true  false failure success
check run  interrupted success false false true  skipped cancelled
check skip no-plan     failure false false false skipped skipped
check skip cancelled   success true  false true  skipped cancelled
echo "    ok: every decided plan runs the evidence job, records-only and interrupted included; a failed plan or a cancelled run does not"

# ---------------------------------------------------------------- 2. what the gather is told
env_of() { printf '%s\n' "$EVIDENCE" | sed -n "s/^      $1: //p" | head -n 1; }
for pair in "MJ_OWES_SUITE=needs.plan.outputs.shell_suite" "MJ_OWES_CRATE=needs.plan.outputs.rust_check" \
            "MJ_OWES_COVERAGE=needs.plan.outputs.rust_coverage" "MJ_RESULT_SUITE=needs.suite.result" \
            "MJ_RESULT_RUST=needs.rust.result" "MJ_RESULT_COVERAGE=needs.coverage.result"; do
  name="${pair%%=*}" want="\${{ ${pair#*=} }}"
  [ "$(env_of "$name")" = "$want" ] || { echo "    FAIL: the evidence job's $name is '$(env_of "$name")', not '$want'"; exit 1; }
done
for ob in 'suite=${MJ_OWES_SUITE:-false}:$MJ_RESULT_SUITE' 'crate=${MJ_OWES_CRATE:-false}:$MJ_RESULT_RUST' \
          'coverage=${MJ_OWES_COVERAGE:-false}:$MJ_RESULT_COVERAGE'; do
  printf '%s\n' "$EVIDENCE" | grep -qF -- "--obligation \"$ob\"" \
    || { echo "    FAIL: the gather does not pass --obligation \"$ob\""; exit 1; }
done
printf '%s\n' "$EVIDENCE" | grep -qF 'name: evidence-receipt' \
  || { echo "    FAIL: the receipt is not kept as an artifact"; exit 1; }
printf '%s\n' "$EVIDENCE" | grep -B3 'name: evidence-receipt' | grep -qF '!cancelled()' \
  || { echo "    FAIL: the receipt's upload does not run when the gather failed"; exit 1; }
echo "    ok: each producer's obligation comes from the plan output that selects it, with its own job's result, and the receipt is kept on every run"
