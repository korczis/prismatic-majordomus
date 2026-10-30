# majordomus-covers: none
# claims: none
# A step of the Pages workflow is bounded from the row it runs, and a run that closes in on
# the bound is named before the run that crosses it.
#
# On 2026-10-20 the `check` step of pages.yml still carried the 3 minutes set while the site
# was English only, where it took 167-171 s. The Czech copy doubled the pages it reads, run
# 39392245576 was killed at 192 s, publish was skipped and the site stayed old. Nothing tied
# the bound to what the row was measured at, and the budget it was measured against was a
# warning. Two halves hold it now:
#   - scripts/ci/step-bound-check --budgets: a step (or job) named after a warm row of
#     .ai/repo/ci/pages.yaml is bounded at no less than budget.bound.percent of that row;
#   - scripts/pages report: a row that spent more than budget.bound.near_percent of its
#     step's bound is named NEAR, which never changes the exit status.
. "$ROOT/test/lib.sh"

CHECK="$ROOT/scripts/ci/step-bound-check"
P="$ROOT/scripts/pages"
MODEL="$ROOT/.ai/repo/ci/pages.yaml"
WF="$ROOT/.github/workflows/pages.yml"
expect_file "$CHECK"
expect_file "$MODEL"
expect_file "$WF"
[ -x "$P" ] || { echo "    scripts/pages is missing or not executable"; exit 1; }

# --- the good shape: this repository's workflow against its own model
expect_exit 0 "$CHECK" --budgets "$MODEL" "$WF"
expect_grep 'every budgeted row of pages.yaml is bounded from its budget'

# --- the broken shape of run 39392245576: `check` back at 3 minutes
awk '/^      - name: check$/ { f = 1 } f && /^        timeout-minutes: / { sub(/[0-9]+/, "3"); f = 0 } { print }' "$WF" > three.yml
cmp -s "$WF" three.yml && { echo "    the mutation did not apply: pages.yml has no bounded check step"; exit 1; }
expect_exit 10 "$CHECK" --budgets "$MODEL" three.yml
expect_grep 'the step check runs the check row, and its bound 3m \(180s\) is under'

# --- a budgeted step with no bound of its own, and one bounded at its job's bound
cat > model.yaml <<'Y'
version: 1
budget:
  warm:
    check: 60
    controlled: 60
  bound:
    percent: 150
    near_percent: 50
Y
cat > unbounded.yml <<'Y'
jobs:
  deploy:
    timeout-minutes: 30
    steps:
      - name: check
        run: scripts/pages check
Y
expect_exit 10 "$CHECK" --budgets model.yaml unbounded.yml
expect_grep 'declares no timeout-minutes; it can be killed only by its job'
printf 'jobs:\n  deploy:\n    timeout-minutes: 30\n    steps:\n      - name: check\n        timeout-minutes: 30\n        run: scripts/pages check\n' > atjob.yml
expect_exit 10 "$CHECK" --budgets model.yaml atjob.yml
expect_grep 'is not smaller than the job bound'

# --- a model without the percent, and a workflow in which no row is bounded, are findings
grep -v 'percent' model.yaml > nopercent.yaml
printf 'jobs:\n  deploy:\n    timeout-minutes: 30\n    steps:\n      - name: check\n        timeout-minutes: 2\n        run: scripts/pages check\n' > ok.yml
expect_exit 0 "$CHECK" --budgets model.yaml ok.yml
expect_exit 10 "$CHECK" --budgets nopercent.yaml ok.yml
expect_grep 'declares no budget.bound.percent'
printf 'jobs:\n  deploy:\n    timeout-minutes: 30\n    steps:\n      - name: other\n        timeout-minutes: 2\n        run: true\n' > none.yml
expect_exit 10 "$CHECK" --budgets model.yaml none.yml
expect_grep 'no step or job of none.yml is named after a row'

# --- NEAR: 169 s against the old 3-minute bound is named, and the verdict does not move
printf 'pages\tcheck\t169\n' > near.tsv
expect_exit 0 env MJ_PAGES_WORKFLOW="$PWD/three.yml" GITHUB_STEP_SUMMARY="$PWD/near.md" "$P" report --advisory --timings near.tsv
expect_grep 'near=check 169s/180s'
expect_grep '^::warning title=Pages step near its bound'
expect_grep 'NEAR its step bound' near.md
# and under the bound the workflow carries now, the same 169 s is not near anything
expect_exit 0 env GITHUB_STEP_SUMMARY="$PWD/far.md" "$P" report --timings near.tsv
expect_grep 'near=none'
expect_no_grep '^::warning'

echo "    a Pages step is bounded from its budget, and a run near its bound is named before it is killed"
