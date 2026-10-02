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


# Phase 1 — Deep discovery and architecture gap audit

## Mission

Map the current Majordomus runtime, context, provider/model, cooperation, session/handover and Cockpit systems before changing them.

This phase should produce implementation-ready evidence, then make only low-risk preparatory refactors that are clearly required. Do not prematurely build UI components around guessed backend semantics.

## Audit targets

Discover the actual code paths and canonical sources for:

- user/task/mission state,
- governance preflight,
- session context retrieval,
- knowledge retrieval,
- ADR discovery,
- Git/history discovery,
- source/test discovery,
- issue/milestone linkage,
- context construction,
- token accounting/budgets,
- summarization/compression,
- provenance/citations/source references,
- model/provider registry,
- model capability metadata,
- context-window metadata,
- latency/cost metadata,
- privacy/local-only constraints,
- provider availability/credentials state,
- tool capability compatibility,
- routing/selection,
- retries/fallbacks,
- validation/confidence/opposition/review,
- cross-model handover,
- cooperation/co-work/MCP/server/websocket/event infrastructure,
- persistence of execution state,
- Cockpit information architecture,
- Cockpit data fetching/subscriptions/live updates,
- CLI/API/OpenAPI/MCP equivalents,
- diagnostics/doctor/check surfaces,
- tests/gates/docs.

## Produce a capability map

Create an internal table:

```text
capability
current canonical source
current type/schema
producer(s)
consumer(s)
persistence
live transport
tests
governance
known drift/duplication
missing abstraction
```

Explicitly identify:

- parallel provider registries,
- frontend hardcoded model/provider lists,
- provider-specific logic outside adapters,
- context assembled directly from raw chat/history,
- lossy summaries without provenance,
- handovers that cannot be rehydrated,
- routing rules duplicated between CLI/backend/UI,
- API/MCP schemas maintained manually,
- confidence fields that are cosmetic/unvalidated,
- event streams with no typed event contract,
- task state that exists only in logs,
- runtime state that cannot survive model switching.

## Find the best canonical integration point

Determine whether the repository already has equivalents of:

- `RepositoryEnvironment`,
- task/session/execution snapshot,
- provider registry,
- diagnostics,
- event bus,
- schema derivation,
- Cockpit resource registry.

Do not create `OrchestrationSnapshot` merely because this prompt names it. Reuse/extend the closest canonical domain object if that is cleaner.

The desired architecture is conceptually:

```text
repository + session + knowledge + runtime evidence
                ↓
typed discovery / registries
                ↓
context compiler state
                ↓
orchestration execution state
                ↓
canonical snapshot + typed events
                ↓
CLI / API / OpenAPI / MCP / Cockpit / docs
```

## UI gap audit

Map every current Cockpit page/component relevant to:

- sessions/tasks,
- providers/models,
- prompts,
- rules/doctrines/policies,
- knowledge/ADRs,
- issues/milestones,
- runtime/server/peers,
- diagnostics,
- cost/usage,
- logs/events.

Mark each as:

```text
keep
extend
merge
replace
delete
```

Avoid adding a seventh overlapping "AI" page if existing navigation can be rationalized into one orchestration control plane.

## Required output and implementation

By the end:

- write/update an architecture note using repository conventions,
- create a precise migration plan,
- add failing/characterization tests for the most dangerous current drift where practical,
- make only foundational refactors that are obviously necessary,
- leave the repository green or document pre-existing failures exactly.

Do not implement mock orchestration data merely to unblock the next UI phase.
