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


# Phase 5 — Cockpit runtime orchestration UX

## Mission

Make runtime orchestration a first-class Cockpit control-plane experience.

Do not build a decorative dashboard. The page must be a live projection/control surface over canonical orchestration state and typed actions.

First inspect the current Cockpit design system, navigation, component patterns, accessibility, responsive behavior and live transport. Reuse them.

## Information architecture

Integrate with existing navigation instead of duplicating pages. The target capabilities should be discoverable as one coherent "Orchestration" area or the repository's equivalent.

Conceptually provide these views/panels, merged with existing ones where appropriate:

### 1. Execution Overview

Show:

- mission/objective,
- status/phase,
- progress,
- active backend,
- execution start/duration,
- token/context budget,
- cost if authoritative,
- confidence/validation status,
- current blockers/diagnostics,
- active privacy/risk/policy constraints.

### 2. Live Execution Timeline

Render typed orchestration events in chronological order:

```text
preflight
evidence discovery
ranking
compression
route evaluation
backend selection
reasoning/execution
validation
switch/escalation
opposition
handover
knowledge update
completion
```

Allow expansion of event details using safe canonical payloads.

Must work under reconnect/refresh without fabricating missing history.

### 3. Context Compiler Inspector

Expose:

- candidate evidence,
- retained working set,
- compression stages,
- token budget,
- source-type distribution,
- provenance coverage,
- conflicts,
- dropped/deferred evidence with reasons.

Provide drill-down from compressed facts to original canonical sources where authorized.

Do not ship the entire knowledge base to the browser.

### 4. Model Router Inspector

Show:

- required capabilities/constraints,
- considered providers/models,
- excluded candidates + reasons,
- comparable scoring/fit dimensions,
- selected backend,
- alternative candidates,
- expected/measured context/cost/latency fields,
- active override, if any.

Unknown data must render as unknown, not zero.

### 5. Switch / Escalation History

For every change:

```text
from
to
reason
trigger type
confidence before/after where available
validation result
handover id
cost/context consequence
```

Distinguish operational failure from epistemic escalation.

### 6. Handover / Epistemic State

Make the current portable execution state inspectable:

- mission,
- constraints,
- confirmed/disputed evidence,
- decisions,
- open questions,
- working hypothesis,
- plan completed/next,
- validation state.

Do not show provider-native raw transcript as the canonical handover.

### 7. Evidence / Provenance Graph or Linked Explorer

Use the Cockpit's existing graph/list/table patterns if any.

The UX should make it possible to answer:

- "Why does the model believe this?"
- "Which ADR/file/commit/test supports it?"
- "What contradicts it?"
- "Which compressed fact came from this source?"
- "Which decision consumed this evidence?"

Do not introduce a graph library merely because graphs are fashionable. A linked table/tree can be superior and cheaper.

### 8. Provider / Model Registry

Render canonical discovered/configured backends:

- provider/model ID/display name,
- availability,
- capabilities,
- context capacity,
- local/remote/privacy properties,
- tool compatibility,
- health,
- cost/latency metadata when supported.

Credentials: status only, never values.

### 9. Policy / Governance Inspector

Show which rules/doctrines/policies affected:

- preflight,
- evidence requirements,
- model eligibility,
- escalation,
- opposition/review,
- validation,
- persistence/knowledge update.

Link to canonical governance entities.

### 10. Historical Executions

If the backend already persists executions, provide searchable/filterable history with stable IDs.

Do not invent persistence solely to render history if project policy deliberately keeps some runs ephemeral. In that case reflect the actual retention model.

## Controls

Actions must call typed canonical backend operations, not mutate local UI state pretending a system action happened.

Where supported, expose actions such as:

- start/re-run execution,
- cancel,
- request validation,
- request independent opposition,
- force/clear a model override,
- trigger escalation,
- inspect/recompile context,
- open source/provenance target,
- resume from handover.

Guard dangerous/expensive actions with existing authorization/confirmation patterns.

## UX constraints

- live but not visually noisy,
- mobile/responsive according to current Cockpit standards,
- keyboard accessible,
- meaningful empty/loading/error states,
- no fake telemetry,
- no hidden polling storm,
- stable ordering,
- preserve scroll/filter state where appropriate,
- linkable execution IDs/routes,
- diagnostics actionable,
- large datasets paginated/virtualized using existing patterns,
- performance measured on representative execution histories.

## Zero duplication

No hardcoded:

- provider lists,
- phase names if registry-derived,
- rule lists,
- routing criteria lists,
- capabilities,
- service endpoints,
- docs links that can be derived.

A newly discovered canonical provider/model or orchestration phase should appear through the registry/schema/projection path with no frontend inventory edit, except generic rendering for a genuinely new schema kind.

## Tests

Add component/view tests and E2E coverage for:

- initial snapshot render,
- streaming event update,
- reconnect/reconciliation,
- backend switch,
- escalation reason,
- context budget update,
- provenance drill-down,
- explicit override + clearing it,
- unknown metrics,
- provider unavailable,
- validation failure,
- completion,
- permission/redaction behavior,
- no stale frontend state after refresh.
