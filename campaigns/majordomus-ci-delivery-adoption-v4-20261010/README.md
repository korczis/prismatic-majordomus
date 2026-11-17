# Majordomus CI Delivery Adoption V4

**Mission:** execute an incremental, security-preserving CI/CD acceleration and roll it into the repository's existing `.ai/`, tooling, documentation, autonomous mesh, and production delivery surfaces. This is an implementation instruction package for Claude Code, NOT a claim of execution.

Repository: `korczis/prismatic-majordomus`. Baseline source review: 2026-10-10. Re-audit live master and open PRs before mutations.

## Proven repository anchors (inspect and correct if evolved)

- `.github/workflows/validate.yml`: `plan → structure / selected jobs → ci` verdict. Historical long structural checks and 4-suite-shard jobs.
- `.github/actions/setup-rust/action.yml`: already uses `Swatinem/rust-cache@v2`, scoped by job. Cache exists; improving it requires evidence.
- `.ai/repo/ci/gates.yaml`: canonical gates and changed-path classes; `scripts/ci-plan` derives the plan and `scripts/ci/run-plan` executes it.
- `scripts/derive`: multi-stage A/B/C/D generation under machine lock; respect tracked-index preflight, generator binary generation, and `scripts/derive-check`.
- `.github/workflows/release.yml`: tagged, verified platform builds, published archive provenance and release record regeneration. Do not downgrade guarantees.
- Earlier pack: `majordomus-ci-under-5min-20261010/`, with 00–07 prompts. V4 extends its intent; reconcile not duplicate.

The workflow's historical comments include `core-check` 698–714 s, shell-lint around 300 s, and suite shard observations of 94–148 minutes. These are **historical notes**, not fresh measured medians. Fetch recent run/job timings as evidence.

## Policy envelope

A previously authorized autonomous work item can continue without chat-based `proceed`, but only through actually deployed supervisor/mesh capabilities. Never assume Claude's context window or this prompt can guarantee durable execution. Existing branch protections, required checks, provenance, secret boundaries, and deploy authorization stay binding. Do not implement a blanket `--dangerously-skip-permissions`, bypass policies, or publish from untrusted PR code.

## Operational definition

- Fast PR feedback is useful only if it includes every *impact-required* gate, with evidence and an auditable reason for each non-selected gate.
- Target **p95 end-to-end PR required verdict <300 seconds** on representative routine PRs, measuring queue separately, with cold-cache percentiles and exceptional classes recorded. No false SLA claims.
- Expensive unaffected tests may be moved to other verification lanes only if canonical impact mapping is complete, stale evidence fails closed, and branch/merge policy actually enforces all impact-relevant prerequisites.
- Continuous release on verified meaningful increments per canonical versioning policy, not arbitrary version bumps. Deploy only to authorized environments, record health and rollback capability.

## Mandatory execution contract (every issue)

1. Read current canonical source, tests and consumers; identify pre-existing functionality and current PR overlap.
2. Produce one minimal implementation slice; delete superseded paths/configs; do not introduce competing registries.
3. Update typed schemas, canonical `.ai/` definitions and any generated projections using existing generators; preserve backward compatibility or migrate explicitly.
4. Write positive, negative, concurrency or failure-injection tests appropriate to the change. Include an adversarial case that would have passed under the old bug.
5. Run impacted local checks, record timings and exact commands as evidence (never fabricate results).
6. Create/update canonical issue and its acceptance criteria, linked to overarching intent, milestone and release slice. Sync to GitHub using existing supported workflow.
7. Commit, push, PR and merge **only** with passing required checks. Reconcile against fresh master after merge.
8. Evaluate canonical release policy; where release-eligible, bump/tag/release/deploy/verify, then continue working. If unable, record a named external blocker and proceed on unblocked dependencies.
9. Emit a machine-readable checkpoint: issue ID, commit, selected/executed/reused gates, performance delta, release/deploy evidence and next ready task.

## Prohibited shortcuts

- No fake green aggregator, no `continue-on-error` on required gates, no `|| true` concealing failures.
- No `paths-ignore` fork of `.ai/repo/ci/gates.yaml`; CI workflow is only an adapter.
- No unsafe cache reuse across trust domains, OS/toolchain/RUSTFLAGS/target/generator generations.
- No broadening cache storage write access to arbitrary fork pull requests.
- No hard-coded duplicate changelog, website registry, UI or documentation feature counts.
- No disabling expensive tests solely to meet the five-minute number.
- No destructive cleanup of PR branches/worktrees owned by another worker without verified lease/owner and merge state.

## Contents / order

`00-EXECUTE.md` is the sole entry point. `01-LIVE-INVENTORY.md` → `02-FAST-VERDICT.md` → `03-CACHE-AND-BUILDS.md` → `04-SUITE-AND-DERIVE.md` → `05-AI-POLICY-MIGRATION.md` → `06-TOOLING-AND-MESH.md` → `07-DOCS-AND-UX.md` → `08-RELEASE-ROLLOUT.md` → `09-VALIDATION-AND-CLOSEOUT.md`.

Each milestone includes independently executable issue sections with stable IDs, blocking tests, and completion evidence. Follow dependencies in `manifest.json`, not filename sequence blindly. `TRACKING.md` defines the status ledger contract.
