# Shared mission and operating contract

You are working inside the live `prismatic-majordomus` repository. Your task is to converge the existing Majordomus architecture around **Intent Driven Development (IDD)** as a first-class, canonical, executable and evidence-verified development model.

Core invariant:

> **Human intent is canonical. Work is derived. Reality is observed. Completion is proven.**

Target loop:

```text
Human Intent
    ↓
Canonical Intent
    ↓
Current-vs-desired gap + governance constraints
    ↓
Derived/linked milestones, issues, dependency graph, execution waves
    ↓
Provider-neutral sessions + mesh peers + handovers
    ↓
Implementation
    ↓
Discovery of actual repository/runtime state
    ↓
Validation + evidence
    ↓
Intent satisfaction evaluation
    ├─ unsatisfied → derive/replan remaining work
    └─ satisfied   → verified closure + proof lineage
```

## Live repository wins

A repository snapshot from 2026-09-11 was used only to build this prompt pack. It showed important existing primitives, but **it is not authoritative over the live checkout**. Before changing code, read the current `AGENTS.md`, `.ai/README.md`, effective rules, doctrines, policies, skills, workflows, ADRs and current implementation. Reconcile this prompt against the live repository. Never regress a newer design because this pack mentions an older path/name.

The snapshot already contained, among other things:

- milestones described as **executable specifications of outcomes**;
- issues described as **bounded execution contracts**;
- derived issue/milestone status rather than authored `status`;
- evidence-gated `DONE`/`VERIFY` semantics;
- validated dependency graphs and execution waves;
- roadmap derivation from milestone graph state;
- sessions and session-context records;
- handovers and blocker continuity across handovers;
- provider/session separation and provider lifecycle hooks;
- peer discovery/announcement/context integration;
- doctrine dispatch/enforcement and finish obligations;
- canonical capability/projection architecture;
- CLI/API/OpenAPI/MCP/site projections;
- strict derivation/cross-surface/canonical-architecture tests.

Treat these as **integration targets to reuse**, not invitations to create parallel machinery.

## Entry protocol

Before substantive work:

1. Update the local view of canonical trunk according to repository policy.
2. Read current `AGENTS.md` and `.ai/README.md` and follow their discovery protocol.
3. Run the current Majordomus context/preflight commands required by repository policy.
4. Inspect current task/milestone/issue/intent state before inventing a new record.
5. Inspect active peers and existing branches/worktrees. Announce intent and expected scope using the repository's current peer mechanism.
6. Use the canonical worktree topology. Do not do feature work in an invalid checkout merely because it is convenient.
7. Push work early enough that other peers can discover it.
8. Use repository-native generators and registries. Do not hand-edit generated projections.

If exact commands have changed since the snapshot, discover the current commands and use those.

## Non-negotiable architecture rules

- One canonical semantic definition, many derived projections.
- No hand-maintained duplicate inventories across CLI/API/MCP/Cockpit/docs/tests.
- Do not make the Cockpit a second source of truth.
- Do not make GitHub a second source of truth.
- Do not make a provider conversation the owner of project intent.
- Explicit human intent and machine-inferred/derived/observed facts must remain distinguishable through provenance.
- If a fact is machine-observable from an authoritative source, derive/observe it instead of asking humans or agents to duplicate it.
- Completion must be derived from current evidence and obligations, not trusted prose and not a manual boolean.
- Evidence must go stale when the underlying observed fact becomes stale or false.
- Cross-provider continuity must be proven at actual adapter/session boundaries, not by storing a provider name in a generic string.
- No mocks on product paths that claim to prove real backend integration.
- No surface-specific business logic that belongs in the canonical domain layer.
- No weakening/removing tests merely to make a new architecture pass.
- New canonical entities should propagate automatically to relevant projections without per-surface registration.

## Scope discipline

Do not explode this mission into unrelated improvements. Adjacent changes are required only when they are necessary to make the IDD invariant real, non-bypassable, testable and maintainable.

Prefer deleting or migrating redundant machinery over layering yet another adapter on top of it.

## Definition of real

For this work, a capability does not count as complete unless, as applicable, it is:

- represented in canonical typed/domain state;
- schema-valid;
- derived/inferred from authoritative sources where possible;
- integrated with existing milestone/issue/session/handover/mesh/evidence/governance architecture;
- exposed consistently through relevant CLI/API/OpenAPI/MCP/Cockpit/docs surfaces;
- covered by unit/integration/contract/E2E/regression tests;
- enforced through the existing doctrine/rule/gate system;
- generated projections are synchronized;
- committed/pushed/integrated according to repository workflow;
- deployed where the project claims a public/runtime surface;
- verified from the deployed/public surface, not merely asserted locally.

If deployment is blocked by a real external dependency, record the exact blocker and evidence. Never mark it successful by narrative.

## Existing tests/areas worth discovering

The 2026-09-11 snapshot included tests with names such as:

`05_handover`, `19_end_to_end`, `25_continuity_lifecycle`, `42_dag_waves`, `46_cross_surface`, `50_blocker_across_handover`, `55_session_context`, `60_session_lifecycle`, `71_context_sync`, `76_capabilities_projections`, `77_rust_evidence`, `90_mcp_shared_server`, `91_canonical_architecture`, `92_openapi_reference`, `98_traceability`, `99_plan_capabilities`, `103_obligations`, `104_strict_derivation`, `104_published_site`, `106_context_peers`, `111_session_per_provider`, `129_executable_issues`.

Search for their current equivalents. Extend the existing behavioral coverage rather than creating disconnected test islands.

## Required working style

Inspect → model → migrate → implement → test → integrate → document → enforce → deploy → verify → close.

Do not stop after an architecture report if implementation is possible. Do not ask for filenames, module names, obvious naming choices or routine confirmation. Infer them from the live repository and execute.

When ambiguity exists, preserve the repository's current architecture and choose the smallest design that makes the invariant explicit and testable.


# Prompt 06 — Cockpit and all canonical surfaces

## Mission

Expose Intent Driven Development through existing product surfaces without creating new truth sources. Cockpit is a command-and-observability projection of canonical backend state, not an independent workflow engine.

## Canonical projection contract

Define or reuse one structured Intent projection consumed by relevant surfaces. Ensure deterministic ordering and stable IDs.

Relevant consumers should include, as supported by current architecture:

- CLI human view;
- CLI JSON;
- REST API;
- generated OpenAPI/Swagger;
- MCP;
- Cockpit;
- generated documentation/GitHub Pages;
- GitHub planning projection.

If a surface is not actually part of the live architecture, do not invent it just to satisfy this list.

## CLI

Users/agents must be able to answer:

- list/show current Intents;
- explain one Intent;
- see current vs desired state;
- see derived satisfaction;
- see unsatisfied conditions and obligations;
- see realization graph/timeline/evidence.

Integrate with existing command/capability registry. Do not hand-edit CLI docs or completion inventories.

## API/OpenAPI

Expose canonical Intent state and relationships through existing resource/operation conventions. Generate OpenAPI from canonical capability/schema definitions.

Contract tests must prove API structured state agrees with the domain/CLI projection.

## MCP

Peers should be able to query the current Intent/work context, unsatisfied conditions, evidence obligations and explain/satisfaction state without reading repository files ad hoc.

MCP is a projection over canonical backend state, not a separate model.

## Cockpit

Create/upgrade a routable Intent view that answers at a glance:

```text
WHAT must become true?
WHY?
CURRENT vs DESIRED state
WHAT remains unsatisfied?
WHAT work realizes it?
WHO/which session/provider is executing it?
WHAT handovers occurred?
WHAT evidence currently proves each condition?
WHAT is blocked/drifting?
WHY is satisfaction derived as it is?
```

Useful drill-down chain:

```text
Intent
→ condition
→ work item
→ session/peer
→ handover/change
→ validation
→ evidence
```

Do not implement frontend sorting/grouping/business rules that contradict backend semantics. Use canonical IDs and backend-derived default ordering.

No mocks for claimed live functionality. If an API/backend piece is missing, implement it in the canonical layer first.

## Docs/landing page

Explain IDD from the outside in:

> **From intent to verified reality.**

Use a real generated/example lifecycle, not marketing-only diagrams disconnected from implementation.

Docs must distinguish:

- human-authored intent;
- derived/inferred work;
- observed reality;
- evidence;
- satisfaction.

Link to actual tests/evidence where the existing site architecture supports it.

## Cross-surface tests

A representative Intent must have the same canonical identity, conditions, derived status/satisfaction and realization lineage through each structured surface.

Do not maintain six golden arrays. Build fixtures from canonical data and test projections/serialization.

## Accessibility/performance

Preserve current Cockpit design-system, responsive and performance requirements. IDD views must not require expensive network/provider discovery merely to render stable project state.
