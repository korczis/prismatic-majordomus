# M04: Suite and derive

## Goal
Replace all-or-nothing critical-path execution with proven targeted checks and deterministic incremental derivation.

## Issues

## Issue V4-04-01: Case impact selection

**Implementation sequence**
1. Extend `suite.yaml` / `test/run.sh` to map runnable cases to inputs/capabilities and long-tail classifications using discovered metadata.
2. Prove union of selected shards equals selected case set; no-skip required cases must never silently skip.
3. Move expensive unaffected cases to merge/schedule/release with explicit valid evidence and mandatory escalation for touched code.
4. Randomize path classes, rename cases and perturb durations in tests.

**Acceptance / evidence**
- [ ] Case coverage loss detected
- [ ] Unknown cases default conservatively

### Required invariant per issue
- Treat existing repo model as canonical; update it rather than adding an independent source of truth.
- Link work to intent/milestone, execute focused positive+negative tests, record evidence and gate results.
- Update relevant README/ADR/rules/claims only when the repo policy requires, and derive generated docs.
- Check scope conflicts with concurrent PRs; publish only verified results. Capture previous/current timing on affected path.

## Issue V4-04-02: Long-tail test isolation

**Implementation sequence**
1. Find longest tests by measured durations and split or optimize their setup while preserving behavior.
2. Avoid repeated cargo/site builds per disposable fixture; share readonly verified inputs, not mutable fixture state.
3. Bound case timeout with diagnostic detail and report runner starvation.

**Acceptance / evidence**
- [ ] Same failure modes covered
- [ ] Critical path shortened measurably

### Required invariant per issue
- Treat existing repo model as canonical; update it rather than adding an independent source of truth.
- Link work to intent/milestone, execute focused positive+negative tests, record evidence and gate results.
- Update relevant README/ADR/rules/claims only when the repo policy requires, and derive generated docs.
- Check scope conflicts with concurrent PRs; publish only verified results. Capture previous/current timing on affected path.

## Issue V4-04-03: Derive dependency graph

**Implementation sequence**
1. Inspect A/B/C/D graph and stage consumers; formalize output/input fingerprints and generator version.
2. Implement selective regeneration only with explicit dependency closure and golden equivalence against full derive.
3. Respect tracked-index preflight, machine lock and read/write ownership; clean partial temp artifacts after cancellation.
4. Do not cache outputs whose correctness depends on undocumented environment/side effects.

**Acceptance / evidence**
- [ ] Incremental derive equals full derive bytes for test corpus
- [ ] Stale generator fails closed

### Required invariant per issue
- Treat existing repo model as canonical; update it rather than adding an independent source of truth.
- Link work to intent/milestone, execute focused positive+negative tests, record evidence and gate results.
- Update relevant README/ADR/rules/claims only when the repo policy requires, and derive generated docs.
- Check scope conflicts with concurrent PRs; publish only verified results. Capture previous/current timing on affected path.

## Issue V4-04-04: Single-flight and recovery

**Implementation sequence**
1. Narrow locking to actual conflicting output sets if safe while keeping `one derive at a time` invariant until conflict proof allows relaxing it.
2. Crash-test partial outputs, process reaping, lock ownership and stale priority marker.
3. Make the eventual converged tree atomic and reproducible across workers/platforms.

**Acceptance / evidence**
- [ ] No orphan processes or partial committed projections
- [ ] Parallel unrelated work demonstrably safe

### Required invariant per issue
- Treat existing repo model as canonical; update it rather than adding an independent source of truth.
- Link work to intent/milestone, execute focused positive+negative tests, record evidence and gate results.
- Update relevant README/ADR/rules/claims only when the repo policy requires, and derive generated docs.
- Check scope conflicts with concurrent PRs; publish only verified results. Capture previous/current timing on affected path.

## Milestone exit gate
- [ ] Fast suite + full suite both provably valid
- [ ] Derived docs and site data unchanged for unchanged inputs
