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


# Phase 4 — Model router, auto-selection, switching, escalation and cross-model handover

## Mission

Turn providers/models into dynamic execution backends selected per phase, not identities bound to an entire session.

## Canonical model/provider registry

Audit and consolidate provider/model capability metadata.

A model entry should derive or expose only supported data, such as:

- canonical provider/model ID,
- availability/configuration state,
- local/remote/privacy characteristics,
- context window,
- capability tags,
- tool compatibility,
- structured-output support,
- reasoning/coding/vision/etc. capabilities where actually known,
- cost metadata if authoritative/configured,
- latency observations/metadata if measured,
- health/circuit-breaker state.

No frontend-maintained provider catalog.

Never expose credentials.

## Routing input

Define/reuse a typed routing request based on:

- execution phase/task type,
- complexity,
- risk,
- required capabilities,
- required tools,
- context characteristics,
- quality/confidence floor,
- privacy/local-only constraints,
- provider availability,
- latency objective,
- cost/budget constraints.

Routing is multi-objective. Do not reduce it to "use the strongest model".

A conceptual utility model may consider:

```text
quality × confidence × task_fit × tool_fit
------------------------------------------
cost × latency × context_cost
```

but implement only metrics the repository can justify. Make unknown values explicit.

## Hard constraints before scoring

Reject incompatible backends before soft optimization:

```text
required capability
required context capacity
privacy/local-only policy
provider availability
tool compatibility
risk/quality floor
```

Explain rejection reasons.

## Per-phase selection

Support distinct backends for phases such as:

```text
discovery
classification
compression
architecture/reasoning
coding
mechanical edits
opposition/review
final synthesis
```

Do not hardcode those exact phases if the canonical workflow differs. Map to repository concepts.

## Runtime switching

Switching/escalation must be possible for:

- provider/API failure,
- context overflow,
- low or falling confidence,
- newly discovered complexity,
- evidence conflict,
- failed validation,
- repeated unsuccessful attempt,
- poor tool/model fit,
- explicit policy requirement for independent opposition.

Distinguish:

```text
retry same backend
fallback
escalation
independent second opinion
ensemble/consensus
manual override
```

if the current architecture supports these semantics.

## Cross-model handover

On switch, transfer canonical execution state, not merely raw chat.

Conceptual handover payload:

```yaml
mission:
  objective: ...
  constraints: ...

evidence:
  confirmed: ...
  disputed: ...
  deferred: ...

decisions:
  accepted: ...
  rejected: ...

open_questions: ...
working_hypothesis: ...
relevant_sources: ...
plan:
  completed: ...
  next: ...
validation:
  passed: ...
  failed: ...
context_budget: ...
```

Use stable references for evidence and selective retrieval. The receiving model may request drill-down.

## Explainability

Every routing decision should be inspectable with:

- candidate backends,
- hard-constraint pass/fail,
- normalized scoring dimensions where supported,
- selected backend,
- selection reason,
- alternatives,
- switch trigger,
- confidence/validation evidence,
- cost/context implications.

Do not expose sensitive internal prompts or secrets just to be "transparent".

## Override semantics

If manual backend selection already exists or is useful, implement it as an explicit policy/constraint override with auditability, not by bypassing orchestration.

The default remains auto-selection.

The UI must make "auto" the canonical path and show when an override is active.

## Tests

Include:

- cheapest fitting backend wins under defined policy,
- capability mismatch excludes candidate,
- local-only prevents remote provider,
- context requirement excludes too-small model,
- validation failure triggers escalation,
- provider failure triggers fallback without epistemic state loss,
- low confidence triggers second opinion when policy requires,
- switch preserves mission/evidence/decisions/plan,
- override is visible/auditable,
- unknown cost/latency does not become zero,
- all routing results deterministic given deterministic inputs/measurements/policy.
