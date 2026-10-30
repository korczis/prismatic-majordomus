# PAGE 08 — Cross-worktree concurrency, leases and execution safety

**Claude Code Opus/Fable execution prompt.**

Audit machine-wide `derive` lock, `lib/machine_lock.sh`, process-tree termination and current shared CARGO_TARGET_DIR behavior. Design safe concurrency: shared immutable CAS, per-worktree mutable staging, per-key single-flight compute and short critical sections. Preserve existing hard guarantee that two conflicting derive writes cannot interleave; do not weaken locks merely to claim parallel execution. Distinguish expensive upstream computation from final worktree commits.

Ensure one worktree's derive cannot overwrite another's artifacts or use incompatible staged file membership. Handle concurrent Rust rebuilds, cache stampedes, interrupted generators, orphan processes, external `MAJORDOMUS_BIN`, path normalization and cross-OS cache separation. Add deterministic stress tests with two/three simultaneous worktrees, killed owner, competing same-key computation and distinct-key independent computations. Measure throughput and lock wait rather than only single-run latency.

Exit gate: no corrupted cache or worktree, no stale outputs, no deadlocks, measurable shared computation benefit, and bounded process lifecycle.


## Mandatory Majordomus discipline
Read `AGENTS.md`, `CLAUDE.md`, applicable `.ai/**` rules, current source, tests and relevant ADRs before editing. Use the current default branch and compare with unfinished PRs/worktrees first. Do not presume paths or APIs remain unchanged. Reuse the existing Rust CLI/capability registry, generator framework, `scripts/derive`, `scripts/derive-check`, CI planner, evidence types, and projection architecture. Never create a parallel source of truth, manual duplicate inventory, or cosmetic documentation-only enforcement. Stay inside this phase's scope. No privileged branch-protection bypass, force push, unsafe cleanup, release tag, or production deploy without existing explicit authorization. Always distinguish implemented, tested, integrated, published, and live-verified. If access is unavailable, produce an exact blocker and verify everything else. Do not claim time savings without measurements.

## Standard completion handoff
At the end write a machine-readable and human-readable checkpoint in the repository's existing approved context/evidence mechanism: source HEAD and worktree state; files changed; exact test commands/results; relevant measured durations; pending blockers; invariants proven; next phase's prerequisite status. Commit and propose/integrate according to repository policy and checks; do not create a new PR if an existing one should be updated. After each change rerun the narrow relevant tests and generated-artifact drift checks. A green test for an old SHA is not proof for a new one.
