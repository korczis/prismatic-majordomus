---
schema: adr/v1
id: adr-0080
kind: adr
title: A CI run reaches the tracked ledger through a recording branch, never through the run that produced it
status: proposed
date: 2026-09-20
tags:
  - evidence
  - claims
  - ci
  - verification
related:
  - claim:evidence-proof-is-an-execution
  - claim:a-skipped-case-is-not-a-proof
  - rule:project.never-reported-is-not-green
  - file:docs/EVIDENCE.md
  - file:.ai/repo/evidence/ledger.json
  - file:.ai/repo/evidence-baseline.txt
  - file:scripts/evidence-check
  - file:.ai/repo/ci/gates.yaml
  - file:apps/majordomus-cli/src/evidence/record.rs
  - test:test/cases/124_evidence.sh
  - file:.ai/repo/adrs/0041-proof-is-a-recorded-execution-and-inputs-unchanged-is-not-proven.md
provenance:
  origin: authored
---

# 80. A CI run reaches the tracked ledger through a recording branch, never through the run that produced it

## Context

The evidence fabric (ADR 0041) decided that a claim's proof is a recorded execution. The
ledger at `.ai/repo/evidence/ledger.json` is tracked, `majordomus evidence show` joins it
with `docs/CLAIMS.yaml` on every read, and `scripts/evidence-check` ratchets the answer.

Measured on master at `8d37d011ef`: the matrix holds 176 claims, 161 of them guaranteed.
The link from claim to test is complete — `scripts/ci/claim-proof-check --strict` passes,
every guaranteed claim names a path a runner drives, and the claim id is named back inside
the test. The link from claim to a recorded execution is not: the ledger holds 6
executions, all `origin: local`, the newest from 2026-09-12, and **no claim in this
repository is `proven`**. 147 guaranteed claims read `not_run` and 14 read `stale`.

The gate is green over that because its ratchet counts `stale` as support and reports new
debt rather than refusing it — both deliberate, both documented in `scripts/evidence-check`
itself. The green is honest about what it measures and says much less than a reader takes
it for.

CI runs the whole suite on every merge to master. It produces exactly the artefact
`majordomus evidence record` consumes — the TSV under `MJ_TEST_REPORT` and `cargo test`'s
output — and then throws the provenance away. Nothing on master, and nothing on any open
branch, carries a mechanism by which a `ci`-origin execution reaches the tracked ledger.
The recording that exists today is a person running the suite locally and committing the
result, which does not survive its author's attention and re-stales on the next merge.

This decision is about that one missing edge. It is deliberately not about what `proven`
means, which ADR 0041 settled, nor about publishing a run's evidence as a page, which is
decided elsewhere and is a different surface with a different lifetime.

## Decision

**A run records; a separate, minimal change lands the recording. The run never commits.**

1. A workflow on the trunk — after a merge, and on a schedule so that a quiet week still
   produces a current ledger — runs the suite and the crate tests exactly as the validating
   workflow does, then `majordomus evidence record --origin ci` against the commit it ran.

2. That job's only output into the repository is `.ai/repo/evidence/ledger.json`. It opens
   a pull request from a branch named for the commit it recorded, whose diff is that one
   file, whose title says which commit was measured, and whose body carries the run's own
   URL. It does not push to the trunk, and it does not commit inside a validating run of
   somebody's pull request.

3. That pull request is subject to the same required checks as any other. Nothing about
   evidence gets an exception from the merge rules, because a ledger that could be written
   by a job nobody reviews is a ledger in which `"outcome": "pass"` costs nothing.

4. `--origin ci` is what distinguishes it. The model is already provider-neutral about
   where a run happened, and a reader can therefore tell a recording made by the machine
   that runs every case from one made by a person on a laptop.

5. The word the runner writes is load-bearing, and this decision depends on the claim
   `a-skipped-case-is-not-a-proof`: a case that declined to run is recorded as `skip` and
   never as `ok`. Automating the recording without that fix would have industrialised the
   defect — every environment-dependent case that cannot run on the runner would have
   entered the ledger as a proof, at the cadence of every merge.

## Consequences

A merge to the trunk is followed by a small, reviewable pull request that moves claims from
`not_run` to `proven`. The repository can then say, for the first time, that a named
guarantee is supported by a named execution against a named commit — which is the sentence
the whole subsystem exists to make available.

The end state `docs/EVIDENCE.md` already names becomes reachable: once the baseline's
unsupported half is empty, `scripts/evidence-check` runs with `--strict` in
`.ai/repo/ci/gates.yaml`, and an unsupported guarantee refuses instead of being reported.
Until then the ratchet holds what has been earned.

The ledger becomes a file with a regular writer, so its history is a record of what this
repository could prove over time, and a claim that stops being proven is visible as a diff
rather than as an absence.

It costs one full suite run per recording. That is real wall time — the last full
validating run on master took about two and a half hours — which is why the cadence is a
decision to make deliberately rather than "on every push", and why the job is separate from
the one that decides whether a pull request may merge.

It adds pull-request traffic on a repository that has been throttled by exactly that
before. The mitigation is that at most one recording branch exists at a time: a newer
recording replaces the open one rather than queuing behind it.

## Alternatives rejected

**The validating run commits the ledger itself.** This is the obvious shape and it is
refused for the reason the evidence subsystem already states: a pull-request run has no
business writing to the repository it is judging, and a job that can commit to the trunk
without review is a hole underneath every gate that protects it.

**CI's evidence stays an artefact and a published page, and never enters the ledger.**
This is a coherent position — a run's evidence is an artefact of the run — and it is
sufficient for a reader looking at a website. It does not move a single claim out of
`not_run` in `evidence show` or in the gate, so under it "every claim is proven" can never
become a standing property of the repository. This ADR does not contradict publishing; it
adds the tracked half, which publishing does not provide.

**A person records locally and commits, as today.** It works, it is what produced the six
executions that exist, and it stopped happening eight days ago. A property that depends on
somebody remembering is not a property.

**Recording per job rather than per run.** Splitting the ledger by job would let a partial
run land faster, at the cost of a ledger assembled from executions made against different
commits. The recorder already handles a partial report correctly — it replaces only the
tests the report names — but a recording whose rows disagree about which tree was measured
makes `proven` meaningless, and `proven` is the entire product here.

## Migration

The first recording does not need the workflow and should not wait for it: a full local
run, recorded and committed, is the smallest change that makes this repository able to say
anything is proven, and it is available on the trunk today. The workflow then replaces the
habit rather than establishing it.

`scripts/evidence-check --baseline` follows each landing in its own commit with its reason,
so the ratchet records the new supported set and can refuse losing it. When the unsupported
half is empty, the gate moves to `--strict` and the non-strict report stops counting
`stale` as support, so that its green never means less than it says.
