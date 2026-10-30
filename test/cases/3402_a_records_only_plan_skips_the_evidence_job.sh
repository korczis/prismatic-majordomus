# majordomus-covers: none
# The evidence job runs only when the plan ran a lane that leaves a report (I3013). A plan of
# records alone builds the executable and runs the registry checks (rust_integration) and no
# test, so the job has nothing to record; when its condition keyed on the rust job's success it
# ran anyway and failed at evidence-collect's exit 12, and #1745, #1746 and #1749 were red for
# that alone (validate run 39483394648).
#
# The case reads the evidence job's `if` out of validate.yml, translates it into a jq
# expression and evaluates it against the needs of each plan:
#
#   records    shell_suite false, rust_check false, rust_integration true, rust success -> skip
#   crate      rust_check true, rust success or failure                                 -> run
#   suite      shell_suite true, suite failure, rust_check false                        -> run
#   cancelled  rust_check true, rust cancelled, suite skipped                           -> skip
#
# A condition the translation cannot read is a failure, never a pass: the old condition is
# evaluated too, and must still say "run" for the records plan, so the evaluator is shown to
# tell the two apart.
. "$ROOT/test/lib.sh"
command -v jq >/dev/null 2>&1 || skip "no jq"
WF="$ROOT/.github/workflows/validate.yml"

job_of() { awk -v j="  $1:" '$0 == j {f=1; next} f && /^  [a-z][a-z-]*:$/{exit} f' "$WF"; }
cond="$(job_of evidence | sed -n 's/^    if: //p' | head -n 1)"
[ -n "$cond" ] || { echo "    FAIL: validate.yml's evidence job has no if"; exit 1; }

# GitHub's expression, as jq: needs.<job>.result and needs.<job>.outputs.<name> become paths
# into the needs object, && and || become and/or, always() is true and strings are quoted
# with double quotes. Anything else left over is a construct this case does not know.
to_jq() {
  printf '%s\n' "$1" | sed \
    -e 's/always()/true/g' \
    -e 's/needs\.\([a-z_-]*\)\.outputs\.\([a-z_]*\)/.needs["\1"].outputs["\2"]/g' \
    -e 's/needs\.\([a-z_-]*\)\.result/.needs["\1"].result/g' \
    -e 's/&&/ and /g' -e 's/||/ or /g' -e "s/'/\"/g"
}
known() { # the translation holds only paths, comparisons, and/or, true and string literals
  local rest
  rest="$(printf '%s\n' "$1" | sed -e 's/\.needs\["[a-z_-]*"\]\.outputs\["[a-z_]*"\]//g' \
    -e 's/\.needs\["[a-z_-]*"\]\.result//g' -e 's/"[a-z]*"//g' \
    -e 's/==//g' -e 's/ and //g' -e 's/ or //g' -e 's/true//g' -e 's/[() ]//g')"
  [ -z "$rest" ]
}
expr="$(to_jq "$cond")"
known "$expr" || { echo "    FAIL: the evidence job's if holds a construct this case cannot evaluate: $cond"; exit 1; }

needs() { # shell_suite rust_check rust_integration suite-result rust-result
  jq -n --arg ss "$1" --arg rc "$2" --arg ri "$3" --arg s "$4" --arg r "$5" \
    '{needs: {plan: {result: "success", outputs: {shell_suite: $ss, rust_check: $rc, rust_integration: $ri}},
              suite: {result: $s}, rust: {result: $r}, coverage: {result: "skipped"}}}'
}
runs() { needs "$@" | jq -e "$expr" >/dev/null; }

check() { # want name needs...
  local want="$1" name="$2"; shift 2
  if runs "$@"; then got=run; else got=skip; fi
  [ "$got" = "$want" ] || { echo "    FAIL: the $name plan makes the evidence job $got, not $want"; echo "    if: $cond"; exit 1; }
}
check skip records   false false true skipped success
check run  crate     false true  false skipped success
check run  crate-red false true  false skipped failure
check run  suite     true  false false failure success
check skip cancelled false true  false skipped cancelled
echo "    ok: records-only and cancelled plans skip the evidence job; a suite or crate-test plan runs it, red or green"

# the evaluator tells the two conditions apart: the condition I3013 replaced ran on the records plan
old="always() && (needs.suite.result == 'success' || needs.suite.result == 'failure' || needs.rust.result == 'success' || needs.rust.result == 'failure')"
expr="$(to_jq "$old")"
known "$expr" || { echo "    FAIL: the evaluator cannot read the condition it replaced"; exit 1; }
runs false false true skipped success || { echo "    FAIL: the evaluator says the replaced condition skipped the records plan; it did not"; exit 1; }
echo "    ok: the replaced condition, evaluated alike, ran the records plan (the red of #1749)"
