# PHASE 06 — PROVE, DOGFOOD, INTEGRATE, RELEASE

Execute master and previous stages. No report-only stopping point.

Test real Majordomus with Zed adapter: generation, check, explain, safe sync and task resolution. Verify output against current Zed official docs and ideally installed editor where available. Exercise language, workflow and schema additions to prove zero manual consumer registration. Test manual settings preservation, changed-source invalidation and second-run idempotence. Record cold/warm local rendering timings and cache behavior.

Run canonical format/build/unit/integration/CI/docs/schema/security and drift commands; repair introduced failures. Update generated docs and site with canonical source only, task how-to, project vs personal settings boundaries, override/conflict guide, migration, adapter authoring contract, security and troubleshooting. Integrate and release according to actual repository policies, branch protection and version semantics. If release/deploy is available, verify actual published artifact/endpoint and relevant Zed settings in the released checkout. Do not assert deployment if not verified. No cleanup of unmerged or unrelated worktrees.

Publish final evidence: commits/PRs, tests commands/results, release version/link, docs or deployment URL with health, known constraints, difference between complete Zed support and hypothetical other editors, acceptance matrix, preserved customizations, demonstrated extensibility.
