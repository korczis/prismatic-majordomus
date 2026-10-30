# PAGE 02 — Typed canonical DerivationGraph and discoverability

**Claude Code Opus/Fable execution prompt.**

Implement a typed derivation graph in the existing Rust generation subsystem, not a second shell scheduler. Model stable node identity, input selectors (file, directory, glob, index-membership, Git, toolchain, environment, virtual), exact outputs, dependency edges, operation version, deterministic ordering, tool identity, execution requirements and output ownership. Derive descriptors from existing registry/generator metadata; add explicit metadata only where inference cannot be sound.

Build a topological sort with stable tie-break and cycle detection, unknown input refusal, duplicate output producer refusal, undeclared/missing generator refusal, and explanation of all edges. Treat stage A -> B -> C -> D as a legacy compatibility contract while migrating to node edges. Separate nodes by expensive meaningful producer, not one per line or whole repository. Ensure stage C cannot accidentally read its own outputs; handle multi-output nodes atomically.

Define JSON Schema from canonical Rust types and expose the graph through current CLI structured output/capability mechanisms. New generator discovered through canonical registry must appear in graph without edits to CLI/UI/CI inventories. Test permutations, cycles, duplicate outputs, missing outputs, deterministic graph snapshots, unrelated addition stability, topology, and minimum existing graph coverage. Exit gate: production graph inventory agrees with legacy artifacts; no legacy producer is silently dropped.


## Mandatory Majordomus discipline
Read `AGENTS.md`, `CLAUDE.md`, applicable `.ai/**` rules, current source, tests and relevant ADRs before editing. Use the current default branch and compare with unfinished PRs/worktrees first. Do not presume paths or APIs remain unchanged. Reuse the existing Rust CLI/capability registry, generator framework, `scripts/derive`, `scripts/derive-check`, CI planner, evidence types, and projection architecture. Never create a parallel source of truth, manual duplicate inventory, or cosmetic documentation-only enforcement. Stay inside this phase's scope. No privileged branch-protection bypass, force push, unsafe cleanup, release tag, or production deploy without existing explicit authorization. Always distinguish implemented, tested, integrated, published, and live-verified. If access is unavailable, produce an exact blocker and verify everything else. Do not claim time savings without measurements.

## Standard completion handoff
At the end write a machine-readable and human-readable checkpoint in the repository's existing approved context/evidence mechanism: source HEAD and worktree state; files changed; exact test commands/results; relevant measured durations; pending blockers; invariants proven; next phase's prerequisite status. Commit and propose/integrate according to repository policy and checks; do not create a new PR if an existing one should be updated. After each change rerun the narrow relevant tests and generated-artifact drift checks. A green test for an old SHA is not proof for a new one.
