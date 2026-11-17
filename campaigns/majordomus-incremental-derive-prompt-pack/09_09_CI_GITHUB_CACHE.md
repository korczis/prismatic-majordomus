# PAGE 09 — GitHub Actions reuse, CI planning and artifact proof

**Claude Code Opus/Fable execution prompt.**

Integrate derivation cache with existing `.ai/repo/ci/gates.yaml`, `scripts/ci-plan`, `validate.yml`, pages workflow and artifact lifecycle. Reuse the same canonical graph/keys; GitHub YAML remains an adapter, never a second mapping from changed paths to generators. Cache local derivations with appropriately scoped GitHub Actions cache, immutable uploaded artifacts or compatible remote CAS according to measurements and trust boundaries.

CI must still judge exact source SHA and staged/generated output drift. Cache hit cannot by itself equal a pass: verify input fingerprint, generator identity, output hashes and current committed content. Fallback on missing remote cache. Never restore PR-controlled cache into a privileged release context without trust isolation. Measure transfer cost, cache upload/download, no-op validation and project-wide cold cost. Avoid whole `target/` archives by default; separate compiler and derivation cache.

Add CI contract tests ensuring a new node appears in CI selection automatically, changed model invalidates dependent nodes, and missing artifact fails, not skips. Ensure required checks and branch protection are not weakened. Exit gate: successful workflow/check artifacts for actual current SHA, with measured warm/cold differences and no generated drift.


## Mandatory Majordomus discipline
Read `AGENTS.md`, `CLAUDE.md`, applicable `.ai/**` rules, current source, tests and relevant ADRs before editing. Use the current default branch and compare with unfinished PRs/worktrees first. Do not presume paths or APIs remain unchanged. Reuse the existing Rust CLI/capability registry, generator framework, `scripts/derive`, `scripts/derive-check`, CI planner, evidence types, and projection architecture. Never create a parallel source of truth, manual duplicate inventory, or cosmetic documentation-only enforcement. Stay inside this phase's scope. No privileged branch-protection bypass, force push, unsafe cleanup, release tag, or production deploy without existing explicit authorization. Always distinguish implemented, tested, integrated, published, and live-verified. If access is unavailable, produce an exact blocker and verify everything else. Do not claim time savings without measurements.

## Standard completion handoff
At the end write a machine-readable and human-readable checkpoint in the repository's existing approved context/evidence mechanism: source HEAD and worktree state; files changed; exact test commands/results; relevant measured durations; pending blockers; invariants proven; next phase's prerequisite status. Commit and propose/integrate according to repository policy and checks; do not create a new PR if an existing one should be updated. After each change rerun the narrow relevant tests and generated-artifact drift checks. A green test for an old SHA is not proof for a new one.
