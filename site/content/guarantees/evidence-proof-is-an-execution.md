+++
title = "A claim's proof is an execution that was recorded with the commit it ran against, not a test path that resolves"
description = "docs/CLAIMS.yaml binds a claim to a test with a path, and a path that resolves proves"
weight = 167
[extra]
claim_id = "evidence-proof-is-an-execution"
status = "guaranteed"
source = "docs/claims/evidence-proof-is-an-execution.md"
+++
{% raw %}

## What it means

`docs/CLAIMS.yaml` binds a claim to a test with a path, and a path that resolves proves
only that a file exists. It does not say the case ever ran, against which revision, with
what result, or whether the result still applies. Until this subsystem, `status:
guaranteed` beside `test: test/cases/84_distribution_model.sh` was rendered on the public
site as a guarantee on the strength of that file being there, and nothing in the repository
could be asked otherwise.

A proof here is an execution: one recorded run of one test, carrying the provenance the run
itself did not have — the outcome, the duration in whole seconds, the full commit it ran
against, whether that commit described the tree, the sha256 of the test's own source at the
time, when it was recorded, where the run happened, and the command that produces it again.
The repository keeps the latest execution of every test in a tracked ledger, and joining
that ledger with the matrix answers, per claim, what can honestly be said. A claim whose
test nobody has ever run reads `not_run` instead of inheriting a guarantee from a file's
existence.

## How it works

Three objects and one derivation, in `apps/majordomus-cli/src/evidence/`.

`TestId` is the stable identity of a test, computed from the path the claim already names.
`test/cases/84_distribution_model.sh` is the case `test/run.sh` knows as
`suite:84_distribution_model`; `apps/majordomus-cli/tests/why.rs` is the binary `cargo test
--test why` runs. Both spellings already existed in the repository, so `TestId::of` names
the join and nothing else: the matrix gained no field, no claim was migrated, and adding a
third runner is a variant and a branch in one function rather than a configuration file —
a runner nothing executes is a runner whose results nothing can record. A path no runner
drives, whether a template, a fixture, a library or a nested path under `test/cases/` the
runner does not enumerate, names no test, and the claim reads `unrunnable` rather than
being counted as covered. The matrix's own placeholder for nothing, `-`, quoted or not,
reads as `no_test`.

`Execution` is one recorded run and `Ledger` — `.ai/repo/evidence/ledger.json`, tracked
JSON, one entry per test — holds the latest of them. `evidence::report` joins the claims
the index holds with the executions the ledger holds, on every read, and decides one of
seven states per claim: `proven`, `inputs_unchanged`, `stale`, `failing`, `not_run`,
`unrunnable`, `no_test`. The join is never written down, so there is no second place for a
proof state to be stored and go stale, and none of the four capabilities caches it.

`evidence record` reads what the runs already wrote — the suite's TSV report under
`MJ_TEST_REPORT`, `cargo test`'s output — and stamps each result with the provenance. It
records; it decides nothing. Only a recognised pass proves anything, and a result word
nobody recognises is `error` rather than silently the good outcome; there is deliberately
no `not_run` outcome in the model, because that is the absence of an execution rather than
one a test had, and a recorder able to write it could have "this did not run" counted among
the things that did. A tree with no commit to name is refused, because a run recorded there
would carry no provenance. A malformed report line is refused rather than skipped: a
silently dropped case leaves a claim reading `not_run` after a run that did run it, which
is a lie in the safe direction and still a lie. A result naming a test this checkout does
not have is reported as unknown and recorded for nobody, because it means the runner and
the matrix have diverged.

The commit is taken when the result is recorded rather than by the runner, because the
runner executes each case in a disposable temporary repository and genuinely does not know
which checkout it was invoked from. Recording in the tree the run was made against takes
the same commit; a dirty tree is recorded as `dirty` for exactly this reason, since that is
the one case where the commit does not describe what ran.

## How to see it

```bash
majordomus evidence show                    # every claim, its state, the run behind it
majordomus evidence show --state not_run    # the claims nothing has ever run
majordomus evidence claim distribution-canonical-model
MJ_TEST_REPORT=suite.tsv bash test/run.sh
majordomus evidence record --suite suite.tsv --origin local
```

Every command takes `--format json`. A repository that has recorded nothing says so and
exits 0 — every claim reads `not run`, which is true — because a repository that cannot
report the absence of evidence would have to report nothing at all.

## What it does not cover

It is not tamper-proof, and it must not be described as one. The ledger is a tracked file;
a person can write `"outcome": "pass"` into it, and what a reviewer sees is the diff. The
recorded digest buys staleness detection that survives a revert, not forgery resistance:
an execution whose test source no longer hashes to what was recorded did not run the test
that is there now, whatever the commit says. Cryptographic ceremony against someone who can
already commit to this repository would be ceremony with no threat model.

It does not judge results. Nothing re-runs a test, re-interprets an outcome or decides that
a failure was environmental, and nothing here can make a claim true: a `proven` state says
the test passed against this tree, and whether the test tests the claim is a question for
review, held by the rule `project.no-claim-without-test`.

It keeps no history and no captured output. The ledger answers *is this claim proven now*,
which only ever reads the newest run; the record of what happened over time is git, over
this file, and the megabytes of a run's log belong where the run happened.

Nothing in CI records into it today. The suite job already writes the TSV and uploads it,
but landing CI's evidence in the repository needs a commit a pull-request run must not
make, so the ledger is written by whoever runs the suite locally and commits it. The states
are honest either way: an unrecorded run is an unrecorded run.

## Why it exists

A reference check and a proof are different questions, and this repository had only the
first. `scripts/generate-site-data --check` verified that every `source`, `implementation`
and `test` path resolved, which made a broken link impossible and a hollow guarantee
undetectable — the failure the rule `project.never-reported-is-not-green` names one level
up. Publishing a sentence as guaranteed because a file of the right name exists is the
green badge whose derivation cannot be inspected, and it is the one thing a repository
whose whole argument is evidence over claims cannot afford to ship.
{% endraw %}
