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


# Prompt 07 — Enforcement, regression barriers and E2E proof

## Mission

Make IDD non-optional and non-decorative. If the repository can drift back to prompt-driven/manual-status behavior without a red gate, the architecture is unfinished.

## Governance invariants

Encode these through the live rule/doctrine/policy system, using existing doctrine dispatch and finish/check/doctor/watch integration rather than bespoke hook scripts:

1. Human-authored intent and machine-derived/inferred/observed state remain distinguishable.
2. Canonical Intent semantics have one owner; equivalent surface-specific copies are forbidden or generated.
3. Required realization work retains Intent lineage.
4. Sessions/providers do not own canonical Intent.
5. Handovers preserve realization lineage and unresolved obligations.
6. Completion/satisfaction is derived from evidence/current state, never manually asserted.
7. Machine-observable facts are derived/observed instead of duplicated where practical.
8. Generated projections must be current.
9. Claimed product/public behavior must have behavioral test evidence.
10. Cross-provider continuity claims require actual adapter/session integration tests.

## Wiring

Ensure every new validator is registered by a doctrine/rule and every declared doctrine is actually wired to the commands/gates that must enforce it. Preserve the repository's "validator without doctrine" and "doctrine without test" failure philosophy if still current.

## E2E canonical scenario

Build one maintained fixture/use-case that runs the full lifecycle:

```text
1. Create/declare a canonical Intent with at least two conditions.
2. Discover initial reality: both unsatisfied.
3. Derive/link work and dependency graph.
4. Start provider/session A and implement condition A.
5. Produce a handover with one unresolved condition/obligation.
6. Start provider/session B; resolve the same Intent lineage automatically.
7. Complete implementation but deliberately leave verification/evidence for condition B failing.
8. Assert work may be VERIFY/complete as appropriate, but Intent is UNSATISFIED.
9. Derive/show remaining required work.
10. Fix condition B.
11. Run verification/evidence collection.
12. Derive Intent SATISFIED.
13. Mutate/break the underlying observed fact.
14. Re-evaluate and assert Intent becomes UNSATISFIED again.
15. Repair and restore SATISFIED.
16. Assert consistent structured state through CLI/API/MCP/Cockpit backend/docs projection.
```

This is the IDD proof. Do not replace it with a dozen isolated unit tests.

## Existing suites

Discover and extend current equivalents of handover, continuity, session-context, DAG/waves, evidence, obligations, strict derivation, canonical architecture, cross-surface, OpenAPI, MCP shared server, peer context, executable issues and public site/deploy tests.

## Negative tests

Deliberately break:

- lineage;
- evidence freshness;
- schema;
- doctrine wiring;
- generated projection;
- cross-surface identity;
- handover continuity;
- manual status/satisfaction insertion;
- duplicate consumer registry.

The relevant canonical gate must fail loudly and actionably.

## Performance

Ensure intent/context/evaluation does not make repository entry/context/status commands unacceptably slow. No network/provider calls on hot paths unless explicitly requested. Use current budgets/bench infrastructure.

## Zero-registration test

Add a representative Intent/work entity and prove it appears automatically in every relevant derived consumer without editing consumer-specific inventories.

## Completion

Run all repository-level gates, not only focused tests. If unrelated pre-existing failures exist, isolate and evidence them; do not silently weaken the contract.
