# 01 — Repository Audit and Gap Map

Read the normative `SPEC.md` first.

Your task is to build an evidence-backed map of why automatic MCP/Cockpit/agent cooperation is not currently guaranteed.

## Inspect deeply

Discover, do not assume, the actual locations and implementations for:

- root/bootstrap instructions,
- `AGENTS.md` and provider-specific agent config,
- `.envrc` / direnv hooks,
- `just` entry commands,
- `majordomus` CLI bootstrap/runtime commands,
- HTTP server/control-plane process,
- MCP server/transport,
- WebSocket/event transport,
- OpenAPI/Swagger generation,
- Cockpit backend/frontend,
- session context storage/loading,
- handovers,
- worktree/branch identity,
- provider adapters,
- agent registration/discovery,
- peer/session state,
- rules/doctrines/schemas,
- tests/CI/gates,
- generated docs/GitHub Pages.

## Build an audit matrix

For every relevant capability capture:

```text
capability
canonical source today
entry point
runtime owner
persistence/state
schema
CLI exposure
REST exposure
OpenAPI exposure
MCP exposure
Cockpit exposure
docs
tests
auto-start behavior
auto-registration behavior
duplication/drift risk
status: working / partial / decorative / missing / legacy-conflicting
```

## Specifically hunt for false automation

Find cases such as:

- docs say auto-start but tests pre-start server,
- `.envrc` prints URL but does not ensure health,
- MCP routes exist but transport is not started,
- Cockpit has pages but backend registry is stale/static,
- WebSocket exists but no peer lifecycle protocol,
- sessions have files but are not loaded automatically,
- agent instructions require a human `join`,
- multiple session directories/formats compete,
- startup races create duplicate processes,
- fixed ports are copied in multiple places,
- PID files can become stale,
- provider hooks implement incompatible concepts,
- CLI/API/MCP duplicate domain logic.

## Run reality checks

Without modifying code initially, reproduce representative states:

1. current healthy-ish normal workflow,
2. control-plane fully stopped,
3. stale PID/socket/state if safely reproducible,
4. two concurrent shells/agent bootstrap attempts,
5. two worktrees,
6. Cockpit opened from cold state,
7. MCP client discovery from cold state.

Do not damage uncommitted work. Respect repository worktree rules.

## Deliverable inside repo

Create/update the repository's appropriate architectural audit artifact according to existing conventions. Do not invent a random docs location if there is already a knowledge/ADR/audit system.

Include:

- root causes ranked P0/P1/P2,
- dependency graph,
- legacy mechanisms to delete/merge,
- canonical owners to preserve,
- exact implementation plan mapped to prompts 02–09.

## Gate

Do not begin broad implementation until you can explain, with file/code/test evidence, the full path from repository entry to agent collaboration and precisely where it currently breaks.
