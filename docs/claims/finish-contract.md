# finish evaluates the finish contract line by line and refuses when any line is unmet

## What it means

"Done" is a checklist, written in the policy before work starts, evaluated by `majordomus finish`. For `completed` the default lines are: touched files within scope; the verification command ran and exited zero; the task record is at or behind `HEAD`, not diverged; no unresolved open question for this task; a handover or completion note with the required sections exists. Every line is printed as pass or fail. If any fails, nothing is written and the exit code is 10.

## How it works

`lib/finish.sh` reads `verification.finish_requires` from the policy and evaluates each named line, running `--verify-command` through `sh -c` in the repository root and recording its command, exit code and duration. Profile requirements are added for `completed`: a regression test path when the profile requires one (a path heuristic that says so), a decision record when required. On success it sets the outcome, appends `task.finished` with the evaluated contract and the verification result to the ledger, and copies a `--note` file under `state/completed/`. `--check` evaluates scope and state without writing, exits 0 with no active task, and is what a pre-push hook runs.

## How to see it

```bash
majordomus finish --outcome completed --verify-command "make test"
# OK   scope         t-… — 12 touched file(s), all within scope
# OK   verification  t-… — make test — exit 0, 41s
# OK   state         t-… — advanced (head 9b1e2d4)
# FAIL blockers      t-… — unresolved entry in open-questions.md  [reproduce: grep -n 'unresolved' .ai/local/state/open-questions.md]
# OK   note          t-… — 20260903T201455Z--main--9b1e2d4--c0ffee.md
# finish: refused, 1 unmet
```

## What it does not cover

It runs the verification you give it and does not decide which tests matter. It does not review code. A refused finish is recorded nowhere by design in v0.1; recording refusals is under review as a contract change.

## Why it exists

In the source environment "done" was a word in a transcript; status fields held free text like "mission accomplished", and a mandatory audit trail had no gate. A contract evaluated by a command that refuses is the difference between a claim and a fact.
