# 00-ORCHESTRATOR: Autonomous execution conductor

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
Your role: execute this ENTIRE pack, not merely summarize it. Read `contracts/GLOBAL_CONTRACT.md`, README and prompts 01-10 including 02A; discover actual repository instructions before edits. Reconcile against already-completed functionality; avoid rewriting working implementations. Create or map milestones/issues via Majordomus existing canonical workflow, sync to GitHub only where configured/authorized, preserving traceability and no duplicates. Complete 02A after 02 and before 03-10, migrate real legacy duplicates and install anti-drift enforcement before declaring any downstream milestone complete. Work in dependency order; within each milestone implement a working end-to-end vertical slice, test, document and land using repo convention. Parallelize only independent, non-overlapping tasks, using mesh peers if actually available; never assert remote peers exist unless observed. Make every milestone resumable via typed session/handover and durable evidence rather than a personal scratchpad. Enforce iteration cap per failing approach and escalate to alternate implementation when needed; do not endlessly loop. Never alter protected branches or bypass approvals to manufacture completion. Prioritize runnable app and user flows ahead of visuals. Deliver closure with pass/fail evidence and unresolved external blockers. Do not stop after writing a plan. See specific milestone prompts for scope and acceptance.

## Mandatory handoff
Record canonical milestone/issue/intent linkage where available, source change IDs, test evidence and durable next milestone context. The next stage must be able to resume without this conversation.
