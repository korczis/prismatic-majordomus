# M01: Live inventory

## Goal
Capture actual master, PR and Actions run graph before altering trust boundaries.

## Issues

## Issue V4-01-01: Canonical CI topology

**Implementation sequence**
1. Read `validate.yml`, all referenced actions and `.ai/repo/ci/gates.yaml`; enumerate every job and actual `needs` edges.
2. Trace `scripts/ci-plan`, `scripts/ci/run-plan`, `scripts/ci/verdict`, `release-verdict` and required branch status mapping.
3. Locate all builds/derives invoked transitively by gates; build a call graph and repeated-work table.
4. Cross-reference outstanding PRs touching workflows, caches, test runners or mesh; deduplicate or integrate rather than racing.

**Acceptance / evidence**
- [ ] Diagram of actual critical path with file+line provenance
- [ ] Current merge protection and required status names recorded, or external blocker explicit

### Required invariant per issue
- Treat existing repo model as canonical; update it rather than adding an independent source of truth.
- Link work to intent/milestone, execute focused positive+negative tests, record evidence and gate results.
- Update relevant README/ADR/rules/claims only when the repo policy requires, and derive generated docs.
- Check scope conflicts with concurrent PRs; publish only verified results. Capture previous/current timing on affected path.

## Issue V4-01-02: Measure current action runs

**Implementation sequence**
1. Fetch at least 20 recent relevant PR runs and job-step timestamps using GitHub Actions; separate queue, setup, build, test, artifacts and verdict.
2. Classify by path class, change size, warm/cold cache and runner environment; do not average incomparable classes.
3. Compute p50/p95/max, failed/cancelled runs, and runner queue distribution; attach raw anonymized run IDs.
4. Identify worst unavoidable serial predecessors and hypothesize optimizations ranked by measured savings.

**Acceptance / evidence**
- [ ] Reproducible raw timing dataset and calculation script
- [ ] Historical comments separated from fresh measurements

### Required invariant per issue
- Treat existing repo model as canonical; update it rather than adding an independent source of truth.
- Link work to intent/milestone, execute focused positive+negative tests, record evidence and gate results.
- Update relevant README/ADR/rules/claims only when the repo policy requires, and derive generated docs.
- Check scope conflicts with concurrent PRs; publish only verified results. Capture previous/current timing on affected path.

## Issue V4-01-03: Cache and artifact census

**Implementation sequence**
1. Inventory Rust/cache setup, keys, job isolation, restore prefixes, save-on-failure, target dirs, feature flags and compiler flags.
2. Inspect npm/site caches and artifact upload/download duplication.
3. Verify fork/untrusted PR cache write boundaries and default branch poison resistance.
4. Record hit rates and cold-run costs; identify source-only jobs that should never build Rust.

**Acceptance / evidence**
- [ ] Cache matrix with trust model
- [ ] Measured bytes restored and time spent

### Required invariant per issue
- Treat existing repo model as canonical; update it rather than adding an independent source of truth.
- Link work to intent/milestone, execute focused positive+negative tests, record evidence and gate results.
- Update relevant README/ADR/rules/claims only when the repo policy requires, and derive generated docs.
- Check scope conflicts with concurrent PRs; publish only verified results. Capture previous/current timing on affected path.

## Issue V4-01-04: Test impact proof inventory

**Implementation sequence**
1. Enumerate test cases and durations, `.ai/repo/ci/suite.yaml`, `suite-durations.tsv`, `test/run.sh`, escape hatches and skip rules.
2. Identify critical correctness gates versus full integration, platform, browser/site and durability stress tests.
3. Create impact coverage matrix: changed path → gate → case(s) → proof freshness requirements.
4. Construct adversarial unmatched-path fixture proving escalation, not blind skipping.

**Acceptance / evidence**
- [ ] Every gate classified with safe impact semantics
- [ ] Proof of fail-closed unknown change

### Required invariant per issue
- Treat existing repo model as canonical; update it rather than adding an independent source of truth.
- Link work to intent/milestone, execute focused positive+negative tests, record evidence and gate results.
- Update relevant README/ADR/rules/claims only when the repo policy requires, and derive generated docs.
- Check scope conflicts with concurrent PRs; publish only verified results. Capture previous/current timing on affected path.

## Issue V4-01-05: Baseline risk and rollback

**Implementation sequence**
1. Record last green master SHA, current tag, deployed site and release state without modifying.
2. Test current CI verdict failure handling in fixtures; capture existing baseline debt rather than papering over it.
3. Create rollback strategy for CI workflow and `.ai/` model updates in case merge queue freezes.

**Acceptance / evidence**
- [ ] Rollback plan and owners
- [ ] Pre-change evidence immutable and indexed

### Required invariant per issue
- Treat existing repo model as canonical; update it rather than adding an independent source of truth.
- Link work to intent/milestone, execute focused positive+negative tests, record evidence and gate results.
- Update relevant README/ADR/rules/claims only when the repo policy requires, and derive generated docs.
- Check scope conflicts with concurrent PRs; publish only verified results. Capture previous/current timing on affected path.

## Milestone exit gate
- [ ] Baseline artifacts committed to canonical evidence mechanism without exposing secrets
- [ ] No workflow mutation before initial critical path documented
