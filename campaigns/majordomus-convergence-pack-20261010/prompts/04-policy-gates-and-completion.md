# Milestone 04: Enforcement-backed verified delivery

Discover live `.ai/repo/ci/gates.yaml`, `scripts/ci/verdict`, existing quality suites, rules, `doctor` proof chain, `finish --check` and GitHub Actions jobs before changing. Published claim about unprotected `master` was measured 2026-09-11; verify current settings by GitHub API, do not assume this persists. Repo file changes cannot create server-side GitHub protection by themselves.

## Issue 04.1: Single authoritative quality policy

Model fast local, PR impacted, complete integration and release-level checks as policies of the existing gate dispatch and claim/evidence system. Avoid duplicated inventories in YAML, shell, CI and Rust. Affected-test selection must be conservative and reproducible, with full fallback when impact cannot be proven. Diagnostics name causal reason, reproducible command and policy owner.

## Issue 04.2: CI wiring and non-bypassable verdict

Prove the status aggregator waits on all required jobs (including failure/skip/cancel), fails closed and corresponds to actual GitHub required status. Provide repository-config steps/API mutation only with explicit authorized access, current audited state and safeguards. Do not claim required checks when settings are missing. Defend against branch race, stale PR check at old head, merge queue/merge commit semantics and CI cancellation. Keep credentials least privileged.

## Issue 04.3: Completion semantics from evidence

A completed task must meet linked intent criteria where present, current HEAD scope, verified tests, generated artifact consistency, review/integration requirements and deployment checks only when deploy is part of declared goal. Distinguish code merged, artifact published, deployed and healthy; no one generic green flag for all. Failure/retry does not mark completion. Preserve current typed outcomes and additive migration compatibility.

## Issue 04.4: Release/deploy provenance

Reuse #874's provenance archive attestation, installer verification and existing version/release machinery. Test artifact hash, signature/attestation or whatever actual implementation provides; do not claim stronger cryptographic guarantees than code implements. Verify rollback/no-corrupt installation, version bump policy and deploy health when target exists. Release jobs must use immutable artifact identity, not an unversioned mutable branch tip.

## Issue 04.5: Adversarial tests, documentation and doctrine

Test bypass attempts, red CI, skipped jobs, malformed gate, incorrect head SHA, mismatched attestation, repeated release, lost deploy acknowledgment, partial rollback and docs/claims drift. Ensure every enforced rule has registered validator, executable rejection test, and CI connection proven by `doctor`. Keep optional complexity optional for adopters.

Acceptance: required checks cannot be bypassed under the configured GitHub policy, `finish` cannot lie about a failed task, and release/deploy state is recorded with actual evidence. Unconfigured GitHub protection must be a visible blocker, not silently declared solved.
