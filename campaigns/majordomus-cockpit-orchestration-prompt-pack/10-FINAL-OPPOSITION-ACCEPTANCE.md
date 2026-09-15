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


# Phase 9 — Adversarial review, architectural opposition and acceptance proof

## Mission

Assume the implementation is wrong in subtle ways. Try to break the architectural claims before calling it done.

Use repository evidence, tests and independent reasoning.

## Attack the implementation

Investigate these failure classes:

### Hidden duplication

Search for second sources of truth for:

- providers/models,
- model capabilities,
- execution phases,
- routing criteria,
- Cockpit navigation/resource inventories,
- API/MCP schemas,
- context source kinds,
- handover fields.

### Fake dynamism

Add a representative new provider/model through the canonical extension mechanism.

Prove it appears automatically in all relevant:

- router candidates,
- CLI inspection,
- API,
- OpenAPI schema/examples where applicable,
- MCP,
- Cockpit registry,
- docs/generated indexes,

without manually registering it in each surface.

Then remove the fixture/test addition if it should not remain.

### Model-switch amnesia

Start an execution with backend A, accumulate evidence/decisions/open questions, switch to B, then assert B receives/resolves the canonical state and references without requiring the complete raw prior transcript.

### Compression lie

Prove compressed facts cannot silently lose required provenance or collapse disputed evidence into confirmed truth.

### Router theatre

Prove auto-selection actually changes when:

- context size changes,
- privacy constraint changes,
- provider availability changes,
- required capability changes,
- validation confidence/failure changes.

If the selected backend never changes, determine whether the router is merely decorative.

### Cockpit divergence

Compare displayed state against canonical API/domain snapshots during:

- fast event bursts,
- reconnect,
- backend switch,
- failed validation,
- cancellation,
- completion.

No stale "active model", ghost progress, or client-only status.

### Secret leakage

Search code, fixtures, snapshots, logs and browser/API/MCP payloads for credential-shaped data and provider-native secret metadata.

### Unknown-is-zero bug

Check cost, latency, confidence and token fields. Unknown/unmeasured must never misleadingly render/score as zero.

### Event ordering/replay

Test duplicated, delayed or out-of-order transport events if transport semantics allow it. The UI must reconcile against authoritative state/revision rules.

### Governance bypass

Intentionally construct representative invalid states and prove canonical gates reject them.

## Independent opposition

If the repository supports multi-provider/opposition workflows, use the actual mechanism to review the implementation architecture. Otherwise perform a separate explicit review pass in this session.

Do not modify code based on opposition blindly. Verify claims against repository evidence.

## Final acceptance checklist

Architecture:
- [ ] canonical typed orchestration state exists/reused,
- [ ] typed event contract exists/reused,
- [ ] provider/model registry is canonical,
- [ ] context compiler state is provenance-preserving,
- [ ] router is capability/policy/context aware,
- [ ] switching preserves epistemic/work state,
- [ ] operational fallback and epistemic escalation are distinguishable.

Cockpit:
- [ ] live execution overview,
- [ ] timeline,
- [ ] context compiler inspector,
- [ ] route explanation,
- [ ] switch/escalation history,
- [ ] handover state,
- [ ] provenance drill-down,
- [ ] provider registry,
- [ ] policy/governance visibility,
- [ ] safe controls,
- [ ] reconnect correctness,
- [ ] no frontend canonical inventories.

Cross-surface:
- [ ] CLI,
- [ ] API,
- [ ] OpenAPI/Swagger,
- [ ] MCP,
- [ ] Cockpit,
- [ ] docs/generated views
share canonical state/registries.

Quality:
- [ ] unit tests,
- [ ] state-machine tests,
- [ ] routing tests,
- [ ] context/provenance tests,
- [ ] cross-surface contract tests,
- [ ] E2E tests,
- [ ] regression tests,
- [ ] security/redaction tests,
- [ ] performance checks,
- [ ] generated drift checks,
- [ ] canonical repo gate green or unrelated failures precisely evidenced.

Governance:
- [ ] rules/doctrines/policies updated appropriately,
- [ ] machine enforcement exists,
- [ ] startup/CI/gates actually execute it,
- [ ] zero-registration extension path tested.

## Final report

Return:

```text
ROOT CAUSES
CANONICAL ARCHITECTURE
CONTEXT COMPILER
ROUTING / SWITCHING
COCKPIT
CROSS-SURFACE CONTRACTS
GOVERNANCE / ENFORCEMENT
LEGACY REMOVED
TEST / VALIDATION EVIDENCE
PERFORMANCE / SECURITY EVIDENCE
ZERO-REGISTRATION PROOF
DOCS / ADR
COMMIT / PUSH / INTEGRATION / DEPLOY / VERIFY STATUS
REMAINING DEBT
```

Do not say "done" if one of those statuses is unknown. State unknown.
