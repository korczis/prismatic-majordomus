+++
title = "CI records every test it ran with the commit and the run it happened in, and the site publishes that evidence as current, stale with its distance, or unavailable with the reason"
description = "Every push to master runs the behavioural suite, the crate's tests and the coverage"
weight = 169
[extra]
claim_id = "evidence-recorded-in-ci"
status = "guaranteed"
source = "docs/claims/evidence-recorded-in-ci.md"
+++
{% raw %}

## What it means

Every push to master runs the behavioural suite, the crate's tests and the coverage
measurement. Those runs are the evidence for most of the matrix, and until this claim none of
them reached anything a reader could see. CI now keeps what it ran and the result of every
test as a recorded execution. Each one carries the commit it ran against and the run it
happened in: the run's identifier, its attempt, the workflow, the job, and the address the
provider gave.

The site publishes that evidence beside the claims, and says how it relates to the commit
the site was built from. Evidence of that very commit is **current**. Evidence of an older
commit is **stale**, with the number of commits and changed files in between. No evidence at
all is **unavailable**, with the reason. Evidence of an older commit is never shown as proof of
the one on the page.

## How it works

The `evidence` job of `validate.yml` reads the reports the suite, the crate and the coverage
jobs left behind and gives them to `scripts/ci/evidence-collect`. The collector records them
through `majordomus evidence record --origin ci`. For a `ci` origin inside a GitHub Actions
environment, that stamps every execution with a `run` read by `RunRef::from_env`. A local
recording names no run, even inside a CI shell. The collector then derives the report through
`majordomus evidence show`, summarises the coverage through `scripts/rust-coverage
--summary-json`, and writes a manifest. The result is kept as the artifact `evidence`.

Before each build, `pages.yml` runs `scripts/pages evidence`. It finds the artifact recorded
against the commit being published or its nearest ancestor and writes
`site/data/evidence.json`, which is never committed. `/evidence/` renders it through the same
terminal the homepage replays runs in. When a master run finishes while its commit is still
the tip, it dispatches the Pages workflow again, and that publication says current.

The decision, and why the evidence is not committed into the tracked ledger, is ADR 68.

## How to see it

```bash
bash test/run.sh 357_ci_records_evidence
scripts/pages evidence --commit HEAD --out /tmp/evidence.json && jq 'del(.panels)' /tmp/evidence.json
```

The case records inside a described run and outside one, gathers a run with a failing test
and an absent report, and publishes gathered evidence against the published commit, against
its parent and against a commit outside its history.

## What it does not cover

The evidence is attached to claims, not yet to features, commands, MCP tools or rules, and
coverage is reported for the crate and its session domain, not per subject. The `evidence` job
does not gate a merge: a broken collector makes the site say unavailable, which is visible and
does not refuse anything. The shell tool's own code has no coverage measurement at all, and
nothing here pretends it has one.

## Why it exists

149 of 173 claims read `not_run` on a repository whose CI ran their tests on every push. A proof
that happens and is thrown away is, to a reader, the same as no proof.
{% endraw %}
