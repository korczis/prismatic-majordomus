# M05: AI policy migration

## Goal
Make new performance/autonomy behavior canonical in `.ai/` and executable enforcement, without accumulating contradictory governance.

## Issues

## Issue V4-05-01: Doctrine and rule inventory

**Implementation sequence**
1. Enumerate live `.ai/` rules/doctrines tied to CI, completion, derive, performance, workers, PR drain, scope and release.
2. Find contradictory old requirements, redundant documents and historical accepted debt.
3. Produce migration mapping old identity → revised/retired identity, retaining links/ADRs and non-regression protections.
4. Apply one minimal authoritative policy update rather than adding ten overlapping documents.

**Acceptance / evidence**
- [ ] No unresolved active contradictory rule pair
- [ ] Deleted/deprecated rules have references migrated

### Required invariant per issue
- Treat existing repo model as canonical; update it rather than adding an independent source of truth.
- Link work to intent/milestone, execute focused positive+negative tests, record evidence and gate results.
- Update relevant README/ADR/rules/claims only when the repo policy requires, and derive generated docs.
- Check scope conflicts with concurrent PRs; publish only verified results. Capture previous/current timing on affected path.

## Issue V4-05-02: Rule enforcement wiring

**Implementation sequence**
1. Ensure revised policy names executable proof in existing gate registry, tests and evidence ledger.
2. Add negative test that intentionally violates fast-plan integrity, cache trust, mandatory check or performance budget; prove CI rejects.
3. Use actual `.ai/` schema and generator/doctor validation, not improvised file format.

**Acceptance / evidence**
- [ ] Every blocking doctrine mechanically tested
- [ ] Existing rule-proof/claim-proof checks pass

### Required invariant per issue
- Treat existing repo model as canonical; update it rather than adding an independent source of truth.
- Link work to intent/milestone, execute focused positive+negative tests, record evidence and gate results.
- Update relevant README/ADR/rules/claims only when the repo policy requires, and derive generated docs.
- Check scope conflicts with concurrent PRs; publish only verified results. Capture previous/current timing on affected path.

## Issue V4-05-03: Intent milestone lifecycle

**Implementation sequence**
1. Link all V4 issues to existing intended capability/criteria; reconcile with V2 and existing issues without duplicates.
2. Define what constitutes a complete increment: code + selected evidence + docs + accepted CI + integration + release eligibility.
3. Allow autonomous scope refinement only under delegated intent and authorization, with typed evidence and conflict resolution.

**Acceptance / evidence**
- [ ] No orphan issue/milestone
- [ ] Completion refuses missing evidence

### Required invariant per issue
- Treat existing repo model as canonical; update it rather than adding an independent source of truth.
- Link work to intent/milestone, execute focused positive+negative tests, record evidence and gate results.
- Update relevant README/ADR/rules/claims only when the repo policy requires, and derive generated docs.
- Check scope conflicts with concurrent PRs; publish only verified results. Capture previous/current timing on affected path.

## Issue V4-05-04: Performance SLO as evidence

**Implementation sequence**
1. Declare 300s p95 and cohort definition in canonical claims/budget model if supported; select appropriate CI performance probe.
2. Implement a ratchet that is statistically meaningful and resistant to random runner outliers; distinguish informational warning from blocking by cohort.
3. Do not claim achieved until representative measured sample passes.

**Acceptance / evidence**
- [ ] Baseline / improvement evidence tracked
- [ ] Unachieved SLA reported explicitly

### Required invariant per issue
- Treat existing repo model as canonical; update it rather than adding an independent source of truth.
- Link work to intent/milestone, execute focused positive+negative tests, record evidence and gate results.
- Update relevant README/ADR/rules/claims only when the repo policy requires, and derive generated docs.
- Check scope conflicts with concurrent PRs; publish only verified results. Capture previous/current timing on affected path.

## Milestone exit gate
- [ ] `.ai/` doctor and proofs pass
- [ ] No parallel policy schema
