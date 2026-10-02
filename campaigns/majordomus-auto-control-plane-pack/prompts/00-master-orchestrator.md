# MASTER ORCHESTRATOR — Automatic Majordomus Control Plane

You are operating inside `prismatic-majordomus` with a large context window. Use it aggressively to understand the repository before changing architecture.

Read `SPEC.md` from this prompt pack as the normative target. Then execute the staged prompts in this directory as one coherent migration program.

## Mission

Make automatic MCP/Cockpit/control-plane startup and cross-agent cooperation a proven repository invariant, not a collection of components or documentation claims.

The final system must converge from a deliberately dead local state to one healthy control-plane, discoverable MCP/API/OpenAPI/WebSocket/Cockpit, auto-attached agent session, loaded session context/handover, peer presence and observable collaboration state without requiring the operator to remember a manual startup/join sequence.

## First principle

Do not trust the prompt's assumptions about current implementation. Inspect actual code, docs, rules, doctrines, schemas, tests, CI, `.ai/**`, `.majordomus/**`, `AGENTS.md`, `.envrc`, CLI, server, MCP, Cockpit and session-context machinery.

If an existing implementation already solves a requirement correctly, reuse it and prove it. Do not create a second mechanism.

## Mandatory working method

For each stage:

1. inspect and map current state,
2. identify canonical owner/source,
3. design smallest coherent correction,
4. implement,
5. migrate legacy behavior,
6. add executable enforcement,
7. test focused behavior,
8. run relevant repository gates,
9. inspect diff for duplication/drift,
10. record evidence before continuing.

Do not stop at analysis if implementation is possible.

## Cross-stage invariants

- one canonical typed domain model,
- one implementation per domain operation,
- derived CLI/API/OpenAPI/MCP/Cockpit/docs where appropriate,
- thin provider adapters,
- thin `.envrc`/agent hooks,
- no consumer-specific inventories,
- no hidden port duplication,
- no network/build on normal shell entry,
- idempotent and concurrency-safe ensure,
- stale-state recovery,
- typed/versioned protocol,
- actionable diagnostics,
- real E2E from stopped state.

## Completion rule

Do not report “done” because individual endpoints exist. Completion requires the cold-start E2E proof specified in `SPEC.md` and cross-surface drift checks.

At the end produce a final evidence report with:

- before/after architecture,
- root causes,
- canonical sources introduced/reused,
- legacy code removed,
- runtime lifecycle behavior,
- session/peer protocol behavior,
- provider adapter behavior,
- CLI/API/OpenAPI/MCP/Cockpit/docs integration,
- rules/doctrines/gates,
- exact tests/commands and outcomes,
- cold-start proof,
- multi-agent proof,
- worktree proof,
- remaining genuine debt only.
