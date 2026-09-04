# An issue cannot be completed while a required piece of evidence is missing

## What it means

Each issue declares `evidence_required`: tokens naming the proofs its completion depends on. `majordomus plan evidence` attaches one piece of evidence against one of those tokens, and refuses a token the issue does not declare. `majordomus plan done` refuses while any token is uncovered. An issue whose `completed_at` is set but whose evidence is incomplete derives `VERIFY`, never `DONE`.

## How it works

`mj_plan_evidence` appends the evidence to the issue's own file, with the commit and the timestamp, and refuses without either a `--command` or an `--artifact`: narrative is not accepted as evidence. `mj_plan_transition` checks every declared token before writing `completed_at`. `lib/project.awk` recomputes coverage on every read, so removing a piece of evidence moves the issue back to `VERIFY` without anybody editing a status.

## How to see it

```bash
majordomus plan done I0001
# majordomus: I0001 has no evidence for: proof (run: majordomus plan evidence I0001 --covers <token> ...)
majordomus plan evidence I0001 --covers proof --type test --command "bash test/run.sh" --result "32 passed"
majordomus plan done I0001
# plan: I0001 DONE
```

## What it does not cover

The tool records the command and what the worker said it produced. It does not rerun the command, and it cannot tell a true result from a typed one. The commit hash stored beside the evidence is what makes a false one checkable afterwards.

## Why it exists

Closing a ticket is not evidence that the work happened. The gate exists so that "done" costs a command.
