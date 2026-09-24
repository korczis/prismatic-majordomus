# majordomus-covers: none
# The suite's bound is declared once and measured every run.
#
# Two failures this case exists for, both observed on this repository:
#
#   1. The bound outgrew its justification in silence. 150 minutes was set from a 93-100
#      minute cost; by 2026-09-20 the suite cost 148.3 at its worst, leaving 1.7 minutes of
#      margin. Nothing measured the drift, because a job killed at its bound is reported
#      `cancelled` — the same word GitHub uses for a run somebody superseded. Master's suite
#      went days without a verdict and it read as noise.
#
#   2. A number declared twice drifts. The workflow's `timeout-minutes` and the model in
#      .ai/repo/ci/suite.yaml are the same fact; if a reader may edit either, they will
#      disagree, and the one nobody reads is the one that is wrong.
#
#   3. The same drift one level down. test/run.sh bounds every case at 2400 s unless the
#      case declares otherwise; by 2026-09-24 the slowest case cost up to 2416 s, and this
#      pull request's own run failed on a case killed at that bound — while the per-case
#      budget of 2700 s sat above it, a budget no case could reach before being killed.
#
# So: each bound is declared once and its two readers agree, a budget sits below the bound
# it warns about, and scripts/ci/suite-budget must refuse a run that costs more than the
# model allows — and must refuse to call an unmeasurable run a pass.
. "$ROOT/test/lib.sh"

W="$ROOT/.github/workflows/validate.yml"
M="$ROOT/.ai/repo/ci/suite.yaml"
B="$ROOT/scripts/ci/suite-budget"

[ -f "$M" ] || { echo "    .ai/repo/ci/suite.yaml is missing; the suite's cost is declared there"; exit 1; }
[ -x "$B" ] || { echo "    scripts/ci/suite-budget is missing or not executable"; exit 1; }

# ---------------------------------------------------------------- one declaration, two readers
bound_model="$(sed -n 's/^bound_minutes: *\([0-9][0-9]*\).*/\1/p' "$M" | head -1)"
[ -n "$bound_model" ] || { echo "    .ai/repo/ci/suite.yaml declares no bound_minutes"; exit 1; }
bound_wf="$(awk '$0 == "  suite:" {f=1; next} /^  [a-z-]+:$/ {f=0} f && /^    timeout-minutes: / {print $2; exit}' "$W")"
[ -n "$bound_wf" ] || { echo "    the suite job of validate.yml declares no timeout-minutes"; exit 1; }
[ "$bound_wf" = "$bound_model" ] || {
  echo "    the suite job is bounded at $bound_wf minutes and .ai/repo/ci/suite.yaml says $bound_model — one fact, two readers, and they disagree"
  exit 1
}

# the workflow runs the budget check over the report the suite writes
grep -q 'scripts/ci/suite-budget' "$W" || {
  echo "    the suite job does not run scripts/ci/suite-budget; a budget nothing runs is a number in a file"
  exit 1
}
awk '$0 == "  suite:" {f=1; next} /^  [a-z-]+:$/ {f=0} f' "$W" | grep -q 'suite-budget' || {
  echo "    scripts/ci/suite-budget is named in validate.yml but not inside the suite job"
  exit 1
}

# ---------------------------------------------------------------- the budget refuses growth
budget_total="$(sed -n 's/^ *cases_seconds: *\([0-9][0-9]*\).*/\1/p' "$M" | head -1)"
budget_slowest="$(sed -n 's/^ *slowest_case_seconds: *\([0-9][0-9]*\).*/\1/p' "$M" | head -1)"
[ -n "$budget_total" ] && [ -n "$budget_slowest" ] || {
  echo "    .ai/repo/ci/suite.yaml declares no budget.cases_seconds and budget.slowest_case_seconds"
  exit 1
}

# ---------------------------------------------------------------- the bound of one case
# the default test/run.sh gives a case that declares no `majordomus-timeout:` of its own is
# the model's case_bound_seconds — one fact, two readers
case_bound_model="$(sed -n 's/^case_bound_seconds: *\([0-9][0-9]*\).*/\1/p' "$M" | head -1)"
[ -n "$case_bound_model" ] || { echo "    .ai/repo/ci/suite.yaml declares no case_bound_seconds"; exit 1; }
case_bound_runner="$(sed -n 's/.*\${MJ_TEST_CASE_TIMEOUT:-\([0-9][0-9]*\)}.*/\1/p' "$ROOT/test/run.sh" | head -1)"
[ -n "$case_bound_runner" ] || { echo "    test/run.sh gives MJ_TEST_CASE_TIMEOUT no numeric default"; exit 1; }
[ "$case_bound_runner" = "$case_bound_model" ] || {
  echo "    test/run.sh bounds a case at $case_bound_runner s and .ai/repo/ci/suite.yaml says $case_bound_model — one fact, two readers, and they disagree"
  exit 1
}
# and the per-case budget sits below it: a case that grows has to be refused by the budget,
# by name, while the bound still has room — a budget above the bound is one no case can
# reach, because the runner kills the case first and reports a cost it never measured
[ "$budget_slowest" -lt "$case_bound_model" ] || {
  echo "    budget.slowest_case_seconds is $budget_slowest s, not below the $case_bound_model s bound a case is killed at — the budget could never fire first"
  exit 1
}

# a run within budget passes, and says what it measured
printf 'a\tok\t10\tparallel\nb\tok\t20\tparallel\n' > "$T/within.tsv"
out="$(cd "$ROOT" && ./scripts/ci/suite-budget --report "$T/within.tsv" 2>&1)" || {
  echo "    a run well inside the budget was refused: $out"
  exit 1
}
printf '%s' "$out" | grep -q 'within budget' || {
  echo "    a run inside the budget did not say so: $out"
  exit 1
}

# a total over budget is refused, naming the slowest cases so a reader knows where it went
# `st=$?` after an `if` block reads the *if statement's* status, which is always 0; the
# command's own status has to be captured where it runs
over=$((budget_total + 1))
printf 'fat\tok\t%s\tparallel\n' "$over" > "$T/over.tsv"
st=0
(cd "$ROOT" && ./scripts/ci/suite-budget --report "$T/over.tsv" > "$T/over.out" 2>&1) || st=$?
[ "$st" = 10 ] || { echo "    a suite costing ${over}s against a ${budget_total}s budget exited $st, expected 10"; cat "$T/over.out"; exit 1; }
grep -q "$budget_total" "$T/over.out" || { echo "    the refusal does not name the budget it measured against"; cat "$T/over.out"; exit 1; }
grep -q 'fat' "$T/over.out" || { echo "    the refusal does not name the case that cost the most"; cat "$T/over.out"; exit 1; }

# one case over its own budget is refused even when the total is comfortable
slow=$((budget_slowest + 1))
printf 'runaway\tok\t%s\tparallel\n' "$slow" > "$T/slow.tsv"
st=0
(cd "$ROOT" && ./scripts/ci/suite-budget --report "$T/slow.tsv" > "$T/slow.out" 2>&1) || st=$?
[ "$st" = 10 ] || { echo "    a case costing ${slow}s against a ${budget_slowest}s per-case budget exited $st, expected 10"; cat "$T/slow.out"; exit 1; }
grep -q 'runaway' "$T/slow.out" || { echo "    the per-case refusal does not name the case"; cat "$T/slow.out"; exit 1; }

# ---------------------------------------------------------------- an unmeasured run is not a pass
st=0
(cd "$ROOT" && ./scripts/ci/suite-budget --report "$T/absent.tsv" > "$T/absent.out" 2>&1) || st=$?
[ "$st" = 12 ] || { echo "    a missing report exited $st, expected 12 — 'I did not look' is not 'it was fine'"; exit 1; }

printf 'x\tok\tnotanumber\tparallel\n' > "$T/empty.tsv"
st=0
(cd "$ROOT" && ./scripts/ci/suite-budget --report "$T/empty.tsv" > "$T/empty.out" 2>&1) || st=$?
[ "$st" = 12 ] || { echo "    a report with no usable case rows exited $st, expected 12"; exit 1; }

echo "    bound ${bound_wf}m and ${case_bound_model}s per case declared once; budget ${budget_total}s total, ${budget_slowest}s per case, refused in both directions"
