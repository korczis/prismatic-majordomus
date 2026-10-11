# RELEASE 15 · 06 The record lands by itself

## Operating contract
As in 00. The executor (`majordomus prs`) is the single merger of master; read the board for who
holds it. I2296 (b4) and I2282 are related and may be in flight.

## Objective
The record's pull request is no longer on the critical path (prompt 01), but it must land without
a person: within one run of its required check, no close/reopen by hand, no `--admin`.

## Issue 1: The record PR gets its own verdict
A PR opened with GITHUB_TOKEN starts no workflow, and a dispatched run's `ci` did not satisfy
branch protection for #866 (I2282). Close the gap with what the owner provides (a GitHub App or a
fine-grained token whose events start workflows, named in docs/DISTRIBUTION.md with its rotation)
or with the executor's reopen (I2296). Pick with evidence; record the choice.
Acceptance: the next release's record PR has a `pull_request` run within a minute of opening.

## Issue 2: The record's verdict is the record's
`scripts/ci-plan` selects the whole shell suite for a record (classes distribution, docs, layer,
site): 86 minutes. With the installer no longer waiting, this is a cost, not a delay: decide with
an impact proof (which cases read release records; prompt 00's data) whether a record-only change
may run a declared subset, and only if the proof covers every case a record can change.
Acceptance: an impact proof checked by a gate, or the decision to keep the full suite, recorded.

## Issue 3: The executor drains it
The executor recognises the record PR, ranks it first, and merges it on green through `prs drain`
with its usual proof. No hand step remains.
Acceptance: a release run end to end with no person after the tag, in the completion report.

## Completion report
Record PR open-to-merge minutes for the next release.
