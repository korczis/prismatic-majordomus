# 04-DESIGN-SYSTEM-UX: Information architecture and truly mobile-first shell

# Majordomus UI implementation mission

You are Claude Code Opus/Fable operating in the CURRENT checkout of `github.com/korczis/prismatic-majordomus` or a repository into which Majordomus has been injected. Work autonomously within granted permissions. Read actual code first. Do not assume documentation claims equal implementation. Never invent files, routes, commands, APIs, architectural components, or successful test results.

## Non-negotiable governing constraints
- Discover and obey current `AGENTS.md`, `CLAUDE.md`, `.ai/**`, relevant doctrines, rules, ADRs, schemas, generated artifacts and repository-specific gates. Where instructions conflict, follow canonical precedence and document it.
- Single canonical model. UI is a derived/inferred/projected consumer, NEVER a parallel source of truth or a second task/intent/mesh engine.
- Reuse typed Rust capability/command registries, RepositoryEnvironment, REST/OpenAPI, MCP, existing Cockpit and data/evidence/provenance where they exist. Add missing canonical capabilities only at their rightful backend owner.
- Every production value comes from real scoped repository state or actual observations. No fake counters, sample peers, fabricated graphs, hardcoded business inventories or 'success' statuses. Empty/unavailable/error states must be truthful.
- No business rules, authorization, task transitions or policy evaluation in JavaScript. Client state may handle view, selection, filter, navigation and responsive affordances.
- Accessible, keyboard-operable, genuinely mobile-first, progressive enhancement, responsive drawers, usable touch targets, loading/retry/empty/error states.
- Secure local bind by default. State-changing operations require explicit intent, authorization/capability checks, origin/CSRF protection appropriate to architecture, audited outcome, and safe handling of untrusted repos. Never expose credentials, tokens, raw environment secrets, shell injection surfaces, or unauthenticated remote writes.
- No arbitrary shell execution endpoint. Operations are typed, allowlisted, validated, provenance-aware, with lifecycle/idempotency/concurrency and cancellation semantics where applicable.
- Use repository-native build/test/generate/check/deploy paths; do not create an unrelated toolchain just to get a green result. Preserve backwards compatibility unless deliberately versioned/migrated.
- Start each milestone with an audit table: current owner, canonical facts, read/write interfaces, missing gap, downstream consumers, security impact, migration, tests.
- Apply smallest vertical slice end to end. Do not stop at proposals if code changes are possible. Avoid speculative refactors. Prefer deletion of duplication to adapters.
- Test with realistic fixtures, negative cases, repository self-use and a minimal injected consumer repository. Prove UI/CLI/API/MCP parity for common domain facts.
- Observe existing versioning/release governance. Never claim released/deployed unless remote artifact and actual running endpoint are verified. If blocked by external credentials/access, leave precise evidence and no false green.

## Work loop
Inspect -> locate canonical owner -> articulate invariants -> change backend contract -> derive projection -> wire UI -> focused tests -> cross-surface tests -> docs -> enforcement -> run real gates -> report evidence. Coordinate with existing issues/milestones/session/mesh when available. Before creating new issues or PRs, detect duplicates. Keep commits coherent and small. Never weaken gates to make failures disappear.

## Completion report required for EVERY prompt
Deliver: implemented architecture with actual file paths; removed duplicate paths; API/CLI/MCP/UI routes or operations with typed schema ownership; changed files; tests commands and observed pass/fail counts; E2E self-host and injected repo proof; accessibility/mobile proof; security boundary and negative tests; performance baseline vs after; docs/ADR/gates; release/deploy artifacts if applicable; remaining blockers with exact reproduction. Claim only what evidence demonstrates.

## Mission-specific execution
Build an accessible, tasteful high-level human interaction shell using the repository's existing frontend conventions. Evaluate Flowbite/Tailwind, Alpine.js, ECharts and Cytoscape against existing dependencies; choose only libraries with actual consumer value. Design top-level navigation: Overview, Intent/Work, Mesh, Sessions, Quality, Knowledge/Governance, Operations; diagnostic details secondary. Author high-level concepts in the project vocabulary, not raw registry type names. Desktop: adaptive sidebar, breadcrumbs, global context selector and contextual command palette/search if existing search facilities permit. Mobile 320/360/390px: functional accessible drawer with overlay focus trap, Escape, backdrop close, proper scroll lock, sticky actions, keyboard support, no horizontal overflow, 44px-equivalent hit areas, reduced motion and sufficient contrast. Show clear provenance/freshness and 'unknown' not green on absent data. Loading skeletons must not masquerade as fake data; actionable empty, permission and error states. Keep navigational inventories derived from capabilities; ensure direct links and reload work. Default view shows useful top-level summary, blockers, recommended next actions and task progress, not 40 widgets. Acceptance: automated a11y audit + keyboard script + mobile viewport screenshots + happy/error/empty tests + no fake content scanners.

## Mandatory handoff
Record canonical milestone/issue/intent linkage where available, source change IDs, test evidence and durable next milestone context. The next stage must be able to resume without this conversation.

## Pack v1.1 cross-cutting gate: derivation and zero manual sync
Before closing this milestone, apply `02A-ELIMINATE-MANUAL-MAINTENANCE.md` to the touched domains. Locate canonical owner, remove consumer-side redundant catalogs and handwritten synchronizations, reuse deterministic typed projections, and provide zero-registration propagation + negative authorization evidence. New manually maintained mirror inventories and frontend business-state logic fail acceptance. Run the anti-drift validation via existing canonical check/CI paths, and record exact observed results; preserve legitimate authoritative human declarations.
