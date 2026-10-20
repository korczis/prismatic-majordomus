# PAGE 07 — Granular site and docs invalidation

**Claude Code Opus/Fable execution prompt.**

Investigate the actual behavior of `scripts/generate-site-data`, `site/data/generated`, `docs/generated`, `docs/SITE_CLAIMS.md`, `docs/PLAN_STATUS.md`, benchmark matrices and index-derived site registry. Map producer/consumer relationships and identify heavyweight scenario runs. Split coarse stage A/B/C into meaningful graph nodes according to real dependencies; minimize changes to established source ownership.

Make a doctrine/rule edit invalidate only relevant policies, registries, site data and pages, not every independent benchmark or provider bootstrap. Make Rust code changing generator implementation invalidate impacted projections. Preserve composite site input hashes, GitHub Pages expected bytes, published source manifest, and `gitattributes` merge derived file list. Do not infer 'unaffected' merely from filename extension: use actual template/shared-layout dependencies, with conservative fallbacks.

Compare clean full vs incremental for: source-only CLI, OpenAPI schema, rules, provider metadata, one docs page, shared site template, added/deleted indexed document, and generated artifact tampering. Assess whether per-page granularity beats its indexing overhead; retain coarser nodes if faster. Exit gate: no missing/stale page, navigation, generated registry or merged file semantics.


## Mandatory Majordomus discipline
Read `AGENTS.md`, `CLAUDE.md`, applicable `.ai/**` rules, current source, tests and relevant ADRs before editing. Use the current default branch and compare with unfinished PRs/worktrees first. Do not presume paths or APIs remain unchanged. Reuse the existing Rust CLI/capability registry, generator framework, `scripts/derive`, `scripts/derive-check`, CI planner, evidence types, and projection architecture. Never create a parallel source of truth, manual duplicate inventory, or cosmetic documentation-only enforcement. Stay inside this phase's scope. No privileged branch-protection bypass, force push, unsafe cleanup, release tag, or production deploy without existing explicit authorization. Always distinguish implemented, tested, integrated, published, and live-verified. If access is unavailable, produce an exact blocker and verify everything else. Do not claim time savings without measurements.

## Standard completion handoff
At the end write a machine-readable and human-readable checkpoint in the repository's existing approved context/evidence mechanism: source HEAD and worktree state; files changed; exact test commands/results; relevant measured durations; pending blockers; invariants proven; next phase's prerequisite status. Commit and propose/integrate according to repository policy and checks; do not create a new PR if an existing one should be updated. After each change rerun the narrow relevant tests and generated-artifact drift checks. A green test for an old SHA is not proof for a new one.
