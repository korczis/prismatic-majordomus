# PAGE 01 — Repository audit, DAG inventory and baseline

**Claude Code Opus/Fable execution prompt.**

Perform a read-only forensic audit of `scripts/derive`, `scripts/derive-check`, `scripts/generate-site-data`, `scripts/gitattributes`, crate generation (`generation.rs`, `build.rs`), all `majordomus generate` producers, generated manifest code, CI workflows/gates and existing cache infrastructure. The known historical process is Stage A Rust generate; Stage B generate-site-data; Stage C Rust generate again; Stage D gitattributes, all under a machine-wide derive lock. Validate this against HEAD. Discover the exact dependency and output contract of every projection, not only stage names.

Create a machine-readable `producer -> canonical inputs -> outputs -> downstream consumers -> algorithm implementation` inventory in the existing documentation/registry model. Classify dependencies: tracked working-tree content, index membership, Git metadata/ref, environment, toolchain, compiler binary-generation identity, non-file data. Identify hidden dynamic reads/globs and non-hermetic tools. Explicitly inspect whether site generator executes use-case scenarios and whether these have effects.

Measure cold, warm, no-op, one doctrine, one Rust source, one site source, one deleted generated output, staged new file, and changed tracked unstaged file. Measure separately Cargo build, stage A/B/C/D, hash discovery, filesystem writes, lock wait, and generated check. Capture environment/hardware and exact SHA. No destructive `cargo clean` on user assets; isolate cold runs in disposable checkout or cache namespace.

Deliver an evidence-based bottleneck table (p50/p95 where available), critical path, top ten highest-ROI changes, dependency correctness hazards, and a graph completeness assessment. Exit gate: full input/output inventory and reproducible baseline with no speculative measurements.


## Mandatory Majordomus discipline
Read `AGENTS.md`, `CLAUDE.md`, applicable `.ai/**` rules, current source, tests and relevant ADRs before editing. Use the current default branch and compare with unfinished PRs/worktrees first. Do not presume paths or APIs remain unchanged. Reuse the existing Rust CLI/capability registry, generator framework, `scripts/derive`, `scripts/derive-check`, CI planner, evidence types, and projection architecture. Never create a parallel source of truth, manual duplicate inventory, or cosmetic documentation-only enforcement. Stay inside this phase's scope. No privileged branch-protection bypass, force push, unsafe cleanup, release tag, or production deploy without existing explicit authorization. Always distinguish implemented, tested, integrated, published, and live-verified. If access is unavailable, produce an exact blocker and verify everything else. Do not claim time savings without measurements.

## Standard completion handoff
At the end write a machine-readable and human-readable checkpoint in the repository's existing approved context/evidence mechanism: source HEAD and worktree state; files changed; exact test commands/results; relevant measured durations; pending blockers; invariants proven; next phase's prerequisite status. Commit and propose/integrate according to repository policy and checks; do not create a new PR if an existing one should be updated. After each change rerun the narrow relevant tests and generated-artifact drift checks. A green test for an old SHA is not proof for a new one.
