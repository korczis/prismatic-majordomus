# PROMPT 09 — CLI, API, OpenAPI, MCP and Cockpit surface parity

## Mission

Ensure the useful canonical information powering the landing page is not trapped inside LiveView.

## Audit existing surfaces

For each canonical landing concept, decide which surfaces legitimately need exposure:

- CLI
- structured JSON
- REST/API
- OpenAPI/Swagger
- MCP
- Cockpit
- docs

Not every visual composition needs its own endpoint.

Prefer exposing canonical underlying models/projections through existing generic interfaces.

## CLI

Provide or reuse commands for useful state such as:

```text
status
environment/runtime health
attention/diagnostics
active sessions
ready tasks
milestones
peers
```

Do not create redundant aliases if suitable commands exist.

## API/OpenAPI

Use canonical schemas and existing routing patterns.

OpenAPI must derive from implementation/schema, not be manually maintained.

## MCP

Expose meaningful list/inspect/action semantics using canonical IDs and schemas.

Do not independently recreate a Cockpit landing abstraction in MCP if underlying domain resources already suffice.

## Contract parity

Add tests showing equivalent canonical objects/IDs/status semantics agree across relevant surfaces.

## Acceptance

Landing is a rich consumer of canonical Majordomus state, not the only place where the state exists.
