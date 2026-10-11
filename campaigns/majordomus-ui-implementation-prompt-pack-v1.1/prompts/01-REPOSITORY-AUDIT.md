# 01-REPOSITORY-AUDIT: Trace actual capabilities and UI debt

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
Inspect current master checkout at the actual commit, git status, `.ai`, Rust CLI, environment module, capability/feature registries, existing Cockpit routing/templates, server/API/OpenAPI/MCP definitions, schemas, site/frontend assets, install script, just modules, release pipeline and tests. Inspect existing command named `ui`, if any; never override without compatibility analysis. Read docs/how-it-works and current docs as hypotheses; verify code and executable behavior. Produce machine-readable `UI_CAPABILITY_MATRIX` in an appropriate existing docs/registry location with per capability: canonical source, read query, safe action, event stream/polling, auth, provenance, list/detail, status (working/partial/absent), test, UI screen. Cover intent, issues/milestones, sessions, mesh peers/discovery/tasks, environment, governance, knowledge, tests/coverage/evidence, releases/deploys, diagnostics, self update. Find manual Cockpit route mapping, frontend hardcoding, shell/Rust naming collisions, stale docs. Record keep/extend/retire decision for Cockpit. Resolve core architecture: whether `majordomus ui` delegates to existing server, where projections live, how it selects repo root. Create ADR only under repository policy. Establish executable baseline tests and small representative injected fixture. Acceptance: no imaginary capabilities; explicit gap list, runnable spike proving existing server serves one real data projection; regression baseline documented.

## Mandatory handoff
Record canonical milestone/issue/intent linkage where available, source change IDs, test evidence and durable next milestone context. The next stage must be able to resume without this conversation.

## Pack v1.1 cross-cutting gate: derivation and zero manual sync
Before closing this milestone, apply `02A-ELIMINATE-MANUAL-MAINTENANCE.md` to the touched domains. Locate canonical owner, remove consumer-side redundant catalogs and handwritten synchronizations, reuse deterministic typed projections, and provide zero-registration propagation + negative authorization evidence. New manually maintained mirror inventories and frontend business-state logic fail acceptance. Run the anti-drift validation via existing canonical check/CI paths, and record exact observed results; preserve legitimate authoritative human declarations.
