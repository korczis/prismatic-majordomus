---
schema: adr/v1
id: adr-0047
kind: adr
title: Proof is a recorded execution, and inputs unchanged is not proven
status: proposed
date: 2026-09-11
tags:
  - evidence
  - claims
  - verification
  - ci
related:
  - rule:project.no-claim-without-test
  - rule:project.never-reported-is-not-green
  - claim:evidence-proof-is-an-execution
  - claim:evidence-currency-is-not-collapsed
  - claim:evidence-partial-run-preserves-the-rest
  - claim:evidence-navigates-both-ways
  - claim:evidence-unsupported-guarantee-is-reported
  - file:docs/EVIDENCE.md
  - file:docs/CLAIMS.yaml
  - file:apps/majordomus-cli/src/evidence/mod.rs
  - file:apps/majordomus-cli/src/evidence/ledger.rs
  - file:apps/majordomus-cli/src/evidence/record.rs
  - file:apps/majordomus-cli/src/capability/builtin/evidence.rs
  - file:scripts/evidence-check
  - file:.ai/repo/evidence-baseline.txt
  - file:.ai/repo/ci/gates.yaml
  - file:.ai/repo/evidence/README.md
  - test:test/cases/124_evidence.sh
  - file:.ai/repo/adrs/0030-a-task-owes-obligations-and-evidence-goes-stale.md
provenance:
  origin: authored
---

# 47. Proof is a recorded execution, and inputs unchanged is not proven

## Context

`docs/CLAIMS.yaml` is the canonical answer to "what does this tool actually do today?", and
every capability sentence on the public site is rendered from it. A claim declares a
`status` and names three paths: the document that defines the behaviour, the file that
implements it, and the test that proves it.

The test is named by **path**. Measured on 2026-09-11, over the matrix as it stood before
this record added its own claims to it: of 149 claims, 141 name a test — 133 references to
69 distinct cases under `test/cases/`, and 8 references to 5 integration tests under
`apps/majordomus-cli/tests/`. Exactly the 6 `planned` and the 2 `rejected` claims name
none, which is correct for what those statuses say. 135 claims declare `guaranteed`.

The only thing that had ever been checked about those 141 paths is that they resolve.
`scripts/generate-site-data --check` refuses a claim whose `source`, `implementation` or
`test` path does not exist, and refuses a `guaranteed` claim with no test path at all. That
is a check on the *reference*. It says a file exists. It does not say the case ever ran,
against which revision, with what result, or whether the result still applies.

So the concrete failure is this: a sentence reading

```text
status: guaranteed
test:   test/cases/84_distribution_model.sh
```

was displayed on the public site as a guarantee on the strength of that file existing.
Nobody — not a maintainer, not a reader, not the tool itself — could answer from the
repository whether anything behind it had been executed this month. A test file that is
never run and a test file that passes are the same file on disk, and the matrix could not
tell them apart.

This repository has already named the general shape twice. `project.never-reported-is-not-green`
says a check that never reported and a check that reported green are the same absence in
the interface a person looks at. [ADR 30](0030-a-task-owes-obligations-and-evidence-goes-stale.md)
says, one level up and about task obligations, that *a test result that cannot go stale is
a claim about the past presented as a claim about the present*, and points at the site's
`source_hash` as the mechanism that already solved it once. The claims matrix is the
largest remaining instance of both, and it is the one that faces the public.

## Decision

**A claim's proof is an execution that was recorded, and the join is derived on every
read.**

- **Identity is derived from the path the claim already names.** `test/cases/84_x.sh` is
  the case `test/run.sh` calls `84_x`; `apps/majordomus-cli/tests/why.rs` is the binary
  `cargo test --test why` runs. `suite:84_x` and `crate:why` name a join between two
  spellings that both already existed. A path no runner drives — a template, a fixture, a
  library — names no test, and is reported as such rather than counted as covered.
- **An execution is the whole of what is durably remembered about one run**: the outcome,
  the duration, the commit it ran against, whether the tree was clean, the `sha256` digest
  of the test's own source at the time, the timestamp, the origin (`local`, `ci`,
  `release`) and the exact command that produces it again. Every field is provenance; a
  result with no commit is an anonymous green and is refused at the point of recording.
- **The ledger is `.ai/repo/evidence/ledger.json`**: the latest execution per test,
  tracked, JSON, ordered by test id, merged per test. It is shaped like the benchmark
  baselines under `.ai/repo/benchmarks/` because it is the same kind of artifact — recorded,
  committed, reviewable measurement — and not something new.
- **Seven states, ranked**: `proven`, `inputs_unchanged`, `stale`, `failing`, `not_run`,
  `unrunnable`, `no_test`. Each carries its own one-sentence meaning in the model, so every
  surface explains the state the same way rather than inventing a gloss.
- **`proven` and `inputs_unchanged` are two states and must stay two.** `proven` is a
  passing run recorded against this exact commit with a clean tree. `inputs_unchanged` is a
  passing run where nothing the claim itself names — its source, its implementation, the
  test's own source — differs between the recorded commit and the working tree. That is the
  **absence of a known invalidation**, not proof at HEAD: a change the claim does not name
  could have broken it. The dependency model is deliberately shallow and non-transitive,
  and the weaker state is labelled as weaker on every surface. A projection that renders
  both as one tick has reintroduced the defect this record exists to remove.
- **Only `guaranteed` is held to its evidence.** `advisory` already says enforcement is not
  observable from outside, `planned` that nothing implements it, `rejected` that nothing
  will. Demanding a current proof of those would be demanding proof of a thing the claim
  says is absent.
- **Four capabilities, one declaration**: `evidence.report`, `evidence.claim`,
  `evidence.test`, `evidence.record`. The first three are read-only and project to the
  command line, HTTP and MCP. The recorder writes a tracked file, so it declares a command
  line and nothing else: this executable's network surfaces are read-only, and that is a
  property of the server rather than an accident of what has been built.
- **The gate starts advisory, with a ratchet and an end state.** `majordomus evidence show
  --check` exits 10 on any unsupported guarantee; `scripts/evidence-check` ratchets that
  against `.ai/repo/evidence-baseline.txt`. `--strict` is the end state, reached when the
  baseline is empty.

## Consequences

- A claim's status can be read against something that happened. `majordomus evidence show`
  is the whole matrix; `evidence claim` and `evidence test` walk the relation in both
  directions from the same derivation, so the two answers cannot disagree.
- On the day this lands, every claim reads `not_run`, because nothing has been recorded.
  That is the true answer and it is visible, which is the improvement: the previous state
  was not "proven", it was "unknown, displayed as guaranteed".
- **The gate cannot block yet.** A blocking gate against an empty ledger makes the tree
  unmergeable for everyone until a full suite run exists. So the debt at landing is written
  to `.ai/repo/evidence-baseline.txt` by `scripts/evidence-check --baseline`, never by hand,
  and the ratchet is the one `pipefail-check` and `liveness-check` already use: a claim in
  the baseline is known debt, a claim not in it is a regression and fails, a baseline line
  that is now supported is reported as stale so the list tightens rather than rots. The
  baseline shrinks only by recording a run that proves a claim. CI switches to `--strict`
  when it is empty. The gate is planned on every push — `evidence-check` in the `structure`
  job of `.ai/repo/ci/gates.yaml`, `always: true` — because a gate nobody runs is the other
  half of `project.never-reported-is-not-green`.
- **`proven` is rare in this repository, by construction.** The ledger is tracked, so
  recording leaves the tree dirty, and committing the ledger moves HEAD past the commit the
  executions name. The strongest state this repository usually shows about itself is
  therefore `inputs_unchanged`. That is the model being honest rather than the model
  failing: after a recording, neither half of `proven` holds, and pretending otherwise
  would be performing the collapse this record refuses on its own data.
  `test/cases/124_evidence.sh` reaches both sides in a fixture repository that ignores its
  ledger, which is the only place the distinction can be demonstrated.
- **CI's half of the recording is not wired.** The suite job already writes
  `MJ_TEST_REPORT=suite.tsv` and uploads it; the recording step is one line, but a ledger
  written in a CI checkout is discarded with the runner. Landing CI's evidence needs a
  push-to-master job that records and commits the file, the way a benchmark baseline is
  written. Until then the ledger is written by whoever runs the suite locally and commits
  it, and the states are honest either way.
- **It is not tamper-proof and does not claim to be.** The ledger is a tracked file; a
  person can write `"outcome": "pass"` into it and the diff is what a reviewer sees. The
  digest buys staleness detection that survives a revert, not forgery resistance.
  Cryptographic ceremony against someone who can already commit would be ceremony with no
  threat model.
- **Two neighbours share the word and not the subject**, and both are named in
  `docs/EVIDENCE.md` so a reader does not conflate them. `src/execution/`
  ([ADR 33](0033-an-execution-is-a-watched-capability-call-not-a-second-registry.md)) is
  in-flight capability executions of a running server, gone with the process; `lib/evidence.sh`
  ([ADR 30](0030-a-task-owes-obligations-and-evidence-goes-stale.md)) is what a task owes
  before it may be called completed. Neither reads this subsystem's records and it reads
  neither of theirs.
- The claims matrix gains five claims about this subsystem, which are themselves subject to
  it. A subsystem that measures proof and exempts itself would be the first thing to
  distrust.

## Alternatives rejected

- **A status field in `docs/CLAIMS.yaml`, written by hand.** `verified: true`, or a
  `last_verified` date beside the test path. It is the same defect with more syntax: a
  hand-written assertion that a run happened, which nothing produced and nothing can
  invalidate, in a file whose whole purpose is to stop hand-written assertions. It would
  also make the matrix a file that both a person and a tool write, which is the
  `project.derived-once` failure.
- **A full execution history rather than the latest per test.** Every run of every test,
  per commit, appended. It grows without bound in a tracked file, and the question this
  answers — *is this claim proven now?* — only ever reads the newest row. The history of
  what was recorded when already exists: it is git, over this file. A second history would
  be a second thing to maintain in service of a question nobody asked.
- **Storing the runs' output.** A captured log per case is megabytes per run, belongs where
  the run happened, and would turn a reviewable file into an artifact nobody reads in a
  diff. What is kept is the durable semantic part plus the command that produces the output
  again, which is what a reader needs to decide whether to believe the result.
- **Requiring an exact commit match as the only notion of currency** — one state, `proven`,
  and everything else stale. It is sound and it is useless here. This repository pushes to
  master every few minutes on an integration day and the full suite takes far longer than
  that, so every claim would be stale except in the seconds after a run, the gate would be
  noise from the first hour, and a noisy gate is one that gets turned off. The two-state
  answer keeps the strong statement available and says plainly when it only has the weaker
  one.
- **The opposite: one passing state.** Collapsing `inputs_unchanged` into `proven` gives a
  green badge whose derivation cannot be inspected, which is the exact artifact this record
  exists to refuse. It is rejected more firmly than anything else here, and it is the
  rejection a future change is most likely to undo by accident, because it looks like
  simplification.
- **A transitive dependency graph for invalidation** — resolving what the implementation
  imports, what the case invokes, what those read. It is more precise in principle and
  wrong in practice: it is a second model of the codebase that has to be maintained, it
  goes stale silently, and when it is wrong it is wrong in the dangerous direction, saying
  "still proven" about something it failed to reach. The shallow model is wrong in the same
  direction, but it is three paths a reader can check by eye, and `docs/EVIDENCE.md` states
  what it does not catch.
- **A new test registry.** A file listing every test with its id, runner, owner and the
  claims it covers. It duplicates the matrix, needs an entry per test, and creates a second
  place for a test to be named — so the first defect would be a test that exists in one and
  not the other. Identity is derived from the path the claim already gives instead.
- **A third-party test-reporting service or a JUnit XML pipeline.** It puts the evidence
  outside the repository, so a clone does not carry it, an offline reader cannot see it,
  and the tool cannot join it. The precedent that already works here is a small tracked
  JSON file beside the benchmark baselines.

## Migration

There is none, and that was a design goal rather than a happy outcome.

No field is added to `docs/CLAIMS.yaml`, no claim is edited, and no data is entered. The
test identity is computed from the `test` path each claim already carries, so the
migration is a function and it ran the moment the code existed: every claim naming a case
or an integration test resolves, and the 8 claims naming none are exactly the `planned` and
`rejected` ones, which is what those statuses mean.

This matters beyond convenience. A migration requiring 141 hand-written entries would have
been done partly, in a hurry, by somebody translating paths into ids — and every mistake in
that translation would be a claim silently reporting `not_run` or, worse, inheriting
another test's evidence. The design constraint was therefore stated first: **the subsystem
must be derivable from the matrix as it stands, or it is the wrong design.** Anything that
required editing the matrix would have been a second declaration of which test proves which
claim, which is the defect this repository calls `project.derived-once`.

The one-way step is the ledger itself: `.ai/repo/evidence/ledger.json` does not exist until
a run is recorded, and its absence is not an error. A repository that has recorded nothing
reports every claim as `not_run`, which is true, and the first recording is a normal commit.
