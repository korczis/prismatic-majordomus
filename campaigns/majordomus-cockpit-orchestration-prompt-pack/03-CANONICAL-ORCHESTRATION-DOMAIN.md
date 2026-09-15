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


# Phase 2 — Canonical typed orchestration domain and live state

## Mission

Create or extend the canonical typed representation of Majordomus runtime orchestration.

The state must survive a provider/model change and be renderable by every consumer without passing raw provider transcripts around as the source of truth.

## Model the execution state

Adapt names to the repository. Conceptually cover:

```text
Mission
  objective
  constraints
  risk/privacy class
  requested outcome

Execution
  execution_id
  phase
  status
  started/updated/completed
  active backend
  prior backends
  parent/child/subtask relationships

Evidence
  canonical evidence id
  kind/source
  provenance
  relevance
  confidence/status
  token estimate
  compressed representation
  drill-down reference

Decisions
  accepted
  rejected
  disputed
  rationale/source refs

Hypotheses / open questions
Validation state
Context budget state
Routing state
Cost/latency/usage state
Plan/progress
Diagnostics
Handover state
```

Do not force fields that cannot be supported by repository data. Prefer explicit `Unknown/Unavailable/NotMeasured` semantics over invented numbers.

## Typed lifecycle

Define an explicit state machine or equivalent validated lifecycle for orchestration phases.

It must be impossible or diagnosable to represent nonsense such as:

- completed before started,
- switched model with no handover boundary,
- evidence marked verified with no validation/provenance when policy requires it,
- active backend not present in provider registry,
- negative token/cost values,
- invalid phase transitions.

If the project already has a workflow/state machine abstraction, reuse it.

## Canonical events

Create/reuse typed events suitable for live Cockpit updates and other consumers, e.g. conceptually:

```text
MissionAccepted
PreflightStarted/Completed
EvidenceDiscovered
ContextRanked
ContextCompressed
BudgetUpdated
RouteEvaluated
BackendSelected
BackendStarted
BackendOutputProgress
ValidationStarted
ValidationResult
ConfidenceChanged
EscalationTriggered
BackendSwitched
HandoverCreated
ExecutionCompleted/Failed/Cancelled
KnowledgeUpdated
DiagnosticRaised
```

Do not generate events just for animation. Events must represent domain truth.

Events must have:

- stable type identifiers,
- schema/versioning according to repo conventions,
- execution correlation,
- timestamps from canonical time source,
- safe/redacted payloads,
- deterministic serialization where required,
- test coverage.

## Snapshot + event relationship

Cockpit needs a consistent snapshot plus incremental updates.

Aim conceptually for:

```text
GET/read snapshot
      +
subscribe typed events from revision N
      ↓
reconcile
```

Use the repository's actual live transport. Do not assume WebSocket/SSE/Phoenix channels if not already canonical.

Handle reconnect/replay or authoritative refresh according to existing runtime capabilities.

## Integration with RepositoryEnvironment

If `RepositoryEnvironment` exists, decide deliberately:

- environment state describes repository/runtime capabilities,
- orchestration state describes active/historical executions,
- the two may reference each other,
- neither should duplicate provider/service/diagnostic inventories.

Make provider/service references by stable canonical IDs, not copies.

## Persistence / handover boundary

Persist the epistemic/work state needed to continue an execution without persisting secrets or blindly duplicating full transcripts.

Define what is:

```text
persistent canonical state
ephemeral execution state
provider-native opaque metadata
derived projection
cache
```

Test serialization/deserialization and schema migration if persistence already exists.

## Acceptance

A synthetic execution can be represented end-to-end with at least two backend switches while preserving mission, evidence, decisions, open questions, plan and validation state, without requiring the next model to receive the entire raw prior conversation.
