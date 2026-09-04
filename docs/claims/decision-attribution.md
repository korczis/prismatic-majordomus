# A decision record without a task, a head and a reason is reported

## What it means

`.majordomus/state/decisions.md` is where a worker externalises a decision so the next one does not reopen it. An entry that does not say which task made it, at which commit, and why, cannot be found by the worker who needs it. That is reported as a warning; it does not stop a command.

## How it works

`mj_decision_malformed` in `lib/decision.sh` returns the line numbers of entries missing `Task`, `Head` or `Why`. `mj_validate_decisions` reports them. The doctrine `decision_records` is declared **advisory**, which is what makes the finding a `WARN` and leaves the exit code at 0 — the class is read by `mj_doctrine_fail` at dispatch time, so the level is not a choice made inside the validator.

The file is deliberately hand-editable: `majordomus decision` writes well-formed entries, and a person editing it directly is a supported way to work. `finish` is where it stops being advisory — a profile that requires a decision record refuses completion when it cannot find one for the task.

## How to see it

```bash
printf '\n## 2026-01-01 — hand written, no fields\n' >> .majordomus/state/decisions.md
majordomus check                  # WARN records decisions.md — entry at line(s) 12 lacks Task, Head or Why
echo $?                           # 0
```

## What it does not cover

The check reads structure, not judgement. An entry carrying all three fields and a reason of "because" passes. Nothing verifies that the rejected alternatives were real, or that the decision was followed.

## Why it exists

The advisory class exists for exactly this shape of rule: the cost of a malformed entry is paid later and by someone else, and blocking a commit over the formatting of a note somebody wrote by hand would teach people to stop writing them.
