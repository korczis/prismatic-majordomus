# PAGE 11 — Derived UI, generated documentation and GitHub Pages

**Claude Code Opus/Fable execution prompt.**

Present the derivation graph and performance via existing Majordomus UI/Cockpit projections, without a frontend duplicate scheduler. Add high-level views for graph status, nodes by state, affected outputs, last run, cache hit ratio, critical path, invalidation explanations and failures. Use existing routing/templates/live execution updates. Mobile first, accessible and interactive; do not convert this phase into a frontend rewrite. Mutating controls, if any, go through canonical effect authorization.

Generate/update documentation: architecture, source-of-truth rules, graph model, hashing algorithm, cache validation, cross-worktree use, CI integration, extension guide, troubleshooting, rollback, benchmarks and benchmark caveats. Ensure docs/Pages generator uses the same canonical registry; no manual node inventories. Verify links, OpenAPI/MCP schema parity where applicable, docs build and GitHub Pages generated current-state checks. Public content must not expose secret paths or cache keys containing secrets.

Publish only via existing authorized Pages pipeline and verify served commit when possible. If no deployment permission, report exact status and do not claim live publication. Exit gate: CLI/API/MCP/UI consistent, generated docs current, Pages publish or precise blocker, responsive interactions tested.


## Mandatory Majordomus discipline
Read `AGENTS.md`, `CLAUDE.md`, applicable `.ai/**` rules, current source, tests and relevant ADRs before editing. Use the current default branch and compare with unfinished PRs/worktrees first. Do not presume paths or APIs remain unchanged. Reuse the existing Rust CLI/capability registry, generator framework, `scripts/derive`, `scripts/derive-check`, CI planner, evidence types, and projection architecture. Never create a parallel source of truth, manual duplicate inventory, or cosmetic documentation-only enforcement. Stay inside this phase's scope. No privileged branch-protection bypass, force push, unsafe cleanup, release tag, or production deploy without existing explicit authorization. Always distinguish implemented, tested, integrated, published, and live-verified. If access is unavailable, produce an exact blocker and verify everything else. Do not claim time savings without measurements.

## Standard completion handoff
At the end write a machine-readable and human-readable checkpoint in the repository's existing approved context/evidence mechanism: source HEAD and worktree state; files changed; exact test commands/results; relevant measured durations; pending blockers; invariants proven; next phase's prerequisite status. Commit and propose/integrate according to repository policy and checks; do not create a new PR if an existing one should be updated. After each change rerun the narrow relevant tests and generated-artifact drift checks. A green test for an old SHA is not proof for a new one.
