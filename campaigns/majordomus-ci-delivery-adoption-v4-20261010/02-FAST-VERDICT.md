# M02: Fast verdict

## Goal
Move correct impact-required evidence onto shortest possible critical path while maintaining merge guarantees.

## Issues

## Issue V4-02-01: Canonical fast-plan schema

**Implementation sequence**
1. Extend existing `.ai/repo/ci/gates.yaml` or its native schema with gate cost/criticality/evidence locality/freshness semantics as supported, not an unrelated YAML file.
2. Update `scripts/ci-plan` to derive minimal safe plan with explainable selected/unselected reasons and dependent closure.
3. Unknown paths, invalid metadata, stale proof or capability drift must escalate/fail closed.
4. Run permutation and matrix tests using the current repository path classes.

**Acceptance / evidence**
- [ ] One canonical gate inventory
- [ ] Plan determinism and escalation tests pass

### Required invariant per issue
- Treat existing repo model as canonical; update it rather than adding an independent source of truth.
- Link work to intent/milestone, execute focused positive+negative tests, record evidence and gate results.
- Update relevant README/ADR/rules/claims only when the repo policy requires, and derive generated docs.
- Check scope conflicts with concurrent PRs; publish only verified results. Capture previous/current timing on affected path.

## Issue V4-02-02: Structure job decomposition

**Implementation sequence**
1. Separate cheap file-only validation from build-heavy checks using canonical gate job descriptors.
2. Eliminate repeated tool installation using pinned actions/tools where appropriate; do not hardcode unmanaged binary releases.
3. Remove unnecessary history fetching only from jobs that provably require neither tags nor cross-ref checks.
4. Ensure cheap gates run immediately and their failures return actionable diagnostics.

**Acceptance / evidence**
- [ ] No dropped gates or duplicate inventories
- [ ] Before/after structure critical path measured

### Required invariant per issue
- Treat existing repo model as canonical; update it rather than adding an independent source of truth.
- Link work to intent/milestone, execute focused positive+negative tests, record evidence and gate results.
- Update relevant README/ADR/rules/claims only when the repo policy requires, and derive generated docs.
- Check scope conflicts with concurrent PRs; publish only verified results. Capture previous/current timing on affected path.

## Issue V4-02-03: Aggregator integrity

**Implementation sequence**
1. Modify `scripts/ci/verdict` and workflow adapter as needed to assert required gates were executed or have valid permitted evidence.
2. Keep a stable required check context; ensure cancellation, skipped job, missing metrics, missing plan and in-progress statuses cannot falsely pass.
3. Use a test fixture to simulate red, skipped, cancelled, missing, cache reused and race scenarios.
4. Expose precise machine readable plan/results digest and provenance.

**Acceptance / evidence**
- [ ] Fail-closed negative fixtures pass
- [ ] GitHub required check protection reconciled

### Required invariant per issue
- Treat existing repo model as canonical; update it rather than adding an independent source of truth.
- Link work to intent/milestone, execute focused positive+negative tests, record evidence and gate results.
- Update relevant README/ADR/rules/claims only when the repo policy requires, and derive generated docs.
- Check scope conflicts with concurrent PRs; publish only verified results. Capture previous/current timing on affected path.

## Issue V4-02-04: Queue and concurrency tuning

**Implementation sequence**
1. Evaluate per-PR cancellation semantics versus always-producing-verdict requirement using measured run churn.
2. Reduce redundant queued runs safely without cancelling the only run from which valid verdict can be derived.
3. Avoid deadlocks in artifact/dependency jobs and avoid a serial job waiting on unused heavy jobs.
4. Document trade-off between freshness and queue consumption.

**Acceptance / evidence**
- [ ] No indefinite no-verdict lifecycle
- [ ] Queue p95 measured before/after

### Required invariant per issue
- Treat existing repo model as canonical; update it rather than adding an independent source of truth.
- Link work to intent/milestone, execute focused positive+negative tests, record evidence and gate results.
- Update relevant README/ADR/rules/claims only when the repo policy requires, and derive generated docs.
- Check scope conflicts with concurrent PRs; publish only verified results. Capture previous/current timing on affected path.

## Milestone exit gate
- [ ] Canary PR receives correct fast verdict
- [ ] Full/unknown-impact PRs remain protected
