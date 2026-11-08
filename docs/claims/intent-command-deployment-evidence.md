# A criterion settled by a command or by a deployment will be met by recorded evidence

## What it means

**This is only half implemented.** It is published so that the remaining gap is visible rather than assumed to be covered.

A criterion may name four kinds of evidence: a `test`, a `claim`, a `command` or a `deployment`. The intended shape is that a recorded run of the command, or a verified state of the deployment, meets the criterion the way a passing test run does. The command half holds today; the deployment half does not.

## How it works today

A `command` criterion is judged from the evidence ledger, with no store of its own: its receipt is the latest recorded execution whose `command` is the criterion's `ref`, byte for byte. A pass of exactly that command at the presented revision, nothing but the ledger having changed since, reads `current`; a run of a different command (one argument more or fewer is different) or no run reads `not_run`, a failing run `failing`, and a pass followed by a change, or recorded at a commit this history lacks, `stale`.

A `deployment` criterion whose deployment exists resolves, so `intent validate` accepts it, and reads `not_derivable`: nothing can meet it yet. So does a command criterion whose `ref` is `majordomus intent explain <its own intent>`, a placeholder for a proof still planned, because `intent explain` exits 0 whatever it finds. An intent whose only unmet criteria are of these two forms reads the verdict `unknown`.

## How to see it

```bash
majordomus intent show <id> --format json   # a command criterion's state and proof; a deployment's not_derivable
```

## What it does not cover

A recorded run of a command says that exactly that command ran at that revision and what it returned, not that the outcome it was chosen to witness is true. A deployment's state is not recorded at all.

## Why it exists

Not every outcome has a test: some are a deployment answering, some a command exiting zero. Judging a command only by a receipt of exactly it, and saying a deployment is never met rather than quietly treating it as met, keeps the intent's stage honest.
