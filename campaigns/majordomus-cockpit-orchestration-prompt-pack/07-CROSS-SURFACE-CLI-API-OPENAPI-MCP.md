# Common Contract — Majordomus Cockpit Runtime Orchestration

You are working inside the actual `prismatic-majordomus` repository with a large context window.

This is not a mockup task and not a request to bolt another dashboard onto the side of the project.

## Non-negotiable operating rules

Before editing anything, discover and obey the repository's current canonical governance and architecture:

- `AGENTS.md` and any nested agent instructions,
- `.ai/**`, `.majordomus/**`, rules, doctrines, policies, schemas,
- ADRs and knowledge/session/handover mechanisms,
- current CLI, API, OpenAPI/Swagger, MCP and Cockpit architecture,
- generated registries and code generation,
- tests and quality gates,
- current provider/model abstractions,
- current runtime/cooperation/server architecture,
- current docs and GitHub Pages generation/deployment,
- current worktree/branch conventions.

Do not assume filenames, frameworks, module names, ports, schemas or endpoints. Discover them.

Preserve the Majordomus invariants:

- one canonical source of truth,
- typed domain models,
- inferred/derived/discovered metadata,
- deterministic behavior,
- zero or near-zero consumer registration,
- no duplicated inventories,
- no frontend-owned copies of backend semantics,
- no stringly typed parallel configuration when typed metadata already exists,
- no secret values in UI/logs/API/MCP,
- no hidden network work in fast local paths,
- no silent fallback that masks invalid state,
- no "temporary" implementation that becomes a second architecture,
- documentation, tests and enforcement are part of the feature, not cleanup.

When multiple representations are required, the intended direction is:

```text
canonical typed domain/state
        ↓
schema / registry / projection
        ↓
CLI JSON
REST API
OpenAPI / Swagger
MCP
Cockpit
docs / generated indexes
tests / diagnostics
```

Do not reverse this by making the Cockpit the source of truth.

## Required work discipline

For every prompt in this pack:

1. inspect before designing,
2. find the actual root cause / missing abstraction,
3. reuse or extend existing canonical mechanisms,
4. implement the smallest coherent architecture that solves the class of problem,
5. migrate legacy consumers,
6. remove obsolete duplicate logic,
7. add unit + contract + integration + E2E tests as appropriate,
8. update docs and governance,
9. run the repository's real gates,
10. inspect the final diff,
11. leave an explicit handover for the next prompt,
12. do not claim completion without evidence.

Do not ask the user to choose obvious implementation details. Infer them from the repository.

If repository evidence contradicts this prompt, preserve the invariant and adapt the implementation to reality. Document the divergence.

## Core product invariant

Majordomus is the continuity layer. Individual models/providers are replaceable execution backends.

The UI must make this visible and inspectable without turning model selection into a manually maintained control panel.

The canonical flow to support is:

```text
USER INTENT
    ↓
GOVERNANCE PREFLIGHT
    ↓
CONTEXT DISCOVERY
    ↓
CONTEXT COMPILER
    ↓
MODEL ROUTER
    ↓
MODEL / PROVIDER SELECTION
    ↓
REASON / OPPOSE / EXECUTE
    ↓
CONFIDENCE + VALIDATION
    ├── sufficient → continue
    └── insufficient → switch / escalate / second opinion
    ↓
VERIFIED RESULT
    ↓
KNOWLEDGE UPDATE
    ↓
CONTEXT COMPRESSION / HANDOVER
```

The UI must represent the epistemic/work state, not pretend that a chat transcript is the system state.


# Phase 6 — Cross-surface orchestration contracts

## Mission

Make the orchestration system uniformly addressable through existing Majordomus surfaces.

Cockpit is a consumer, not a special privileged implementation.

## Canonical operations/resources

Based on the actual repository architecture, expose the minimum coherent set of typed operations to:

- inspect current/known executions,
- inspect one execution snapshot,
- inspect event history/stream metadata,
- inspect context compiler state,
- inspect routing decisions,
- inspect handovers,
- request supported orchestration actions,
- inspect provider/model registry,
- inspect diagnostics/policies affecting an execution.

Do not blindly create REST endpoints named from this prompt. Integrate with existing resource/action conventions.

## CLI

Extend existing command trees.

Provide machine-readable structured output from canonical types.

Potential concepts:

```text
majordomus orchestration status
majordomus orchestration execution <id>
majordomus orchestration context <id>
majordomus orchestration route <id>
majordomus orchestration handover <id>
majordomus orchestration providers
majordomus orchestration explain <id>
```

Only create commands that fit current CLI structure. Prefer aliases/bridge generation if the repo already derives commands.

Interactive actions must use the same backend service layer as API/MCP/Cockpit.

## REST/API

Use existing API conventions and serialization.

Requirements:

- typed request/response schemas,
- stable IDs,
- pagination/filtering for histories where needed,
- canonical ordering,
- explicit errors,
- no secret leakage,
- no UI-only fields mixed into the domain,
- revision/version information where live reconciliation needs it.

## OpenAPI / Swagger

Generate or derive documentation from the same types/routes according to the existing stack.

No manually maintained duplicate schema.

Examples and descriptions should explain:

- snapshot vs event stream,
- auto-selection vs override,
- operational fallback vs epistemic escalation,
- unknown vs measured cost/token/confidence fields,
- redaction/authorization behavior.

## MCP

Expose orchestration through the existing MCP model.

Potential resource/tool semantics:

- read execution,
- inspect compiled context/evidence,
- inspect routing explanation,
- inspect handover,
- list provider/model capabilities,
- request validation/opposition/escalation if allowed.

Do not expose a giant "do everything" string prompt when typed MCP inputs can be used.

## Live transport

If Cockpit receives typed events via a runtime transport, ensure:

- API/MCP/CLI consumers can still obtain an authoritative snapshot,
- event schemas are canonical,
- reconnect semantics are documented/tested,
- event transport is not the only place state exists,
- server restarts do not produce impossible state.

## Contract tests

Use shared fixtures and stable canonical IDs to prove consistency between:

```text
domain snapshot
CLI JSON
REST response
MCP response
Cockpit data projection
OpenAPI schema
```

Do not maintain six independent golden inventories.

## Completion and discoverability

If command completion / Cockpit command palettes / docs indexes are generated from registries, extend those generators rather than registering orchestration commands in each consumer separately.
