# A doctrine's class decides whether a violation stops the command

## What it means

There are two classes. `blocking` means a violation ends the command with a non-zero exit; `advisory` means the violation is reported and the command still succeeds. The class is read at dispatch time and is what routes the finding, so it is a mechanism rather than a label.

## How it works

A validator never calls `mj_fail` directly. It calls `mj_doctrine_fail`, which reads `MJ_DOCTRINE_CLASS` — set by the dispatcher from the registry — and emits `FAIL` for a blocking doctrine and `WARN` for an advisory one. Only `FAIL` and `DRIFT` increment the counter the command turns into its exit code. An unknown class is neither: it is a configuration error, reported as such, and it fails closed rather than defaulting to advisory.

`watch` is the one command that overrides this, and deliberately. It never blocks work — it reports what has moved and exits 11 to say it found something — so under `watch` every deviation is drift, advisory ones included.

## How to see it

```bash
majordomus check                       # WARN checkpoint  … 40m ago, interval 15m   → exit 0
# change class: advisory to class: blocking for checkpoint_freshness
majordomus check                       # FAIL checkpoint  … 40m ago, interval 15m   → exit 10
```

`test/cases/17_doctrine_enforcement.sh` performs exactly that edit in a throwaway copy of the tool and asserts the exit code moves.

## What it does not cover

The class says what a violation costs, not how serious the underlying problem is. There is no severity ladder, and adding one would mean adding a level that neither blocks nor is worth reporting.

## Why it exists

Enforcement systems drift toward a middle setting where a rule is nominally enforced and nothing ever fails. Two classes make that impossible to express: a rule either stops the work or it does not, and the registry says which in one word that the code reads.
