# PAGE 04 — Incremental scheduler, skip logic and full fallback

**Claude Code Opus/Fable execution prompt.**

Implement an incremental execution planner atop the typed graph, retaining current lock/preflight and interruption/orphan safety. The planner computes dirty nodes from input keys, dependency keys, missing/corrupt outputs, operation version and explicit force flags; marks nodes REUSE, RESTORE, REGENERATE, INVALID, BLOCKED with concrete reasons. Execute topological ready sets; only parallelize nodes with verified disjoint mutable inputs/outputs and safe process behavior. Do not blindly parallelize current phases under a global lock.

A true warm no-op must not re-run Rust generators or `generate-site-data`; identify whether Cargo build itself can be skipped safely using current generation guard and verified executable. `derive-check` stays read-only and fail-closed; missing outputs and stale output bytes must invalidate a cache hit. Atomic output staging with rename and rollback of incomplete batches; preserve untracked preflight, generated-only policy and exact exit codes where public contracts depend on them.

Keep `derive --full` or existing equivalent as byte-correct oracle and emergency fallback. Test interrupted execution, partial output writes, no-op, single node, transitive invalidation, missing generated output, tampered output, multiple generators, dry-run purity and cancellation. Benchmark at least no-op and isolated changes. Exit gate: incremental and full produce identical tracked generated file contents on representative inputs.


## Mandatory Majordomus discipline
Read `AGENTS.md`, `CLAUDE.md`, applicable `.ai/**` rules, current source, tests and relevant ADRs before editing. Use the current default branch and compare with unfinished PRs/worktrees first. Do not presume paths or APIs remain unchanged. Reuse the existing Rust CLI/capability registry, generator framework, `scripts/derive`, `scripts/derive-check`, CI planner, evidence types, and projection architecture. Never create a parallel source of truth, manual duplicate inventory, or cosmetic documentation-only enforcement. Stay inside this phase's scope. No privileged branch-protection bypass, force push, unsafe cleanup, release tag, or production deploy without existing explicit authorization. Always distinguish implemented, tested, integrated, published, and live-verified. If access is unavailable, produce an exact blocker and verify everything else. Do not claim time savings without measurements.

## Standard completion handoff
At the end write a machine-readable and human-readable checkpoint in the repository's existing approved context/evidence mechanism: source HEAD and worktree state; files changed; exact test commands/results; relevant measured durations; pending blockers; invariants proven; next phase's prerequisite status. Commit and propose/integrate according to repository policy and checks; do not create a new PR if an existing one should be updated. After each change rerun the narrow relevant tests and generated-artifact drift checks. A green test for an old SHA is not proof for a new one.
