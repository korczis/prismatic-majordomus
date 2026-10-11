# M08: Release rollout

## Goal
Merge and activate verified improvements on the actual default branch, then ship eligible release safely.

## Issues

## Issue V4-08-01: Canary rollout

**Implementation sequence**
1. Create small canary PR(s) on current master using canonical PR workflow.
2. Exercise target path on change sets: docs-only, shell, Rust, schema, workflow, unknown path, fork PR, forced cold cache and merge-base races.
3. Observe before/after sample, error rates and gate execution proof; do not use raw speed alone as criterion.

**Acceptance / evidence**
- [ ] Canary demonstrates correct merge verdict
- [ ] Rollback procedure rehearsed

### Required invariant per issue
- Treat existing repo model as canonical; update it rather than adding an independent source of truth.
- Link work to intent/milestone, execute focused positive+negative tests, record evidence and gate results.
- Update relevant README/ADR/rules/claims only when the repo policy requires, and derive generated docs.
- Check scope conflicts with concurrent PRs; publish only verified results. Capture previous/current timing on affected path.

## Issue V4-08-02: Integrate and version

**Implementation sequence**
1. Drain mergeable verified PRs incrementally with existing `prs drain`, fresh review of master and conflict checks.
2. Run canonical release analysis and bump version only if required; generate changelog/release record from canonical sources.
3. Execute release workflow with tag/version and attestation verification; distinguish pending publication from done.

**Acceptance / evidence**
- [ ] All merges and tag SHAs recorded
- [ ] Release provenance verified

### Required invariant per issue
- Treat existing repo model as canonical; update it rather than adding an independent source of truth.
- Link work to intent/milestone, execute focused positive+negative tests, record evidence and gate results.
- Update relevant README/ADR/rules/claims only when the repo policy requires, and derive generated docs.
- Check scope conflicts with concurrent PRs; publish only verified results. Capture previous/current timing on affected path.

## Issue V4-08-03: Deploy and activate

**Implementation sequence**
1. Use existing Pages/service deploy workflow, respecting configured environments and required approvals.
2. Verify live version, health, install archive checksum and signed provenance, docs routes and UI projections after rollout.
3. Activate default fast paths in self-hosting and installed consumers only following canary success; phase rollout if needed.

**Acceptance / evidence**
- [ ] Actual deployed URLs and commit SHAs captured
- [ ] Production smoke and rollback verified

### Required invariant per issue
- Treat existing repo model as canonical; update it rather than adding an independent source of truth.
- Link work to intent/milestone, execute focused positive+negative tests, record evidence and gate results.
- Update relevant README/ADR/rules/claims only when the repo policy requires, and derive generated docs.
- Check scope conflicts with concurrent PRs; publish only verified results. Capture previous/current timing on affected path.

## Issue V4-08-04: Continuous improvement feedback

**Implementation sequence**
1. Feed per-PR timing cohorts, cache hit rates, CI failures and mesh integration throughput into canonical evidence.
2. Define regression threshold and automatic alert/rollback or block path according to existing policy.
3. Ensure subsequent changes trigger intended gate selection without manual override.

**Acceptance / evidence**
- [ ] After rollout, new PRs run updated model
- [ ] Observed sample counters and drift safeguards

### Required invariant per issue
- Treat existing repo model as canonical; update it rather than adding an independent source of truth.
- Link work to intent/milestone, execute focused positive+negative tests, record evidence and gate results.
- Update relevant README/ADR/rules/claims only when the repo policy requires, and derive generated docs.
- Check scope conflicts with concurrent PRs; publish only verified results. Capture previous/current timing on affected path.

## Milestone exit gate
- [ ] Release/deploy truly verified rather than assumed
- [ ] PR p95 target met or documented blocked with raw data
