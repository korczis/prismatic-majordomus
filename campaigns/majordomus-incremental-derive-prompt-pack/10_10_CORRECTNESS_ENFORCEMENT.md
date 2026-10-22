# PAGE 10 — Differential oracle, adversarial tests, .ai doctrines

**Claude Code Opus/Fable execution prompt.**

Codify the invariant: `incremental_derive(inputs) == clean_full_derive(inputs)` at the byte level for all owned artifacts, including creation/deletion. Use disposable Git fixtures across scenario matrix: no-op, staged/unstaged changes, source edit, new doctrine, renamed indexed object, deleted document, changed schema, altered generator binary, corrupted cache, altered output, concurrent derives, symlink, index/worktree disagreement, environmental input, CI artifact loss. Verify exit-code compatibility with existing `derive-check` and missing output failure semantics.

Introduce executable `.ai` enforcement through current rules/doctrine/gate owners: every generated output has one producer; every producer declares complete dependencies; no graph cycles; no hand-maintained graph copies in consumers; stale borrowed generator cannot write; cached proof checks bytes; clean full vs incremental equivalence; read-only `--check`; invalid cache fails safe; performance budgets measured but not used as flaky timeout guesses.

Provide mutation tests: deliberately omit dependency edge, tamper output, delete manifest, corrupt CAS entry and ensure gates go red. Run focused and cross-surface tests. Do not make tests pass by updating golden files automatically without reviewing correctness. Exit gate: adversarial suite green, graph complete, intentional violations detected, original strict gates retained.


## Mandatory Majordomus discipline
Read `AGENTS.md`, `CLAUDE.md`, applicable `.ai/**` rules, current source, tests and relevant ADRs before editing. Use the current default branch and compare with unfinished PRs/worktrees first. Do not presume paths or APIs remain unchanged. Reuse the existing Rust CLI/capability registry, generator framework, `scripts/derive`, `scripts/derive-check`, CI planner, evidence types, and projection architecture. Never create a parallel source of truth, manual duplicate inventory, or cosmetic documentation-only enforcement. Stay inside this phase's scope. No privileged branch-protection bypass, force push, unsafe cleanup, release tag, or production deploy without existing explicit authorization. Always distinguish implemented, tested, integrated, published, and live-verified. If access is unavailable, produce an exact blocker and verify everything else. Do not claim time savings without measurements.

## Standard completion handoff
At the end write a machine-readable and human-readable checkpoint in the repository's existing approved context/evidence mechanism: source HEAD and worktree state; files changed; exact test commands/results; relevant measured durations; pending blockers; invariants proven; next phase's prerequisite status. Commit and propose/integrate according to repository policy and checks; do not create a new PR if an existing one should be updated. After each change rerun the narrow relevant tests and generated-artifact drift checks. A green test for an old SHA is not proof for a new one.
