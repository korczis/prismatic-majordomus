# M09: Validation and closeout

## Goal
Prove the architecture survives adversarial input and is used today, not merely described.

## Issues

## Issue V4-09-01: Cross-layer negative tests

**Implementation sequence**
1. Run/implement matrix: stale SHA, missing verdict, cancelled job, mismatch toolchain, forged cache artifact, fork PR, unknown changed path, concurrent derive, lost worker, unauthorized deploy.
2. Require each bad case to fail closed while allowed good cases succeed and complete.
3. Check policy and docs drift gates after intentional perturbation.

**Acceptance / evidence**
- [ ] Negative suite green without weakening behavior
- [ ] No silent success or missing gates

### Required invariant per issue
- Treat existing repo model as canonical; update it rather than adding an independent source of truth.
- Link work to intent/milestone, execute focused positive+negative tests, record evidence and gate results.
- Update relevant README/ADR/rules/claims only when the repo policy requires, and derive generated docs.
- Check scope conflicts with concurrent PRs; publish only verified results. Capture previous/current timing on affected path.

## Issue V4-09-02: SLO and coverage proof

**Implementation sequence**
1. Collect >=10 comparable routine PR validation runs (and >=3 cold misses) as initial performance evidence; report cohorts, queue and p50/p95/max.
2. Verify complete suite, targeted suite, derived-site and cross-surface contract tests on current integrated SHA.
3. Report 5-minute requirement truthfully: pass only if measured according to documented cohort; else list exact blockers and follow-up issues.

**Acceptance / evidence**
- [ ] Raw metrics with reproducible calculations
- [ ] No claim of success without real measurements

### Required invariant per issue
- Treat existing repo model as canonical; update it rather than adding an independent source of truth.
- Link work to intent/milestone, execute focused positive+negative tests, record evidence and gate results.
- Update relevant README/ADR/rules/claims only when the repo policy requires, and derive generated docs.
- Check scope conflicts with concurrent PRs; publish only verified results. Capture previous/current timing on affected path.

## Issue V4-09-03: Consumer self-host proof

**Implementation sequence**
1. Install current release into clean independent repository; run impacted check via canonical entry point.
2. Validate `.ai/` update/migration, completions, MCP/UI parity and mesh handover if enabled.
3. Capture install, smoke, optional rollback and version match evidence.

**Acceptance / evidence**
- [ ] Out-of-repo reproducibility
- [ ] No hard-coded Majordomus own repository assumptions

### Required invariant per issue
- Treat existing repo model as canonical; update it rather than adding an independent source of truth.
- Link work to intent/milestone, execute focused positive+negative tests, record evidence and gate results.
- Update relevant README/ADR/rules/claims only when the repo policy requires, and derive generated docs.
- Check scope conflicts with concurrent PRs; publish only verified results. Capture previous/current timing on affected path.

## Issue V4-09-04: Final governance and cleanup

**Implementation sequence**
1. Delete obsolete duplicate caches, scripts, contradictory policy, workflows and docs after consumers migrate.
2. Run canonical full validation, check generated diffs and compare claimed guarantees with evidence ledger.
3. Report merged/open PRs, releases, deployments, SLO, uncovered risk and next iteration; leave no orphaned issue.

**Acceptance / evidence**
- [ ] Exact commands/outcomes and PR/release links
- [ ] Final status PASS or INCOMPLETE honestly

### Required invariant per issue
- Treat existing repo model as canonical; update it rather than adding an independent source of truth.
- Link work to intent/milestone, execute focused positive+negative tests, record evidence and gate results.
- Update relevant README/ADR/rules/claims only when the repo policy requires, and derive generated docs.
- Check scope conflicts with concurrent PRs; publish only verified results. Capture previous/current timing on affected path.

## Milestone exit gate
- [ ] Evidence-backed final report published in canonical state
- [ ] No manual `proceed` to finish an authorized recoverable workflow when runtime support exists
