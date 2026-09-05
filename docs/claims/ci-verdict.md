# The one status a branch rule requires is green only when planning succeeded and every gate the plan selected ran in a job that succeeded; a failed or cancelled job, a selected gate whose job was skipped, or a plan that selected nothing is red

## What it means

There is one status to require, `ci`, and it cannot be green by accident. It is computed from two facts: the plan (which gates this change had to run) and what the jobs reported. A gate the plan selected must have run in a job that succeeded; a job that failed or was cancelled is red whether or not its gate was planned; a plan that selected nothing is a broken plan and red; a failed planning job is red. A gate the plan left out is allowed to be skipped, and the table says why.

## How it works

`scripts/ci/verdict` takes the plan (`plan.json`, the `ci-plan` artifact) and the workflow's `needs` context (every job's result) and decides, writing a table of every gate, its job, whether it was planned and why, and the job's result, to the step summary. The `ci` job of `.github/workflows/validate.yml` needs every job and runs with `if: always()`, so a skipped job cannot leave the status pending. `test/cases/94_ci_plan.sh` drives the verdict with fixture plans and needs contexts through every red condition and the green one; `test/cases/26_ci_wiring.sh` proves the `ci` job needs every job of the model and always runs.

## How to see it

```bash
scripts/ci-plan --full x > plan.json
printf '{"plan":{"result":"success"},"structure":{"result":"success"},"suite":{"result":"failure"}}' > needs.json
scripts/ci/verdict --plan plan.json --needs needs.json; echo "exit $?"
just test-shell 94_ci_plan
```

## What it does not cover

The verdict trusts the jobs' results as GitHub reports them; it does not rerun a gate. A gate the model does not declare is not in any plan and not in any verdict. Branch protection is a repository setting, not a file in this tree: the status exists to be required, and whether it is required is decided on GitHub.

## Why it exists

A workflow that is skipped by a path filter never reports, and a required check that never reports leaves a pull request pending forever; a workflow that always reports green when nothing ran is worse. Majordomus' philosophy is evidence over claims: the status names what ran and why, and refuses to summarise nothing as a pass.
