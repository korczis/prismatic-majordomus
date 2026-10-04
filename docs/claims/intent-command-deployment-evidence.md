# A criterion settled by a command or by a deployment will be met by recorded evidence

## What it means

**This is not implemented.** It is published so that a known gap is visible rather than assumed to be covered.

A criterion may name four kinds of evidence: a `test`, a `claim`, a `command` or a `deployment`. The intended shape is that a recorded run of the command, or a verified state of the deployment, meets the criterion the way a passing test run does.

## How it works today

A `command` criterion and a `deployment` criterion whose deployment exists both resolve, so `intent validate` accepts them, and both read `not_derivable`: the evidence ledger records runs of tests and claims only, so nothing can meet them. An intent whose criteria include one of these cannot be satisfied today.

## How to see it

```bash
majordomus intent show <id> --format json   # the criterion's state is not_derivable
```

## What it does not cover

Recording the run of a command would say that it ran and what it returned, not that the outcome it was chosen to witness is true.

## Why it exists

Not every outcome has a test: some are a deployment answering, some a command exiting zero. Saying they are never met, rather than quietly treating them as met, keeps the intent's stage honest until the ledger can record them.
