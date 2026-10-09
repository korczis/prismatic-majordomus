# A finish asks the intent engine what stands for the work the task named — the required criteria its issue serves and the guards of the intents it serves — reports it in every mode, refuses `completed` on it only where the policy holds it, and writes nothing about any intent

## What it means

A task that was started with `--issue` or `--intent` says what it serves. When it finishes, the completion report asks the intent engine two things about that work: whether every required criterion the task's issue serves has current evidence, and whether a guard of an intent it serves is violated. The answer names each criterion with the state of its evidence, the run that judged it and the command that reproduces it.

What happens next is the policy's. With `intent.completion: advisory` the answer is reported and withheld: `majordomus check` prints it, the question carries the status it would have had, and nothing is refused. With `required`, and `verification.completed_means_complete: true`, a served criterion without current evidence or a violated guard refuses `finish --outcome completed`. A partial finish is always available, and the refusal says so.

## How it works

`share/completion.yaml` declares the two questions, `criteria-served` and `guards-hold`, with the source `intent:criteria` and `intent:guards`. `gates.completion` answers them by executing `intents.binding` for the issue and the intent the active task names — never for its paths — and carries the engine's own state for each criterion and guard. `apps/majordomus-cli/src/gates/done.rs` translates that state once: failing or unresolved evidence fails, stale is stale, never run is owed, evidence the ledger cannot derive is unknown, and the worst decides. It computes no criterion, guard or verdict of its own.

A binding that cannot be executed or read, or that is refused before it reaches an intent, is unknown in every mode and never an exemption. A task that names nothing, one started under an exemption and maintenance are exempt; a task that names only an intent is held to that intent's guards and to no criterion. ADR 0115 records the decision and the measurement behind the default.

## How to see it

```bash
majordomus check                                   # prints each withheld answer under advisory
```

<!-- majordomus:unrun needs an active task that names an issue; the report is the capability's answer for that task -->
```bash
majordomus-cli run gates.completion --input '{}' --format json
```

## What it does not cover

A finish records no evidence: the run that produced it does, with `majordomus evidence record` and a stamp, and the remediation names those steps. A repository with no CI model is asked none of this, because the completion gates are not judged there at all. A criterion whose evidence is a command or a deployment reads unknown, so under `required` the issues serving it cannot be finished `completed` until that evidence can be derived. Whether the review of the plan is current is asked at `start`, not here. Nothing here decides whether an intent is satisfied or when an issue is done.

## Why it exists

A task could start bound to the criterion it exists to make true and finish `completed` with that criterion never run. The completion policy knew how to refuse and the intent engine knew what stood; this joins them without letting completion decide anything the engine owns.
