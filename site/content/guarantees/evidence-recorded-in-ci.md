+++
title = "CI records every test it ran with the commit and the run it happened in, and the site publishes the tracked ledger's verdict at that commit as current only when CI's run of the published commit measured clean trees and nothing was absent, refused or failed; otherwise as stale with its distance or its failures, as unknown with its reasons, or as unavailable with the reason"
description = "Every push to master runs the behavioural suite, the crate's tests and the coverage"
weight = 172
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
provider gave. For a pull request, the commit is the merge commit that ran, and the head it was
built from is carried beside it.

What the site publishes beside the claims is the tracked ledger's verdict at the commit CI ran,
not a verdict CI's own executions decided. Those executions are a run record: they are counted,
and they decide nothing. The site then says whether CI's run confirms that verdict for the
commit the site was built from. It is **current** only when the run is of that very commit,
every job that ran tests measured a clean tree, the report was derived on a clean checkout, and
nothing was absent, refused or failed. A run of an older commit is **stale**, with the number
of commits and changed files in between, and so is a run of that commit that recorded failures.
A run of that commit that cannot confirm the report is **unknown**, with every reason. No
evidence at all is **unavailable**, with the reason. Nothing but a confirming run of the very
commit on the page is shown as current.

## How it works

The `evidence` job of `validate.yml` downloads the reports the suite, the crate and the
coverage jobs left, and the measurements the suite and the rust job took of the trees they ran
in, into the runner's temporary directory, and gives them to `scripts/ci/evidence-collect`.
Each measurement excludes by name the outputs its own run names, and nothing else.

The collector derives the report first: it measures its own checkout and runs `majordomus
evidence show` before anything is recorded, so the report is the tracked ledger's verdict at
that commit. Only then does it record the reports through `majordomus evidence record --origin
ci`. For a `ci` origin inside a GitHub Actions environment, that stamps every execution with a
`run` read by `RunRef::from_env`; a local recording names no run, even inside a CI shell. A
report whose job measured another commit is refused and never recorded, and a report that
yielded no execution is named as absent. The collector summarises the coverage through
`scripts/rust-coverage --summary-json`, and writes a manifest that carries the producers'
measurements, the working tree derived from them, the tree the report was derived in, and what
was absent or refused. The result is kept as the artifact `evidence`.

Before each build, `pages.yml` runs `scripts/pages evidence`. It finds the artifact recorded
against the commit being published or its nearest ancestor and writes
`site/data/evidence.json`, which is never committed. It publishes the report as it was derived
and reads the manifest only to decide the state: a reason can withhold current, and never
changes the report. `/evidence/` renders it through the same terminal the homepage replays runs
in, and lists every reason under the state. When a master run finishes while its commit is
still the tip, it dispatches the Pages workflow again, and that publication says current when
the run confirms the report.

The decision is ADR 68, as ADR 0087 amends it. A validating run never commits its rows, and a
trunk run's rows reach the tracked ledger only through ADR 0080's recording pull request.

## How to see it

```bash
bash test/run.sh 357_ci_records_evidence
scripts/pages evidence --commit HEAD --out /tmp/evidence.json && jq 'del(.panels)' /tmp/evidence.json
```

The case records inside a described run and outside one, and gathers runs in a fixture with a
claims matrix of its own. It shows that a claim only the run proved still reads `not_run` in the
gathered report, that the tree is the one the producing jobs measured, and that a report
measured at another commit is refused. It publishes gathered evidence against the published
commit, confirmed and not, against its parent and against a commit outside its history, and it
reads the checked-in `validate.yml` for the wiring.

## What it does not cover

The evidence is attached to claims, not yet to features, commands, MCP tools or rules, and
coverage is reported for the crate and its session domain, not per subject. The recorded rows
still carry the recorder's own tree stamp rather than the producers' measurement, which the
manifest keeps apart; stamping the measurement into each row is a later change. A trunk run's
rows are not yet committed to the tracked ledger, because the recording pull request is not
wired. The `evidence` job does not gate a merge: a broken collector makes the site say
unavailable, which is visible and does not refuse anything. The shell tool's own code has no
coverage measurement at all, and nothing here pretends it has one.

## Why it exists

Most claims read `not_run` on a repository whose CI ran their tests on every push. A proof that
happens and is thrown away is, to a reader, the same as no proof. And a proof published over the
run's own rows, from a checkout that measured nothing that ran, under a badge saying current, is
worse: it looks like the authority's verdict and is not.
{% endraw %}
