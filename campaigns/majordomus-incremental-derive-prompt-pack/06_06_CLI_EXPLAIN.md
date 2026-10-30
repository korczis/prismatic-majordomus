# PAGE 06 — Operator commands: plan, explain, status, metrics, doctor

**Claude Code Opus/Fable execution prompt.**

Expose incremental derivation through the existing canonical command/capability hierarchy. Reconcile current names before adding new ones. Required semantics: `derive plan` (read-only deterministic node plan), `derive explain <node|artifact>` (precise invalidation/provenance), `derive status` (current/reused/dirty/missing), `derive run` or canonical existing `scripts/derive` adapter, `derive check` (strict read-only), `derive stats` (execution/cache timings), `derive cache gc` (explicit mutating operation). Preserve old commands and scripts as thin compatible adapters with tested exit codes.

Machine JSON should be schema-derived, stable and record per-node key, state, inputs and reasons without secrets. Human output should be concise and stream progress for slow nodes. Provide --full, --no-cache, --json where consistent with repo conventions. Show why an unrelated file changed no nodes, why the borrowed binary is rejected, and why a specific output was restored or generated.

Ensure API/OpenAPI/MCP projections derive from the existing capability registry where already supported, with effect and authorization classifications. Test help/completions/CLI examples, non-TTY output, failure codes, dry-run no mutation and deterministic JSON. Exit gate: user can diagnose any derive decision without reading source code or logs.


## Mandatory Majordomus discipline
Read `AGENTS.md`, `CLAUDE.md`, applicable `.ai/**` rules, current source, tests and relevant ADRs before editing. Use the current default branch and compare with unfinished PRs/worktrees first. Do not presume paths or APIs remain unchanged. Reuse the existing Rust CLI/capability registry, generator framework, `scripts/derive`, `scripts/derive-check`, CI planner, evidence types, and projection architecture. Never create a parallel source of truth, manual duplicate inventory, or cosmetic documentation-only enforcement. Stay inside this phase's scope. No privileged branch-protection bypass, force push, unsafe cleanup, release tag, or production deploy without existing explicit authorization. Always distinguish implemented, tested, integrated, published, and live-verified. If access is unavailable, produce an exact blocker and verify everything else. Do not claim time savings without measurements.

## Standard completion handoff
At the end write a machine-readable and human-readable checkpoint in the repository's existing approved context/evidence mechanism: source HEAD and worktree state; files changed; exact test commands/results; relevant measured durations; pending blockers; invariants proven; next phase's prerequisite status. Commit and propose/integrate according to repository policy and checks; do not create a new PR if an existing one should be updated. After each change rerun the narrow relevant tests and generated-artifact drift checks. A green test for an old SHA is not proof for a new one.
