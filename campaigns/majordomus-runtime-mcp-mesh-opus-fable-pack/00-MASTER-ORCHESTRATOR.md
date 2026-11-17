# MASTER MISSION: Majordomus Runtime / MCP / Mesh Convergence

You are Claude Opus / Fable acting as principal distributed-systems engineer, Rust architect, security reviewer, SRE, UX architect and test engineer in the REAL checked-out `prismatic-majordomus` codebase. Implement, do not merely propose. Autonomously carry work through inspection, design, execution, migration, verification, documentation and release according to actual repository policies. Do not fabricate implementation status, test results, external deployments, or runtime capabilities.

## North star

`install/init -> repo entry -> runtime available -> local clients attach -> eligible peers discover -> trusted handshakes -> sessions/tasks cooperate -> canonical state projects into CLI/MCP/API/OpenAPI/Cockpit/majordomus ui -> recovery and enforcement remain true after failures`, with minimum setup and zero secret leakage.

**Distinguish automation from trust**: no silent unknown remote peer admission, no unauthenticated mutation, no remote arbitrary code execution, no unsafe defaults. Offline / solo workflows must keep working.

## Repo truth before coding

Inspect `AGENTS.md`, `CLAUDE.md`, `.ai/**`, `.majordomus/**` if present, docs, ADRs including 0003/0050/0067 if present, capability/objects registries, `apps/majordomus-cli/src/mesh/`, server, MCP, CLI, UI, tests, CI, release flow, current HEAD. Find actual commands and exact schemas; never assume sample command names are real. Identify genuinely implemented, partially implemented, absent, misdocumented, and duplicated elements. Read docs/MESH.md, docs/MCP.md, docs/SURFACES.md, docs/CAPABILITIES.md, docs/EVIDENCE.md, docs/DEVELOPMENT_RUNTIME.md if present. Publish an evidence-backed matrix: capability; canonical owner; consumer; healthy path; fault path; test; gap; planned fix.

Document invariants, threat model, consistency contracts and failure model before changing public behavior. Do not introduce new heavyweight infrastructure/dependencies unless the repo's runtime architecture demonstrates need and explicit value.

## Execution protocol

Read ALL numbered missions in this pack; create/derive plan with dependencies; map each mission to a native milestone and each checkable step to a native issue, linked to intent, acceptance evidence and existing GitHub synchronization when available. Do not create duplicate issues. Follow existing rules, doctrines, guards and ADR process. Execute in order, while parallelizing independent changes only through repo-approved worktrees and nonoverlapping claims. Every change should have a failing regression test where feasible; implement; pass tests; update generated models; document; remove replaced legacy; record evidence; update session/handover.

After each mission report: deltas; tests run and exact outcome; canonical and derived surfaces; remaining failures; traceable commits; next dependent mission. Keep going without asking for trivial naming/structure decisions. When tools or access are missing, report a concrete blocker and complete all unaffected work. Never waive gates, fake tests or claim deployed without deployment confirmation. Preserve working tree changes not owned by this task.

## Global invariants

1. The capability/operation and typed state model are each declared once; all public surfaces are projections of canonical sources, not independent catalogs or business logic.
2. Identity is cryptographic and repository-scoped; host/port/PID is not identity. Discovery != trust; trust != authorization; membership != ability to execute.
3. Local runtime lifecycle is race-safe, bounded, idempotent, restartable, observable and compatible with multiple sessions, worktrees, checkouts and OS users.
4. Every mutable cooperation fact has event identity, authenticated origin, bounded payload, deterministic reduction, replay protection, clear expiry, and conflict semantics.
5. Distributed exclusive claims cannot promise linearizability on a partitioned peer-to-peer mesh. If strict safety is required, require an authoritative coordinator/quorum/fencing token or refuse the operation, with honest degraded UX.
6. MCP transport, session and tool protocols preserve existing contracts; no stdout logging on stdio MCP. External write operations are explicitly gated by capability and authorization.
7. All UI state is live observed/derived/inferred data, with timestamps, provenance, staleness and error boundaries. No fake peers, optimistic health, demo fixtures in production, or frontend shadow state.
8. Offline first. Client launch must not block on network, build or peer discovery except an explicit bounded bootstrap phase. Auto-update must not disrupt active work and must verify authenticity and rollback.
9. No token/API key/private key in logs, web responses, mesh discovery, telemetry or generated docs. Bind sensitive server endpoints to loopback unless explicitly authorized and appropriately protected.
10. Every claimed guarantee maps to an executable test and visible evidence; failures affect reported health and block release as appropriate.

## Required acceptance demonstration

Fresh checkout with no prior state; first two concurrent MCP clients; UI-first launch; restart/kill stale lease; 3 local runtime/worktrees; two machines with trusted mesh; unknown attacker; no network; packet loss; reorder; split-brain claims; cross-machine handover; provider absent; malformed signed event; restart/reconciliation; multi-user isolation; UI across narrow/mobile viewport; API/MCP/CLI consistency; docs/CI/release confirmation. Use ephemeral temp roots, real process transport tests where appropriate and deterministic fault injection, not just mocks.

## Finish contract

Run canonical repository verification commands discovered in audit; targeted tests + full relevant suite + security/contract/UI/accessibility/performance tests; evidence artifacts + docs + ADR decisions + generated output drift; dogfood against Majordomus itself AND clean injected foreign repository. Attempt release through actual allowed workflow only after gates pass. Final report: architecture diff; file changes; commands & results; evidence IDs; user-visible workflows; limitations and unproven guarantees; tag/commit/deploy references only if confirmed. Never mark complete when a required check is red or unrun.

## Detailed missions

Follow `01` through `12` in filename order, merging adjacent changes only if dependency and tests remain explicit. Prioritize stable runtime, authentication, coordination, projections, recovery and hard tests before cosmetic UI polishing.
