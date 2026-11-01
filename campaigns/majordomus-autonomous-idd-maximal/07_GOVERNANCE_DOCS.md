# PHASE 07 — `.ai`, ENFORCEMENT, USE CASES, DOCS AND PUBLICATION

Update existing policy owners, never derived copies. Discover applicable rule/doctrine/schema/workflow conventions under `.ai/repo/`; modify only the lowest canonical source needed and validate all generated material. Preserve `.ai` as declarative context, not executable runtime or private memory. Respect manifest discovery and scoped README precedence. Do not make deployment behavior or UI layout an unbounded policy document if existing types enforce it better.

Machine-enforced invariants (extend existing rules/doctrines/gates instead of duplicating):
- Every user-visible effect is a canonical typed capability with authorization, audit and idempotency/reconciliation semantics.
- Every issue has an accountable intent/criterion or explicitly justified maintenance origin.
- Every execution is claimed/scoped and recoverable; no duplicate side effect after replay.
- No issue/milestone/intent marked complete without applicable fresh evidence.
- No auto merge bypasses required GitHub checks/reviews/branch rules.
- No release without tested merged source and reproducible artifact; no deployed/satisfied claim without observed verification.
- No manually maintained cross-surface catalog/order/route/status mapping when derivable from canonical metadata.
- No raw secret, local prompt, token or private session context in public surfaces or generated docs.
- Every changed capability has executable use-case coverage and a corresponding UI/API/MCP projection when applicable.
- Generated policies/bootstrap files, OpenAPI and docs must be free from source drift.
- No unreviewed dangerous effects through mesh or UI; explicit project-scoped safety profile.

Add to existing CI and local canonical check path: schema validation, capability generation drift, `.ai` context validation, use-case coverage, UI route/link integrity, docs link checks, reproducible build, integration unit tests, E2E recovery test, release/deploy dry-run. Fast precommit path vs slower integration suite consistent with current CI policy. Diagnose violations with rule ID, failing entity, canonical owner and recommended fix. Rules must be executed, not decorations in Markdown.

Documentation updates (respect current docs hierarchy and front matter): `docs/PLANNING.md` (intent planner and synthesis), `docs/EXECUTIONS.md` (coordinator/reconciliation), `docs/MESH.md` (execution boundary vs coordination and failure semantics), `docs/WORKTREES.md` (ownership/cleanup), `docs/DELIVERY.md` (integration), `docs/RELEASE.md` (evidence/deploy/rollout), `docs/EVIDENCE.md` (provenance/freshness), `docs/CAPABILITIES.md` and `docs/MCP.md` (generated discovery), `docs/UI.md`, `docs/COCKPIT.md`, `docs/DESIGN_SYSTEM.md` (operation and UI model), `docs/USE_CASES.md`, `docs/DOGFOODING.md`, `docs/PERFORMANCE.md`, `docs/INTEGRATION.md` as appropriate. Add or update ADR only when repo ADR policy requires an architectural decision. Keep normative prose concise and link canonical schema/source; generate inventory tables from source.

Public site/GitHub Pages: adapt existing derived publication mechanism. Include approachable project-oriented explanation, working interactive walkthrough of example execution (clearly sample vs live), deep links, diagram of intent pipeline, honest verified capabilities/limitations, quickstart install and `majordomus ui`, API/OpenAPI/MCP links, and versioned release notes. No fabricated green badges, performance numbers or remote peers. Site claim validation must compare asserted capabilities to executable use cases and deployment evidence where machinery exists. Redact private repo state automatically.

Dogfooding: install/inject `.ai` in a sample external repo without assuming the Majordomus source tree; verify discovery and source context, UI project switching, remote mesh optional/degraded mode, no leaked local state, and all generated surfaces. Record migration policy for existing `.ai` projects. Run generate/check/doctor/usecase coverage and docs build. Deliver documented extension story: new intent criterion/capability manifests once; CLI/API/MCP/UI/docs update as consumers automatically.

Acceptance: validation fails on a deliberately introduced hardcoded transport list, stale evidence, unsafe unapproved deploy and duplicate claim; docs/site derive from the same canonical implementation rather than claim aspirations.
