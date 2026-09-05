# A change runs the gates its paths can affect, decided by one model of what affects what, and a change to the pipeline itself or a path the model does not know runs every gate

## What it means

A pull request does not run every gate because a gate exists; it runs the gates a change under its paths can reach, and the decision is not a guess. `.ai/repo/ci/gates.yaml` declares the gates and the path classes with the gates each class can affect, and `scripts/ci-plan` computes the selection from that model and the changed paths. The always gates (the shell lint, the structural checks) are in every plan. A path that no class knows, or a path in the pipeline itself, escalates to the full plan, and the plan names the path that did.

## How it works

The planner flattens the model, matches every changed path against every class pattern (`**` crosses directories, `*` does not), takes the union of the matched classes' gates, adds what a selected gate implies and requires, and writes a plan: the mode, the reason, every gate with its job and whether and why it was selected. The `plan` job of `.github/workflows/validate.yml` runs it against the pull request's diff from the base it will merge into and turns the plan into job outputs the other jobs are gated on; the workflow carries no path list of its own. `test/cases/94_ci_plan.sh` drives the planner with listed paths and asserts the relationships that matter: Rust, site, documents, the distribution, the pipeline, an unknown path, an inert path, the union, the implied and required gates, and that a model with a dangling reference is refused.

## How to see it

```bash
just ci-plan                                   # this working tree against master
printf 'docs/DESIGN.md\n' | scripts/ci-plan --files - --format text
printf 'apps/majordomus-cli/src/lib.rs\n' | scripts/ci-plan --files - | jq .selected
scripts/ci-plan --check                        # the model resolves
just test-shell 94_ci_plan
```

## What it does not cover

The model is a statement about what reads what, written by people and checked by the cases that name representative paths; a new consumer of a path that nobody added to the model is a gap the full plan on master closes after the merge, not before. The planner decides which gates run, never how they pass: each gate is its own script with its own evidence.

## Why it exists

Before the model, every push and every pull request ran two operating-system matrices of every gate over every commit, and a separate Pages workflow ran the suite and the browser probe again over the same commit; a documentation change waited for the Rust coverage build and a Rust change for the browser. The measurements are in `.ai/repo/ci/baseline.json` (`scripts/ci-baseline --table`). Majordomus prefers data-driven behaviour, so the dependency map lives once, as data the tests read, rather than as path filters scattered through workflow files, which are also what leaves a required check pending forever when a filtered workflow never starts.
