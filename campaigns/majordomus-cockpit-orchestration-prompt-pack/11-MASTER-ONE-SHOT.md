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


# MASTER — Implement Majordomus Runtime Orchestration as a Cockpit Control Plane

Use this when running a high-context agent that should execute the entire program in one coordinated implementation.

## Mission

Implement a first-class Majordomus runtime orchestration subsystem and make it fully visible/controllable in Cockpit while preserving one canonical typed source for CLI/API/OpenAPI/MCP/docs.

The architecture must embody:

```text
USER INTENT
  ↓
GOVERNANCE PREFLIGHT
  ↓
CONTEXT DISCOVERY
  ↓
CONTEXT COMPILER
  ├ relevance
  ├ deduplication
  ├ provenance
  ├ compression
  └ token budget
  ↓
MODEL ROUTER
  ├ task/phase
  ├ complexity/risk
  ├ required tools/capabilities
  ├ context characteristics
  ├ privacy
  ├ provider availability
  ├ latency/cost
  └ confidence floor
  ↓
AUTO MODEL SELECTION
  ↓
REASON / OPPOSE / EXECUTE
  ↓
CONFIDENCE + VALIDATION
  ├ sufficient → continue
  └ insufficient
       ↓
     switch / escalate / second opinion
       ↓
verified result
  ↓
KNOWLEDGE UPDATE
  ↓
CONTEXT COMPRESSION / HANDOVER
```

## Product thesis to preserve

Majordomus does not transfer a conversation between models. It transfers the state of knowledge and work.

Context does not accumulate; it compiles.

The model is not the identity of the agent. It is a dynamically chosen execution backend.

A stronger/more expensive backend is used when its marginal utility justifies it under capability, risk, context, privacy, latency and cost constraints.

Each step returns to persistent, inspectable, provenance-preserving project state.

## Execute the entire program

### A. Discovery

Deeply audit the repo's actual:

- session/context/knowledge/ADR/Git/source/issues discovery,
- provider/model adapters and registries,
- routing/retry/fallback,
- confidence/validation/opposition,
- persistence/handover,
- cooperation/runtime/event transport,
- Cockpit,
- CLI/API/OpenAPI/MCP,
- governance/gates/docs.

Produce a gap map before architecture changes.

### B. Canonical domain

Create/reuse typed canonical orchestration state for:

- mission,
- execution lifecycle,
- evidence/provenance,
- decisions,
- open questions/hypothesis,
- plan/progress,
- context budget,
- routing decisions,
- backend history,
- validation/confidence,
- diagnostics,
- handover,
- usage/cost/latency where supported.

Create/reuse typed domain events and authoritative snapshot/reconciliation semantics.

### C. Context compiler

Implement/reuse a deterministic compiler pipeline:

```text
large available knowledge universe
→ retrieve candidates
→ rank
→ deduplicate/conflict-group
→ provenance-preserving compression
→ token budget
→ model-specific working set
```

Support drill-down from compressed fact to sources.

Never collapse disputed evidence into confirmed fact.

Never dump the full knowledge universe to the browser by default.

### D. Router

Implement/reuse canonical provider/model capability registry.

Apply hard constraints first:

- required capability,
- context capacity,
- privacy/local-only,
- provider availability,
- tool fit,
- risk/quality floor.

Then optimize soft objectives using only justifiable/known metrics.

Support per-phase model selection.

Support operational fallback and epistemic escalation as distinct mechanisms.

Switch on low confidence, evidence conflict, validation failure, context mismatch, complexity increase, poor backend fit and transport failure where policy dictates.

### E. Cross-model handover

On every backend switch preserve canonical execution state and source references.

Do not make the next model dependent on the entire previous raw conversation.

Ensure handover is typed, persisted/versioned according to existing repo practice, and inspectable.

### F. Cockpit

Integrate a coherent Orchestration control plane into the existing Cockpit design/navigation.

Must support, merged with existing pages where appropriate:

- execution overview,
- live timeline,
- context compiler inspector,
- routing candidate/decision inspector,
- backend switch/escalation history,
- handover/epistemic state,
- evidence/provenance drill-down,
- provider/model registry,
- governance/policy inspector,
- execution history if retention already supports it,
- safe typed actions such as validate/opposition/escalate/override/cancel/re-run where backend semantics support them.

Cockpit must render canonical state. No hardcoded provider/phase/policy inventories.

### G. CLI/API/OpenAPI/MCP

Expose the same state/actions through existing canonical conventions.

Structured CLI/REST/MCP outputs must agree on IDs and semantics.

Generate/derive OpenAPI schema/docs.

Extend completion/command registries via canonical generation/discovery if present.

### H. Governance

Encode and machine-enforce:

- context compilation + provenance,
- model-as-backend,
- handover on switch,
- hard routing constraints before soft scoring,
- epistemic escalation,
- canonical cross-surface state,
- secret redaction,
- testing/documentation.

Make validators/gates catch duplication and bypasses.

### I. Legacy migration

Remove/replace:

- duplicated frontend model/provider lists,
- provider-specific session identity,
- raw-transcript handovers,
- legacy fallback/router paths,
- duplicated schemas,
- stale Cockpit pages,
- stale docs,
- old generated artifacts.

No gratuitous compatibility cemetery.

### J. Testing

Add:

- state transition tests,
- deterministic route tests,
- context/provenance tests,
- model-switch/handover tests,
- fallback vs escalation tests,
- cross-surface contract tests,
- Cockpit live/reconnect tests,
- E2E execution scenarios,
- security/redaction tests,
- zero-registration extension test,
- performance/large-history tests,
- generated drift/gate tests.

### K. Documentation / ADR / deployment

Update canonical documentation and generated pages.

Handle ADR according to repo policy.

Run canonical build/test/check/doc/generate/deploy workflows.

Verify deployment only where repository workflow and permissions permit.

## Cockpit acceptance UX

A user opening an active execution should be able to answer, from one coherent UI:

1. What are we trying to accomplish?
2. What phase are we in?
3. What evidence was discovered?
4. What was retained/compressed/dropped and why?
5. Where did each important fact come from?
6. Which models were considered?
7. Why was this backend selected?
8. Which hard constraints excluded alternatives?
9. How much context/token/cost/latency budget is being consumed, if known?
10. Did confidence/validation change?
11. Why did a switch/escalation happen?
12. What epistemic/work state crossed the handover?
13. What rules/doctrines/policies governed this decision?
14. What remains unresolved?
15. What will happen next?

If the UI cannot answer those questions from canonical data, the implementation is incomplete.

## Final adversarial proof

Before completion, deliberately attempt to falsify:

- zero-registration extensibility,
- handover continuity,
- provenance preservation,
- router adaptivity,
- Cockpit/API consistency,
- event reconnect correctness,
- secret redaction,
- governance enforcement.

Do not claim success because the happy path screenshot is attractive. Screenshots have been enabling architectural fraud since the invention of PowerPoint.

## Final report

Report exact evidence for:

- root cause,
- architecture,
- migrations,
- tests,
- gate commands,
- cross-surface parity,
- Cockpit behavior,
- security,
- performance,
- docs/ADR,
- commit/push/integration/deploy/verify status,
- remaining debt.
