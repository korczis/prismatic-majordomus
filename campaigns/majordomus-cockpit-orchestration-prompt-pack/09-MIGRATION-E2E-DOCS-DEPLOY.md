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


# Phase 8 — Legacy migration, E2E tests, documentation and deployment

## Mission

Finish the feature. Remove the old paths, validate real behavior, document the architecture and deploy/verify according to repository doctrine.

## Legacy migration audit

Search for and migrate/delete:

- frontend provider/model lists,
- old provider selection controls bypassing router,
- provider-specific session state,
- raw transcript handover assumptions,
- local sorting/filtering that contradicts canonical ordering,
- duplicate API/MCP models,
- stale docs diagrams,
- manual Swagger schemas,
- old session-context compression/summarization paths,
- legacy retry/fallback logic outside the canonical router,
- diagnostics that only appear in logs,
- outdated generated artifacts,
- orphan Cockpit pages replaced by the orchestration area.

Do not keep dead paths "for compatibility" without a real consumer/test.

## E2E scenarios

Implement real end-to-end tests using supported test doubles/fake adapters where external provider calls would be nondeterministic or expensive.

At minimum exercise:

### Scenario A — cheap/local path

```text
task accepted
→ preflight
→ small context discovered
→ compiler produces small working set
→ fitting cheap/local backend selected
→ execution succeeds
→ validation sufficient
→ knowledge/handover state updated
→ Cockpit timeline reaches completed
```

### Scenario B — context-driven route

```text
large evidence set
→ long-context-compatible backend selected for compression/analysis
→ compressed provenance-preserving working set
→ stronger reasoning backend
→ opposition backend
→ final verified result
```

### Scenario C — epistemic escalation

```text
backend A
→ low confidence or conflicting evidence
→ escalation event
→ handover persisted
→ backend B
→ validation
→ completion
```

### Scenario D — provider failure

```text
backend A unavailable/fails
→ operational fallback
→ no loss of canonical execution state
→ backend B resumes
```

### Scenario E — privacy constraint

```text
local-only mission
→ remote models excluded
→ explainable route
→ no remote call
```

### Scenario F — Cockpit reconnect

```text
open execution
→ receive live events
→ disconnect
→ more events occur
→ reconnect/reconcile
→ no duplicate/missing/impossible state
```

### Scenario G — provenance drill-down

```text
compressed fact
→ Cockpit/API/CLI reference
→ canonical source resolution
→ ADR/file/commit/test/knowledge target
```

## Cross-surface acceptance

For the same fixture/execution, prove:

- CLI JSON,
- REST,
- MCP,
- Cockpit projection

agree on canonical IDs, phase/status, active backend, handover/switch history and routing/context summaries.

## Documentation

Update the repository's actual documentation hierarchy.

Document:

- architecture and why it exists,
- canonical orchestration state,
- context compiler,
- provenance/drill-down,
- routing constraints/scoring,
- switching/escalation,
- cross-model handover,
- provider adapter extension,
- adding a new routing criterion,
- Cockpit operator/developer usage,
- CLI/API/OpenAPI/MCP usage,
- security/redaction,
- event/reconnect semantics,
- validation/gates,
- troubleshooting.

Generate inventories from canonical registries where possible.

Handle ADR creation/update according to existing ADR policy, not because this prompt happens to like ADRs.

## GitHub Pages / deployment

Follow the repository's existing release/docs/deployment policy.

If Cockpit/static docs/GitHub Pages are deployable from this change:

- build them through canonical commands,
- ensure generated artifacts are synchronized,
- deploy only through approved workflow,
- verify the deployed artifact/version/health using repository tooling.

Do not invent a deployment target or publish from an unclean/invalid state.

## Performance/security validation

Measure/check:

- snapshot serialization size,
- large event timeline behavior,
- context inspector pagination/lazy loading,
- no browser payload containing raw secrets,
- no accidental full knowledge-base dump,
- no network work added to fast environment/banner paths,
- no provider probes just to render static registry rows,
- no pathological polling.

## Final repository gate

Run the full canonical quality gate and all targeted tests. Inspect `git diff`, generated drift and repo status.

Leave the feature in the state demanded by the project's "done" invariant: tested, documented, committed/pushed/integrated/deployed/verified only to the extent the repository's actual workflow and current permissions permit. Report each status explicitly rather than implying success.
