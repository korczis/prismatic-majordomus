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


# Prompt 05 — Satisfaction, evidence, re-evaluation and drift

## Mission

Close the most important IDD loop: Majordomus must derive whether current reality satisfies Intent. Completion is not a historical checkbox; it is an evidence-backed evaluation that can become false again when reality regresses.

## Satisfaction model

Use the live domain vocabulary, but implement explicit semantics equivalent to:

```text
unknown / not_evaluated
unsatisfied
partially_satisfied (only if meaningful and non-ambiguous)
blocked
satisfied
invalidated/superseded (where domain semantics require)
```

Do not persist an authoritative `satisfied: true`. Persist facts/evidence/events as required; derive satisfaction.

## Inputs

Satisfaction must be computed from:

- Intent success conditions;
- governance-derived obligations;
- current evidence;
- current observable repository/runtime/deployment state;
- validity/freshness of evidence;
- completion state of required realization work only where that is semantically relevant.

A closed issue count alone never proves the Intent.

## Evidence

Reuse the existing evidence/obligation machinery and strengthen it where needed.

Evidence should have typed provenance, evaluator/source, freshness and relation to the condition/obligation it proves.

Prefer direct observations over narrative:

- command result;
- test result;
- coverage artifact;
- git/ref/remote state;
- generated schema consistency;
- deployed URL probe;
- runtime/API response;
- public docs/site evidence;
- other canonical project-specific evaluator.

## Staleness and regression

Prove this lifecycle:

```text
Intent satisfied
→ underlying observable fact changes
→ prior evidence becomes stale/false
→ satisfaction re-evaluates UNSATISFIED
→ diagnostics name the failed condition/evidence
```

Do not mutate historical evidence out of existence. Distinguish historical proof from current validity.

## Drift

Implement/extend diagnostics for at least:

- intent-vs-runtime drift;
- intent-vs-generated/docs claim drift when relevant;
- satisfied state with stale evidence;
- completed work with unmet evidence obligation;
- evidence with no corresponding condition/obligation;
- condition with no evaluator where project policy requires one;
- closed/superseded records whose current projection is contradictory.

Use existing diagnostics/doctor/watch/doctrine infrastructure. No separate warning universe.

## Proof bundle

Create a canonical projection that can explain a satisfied Intent as a bundle/graph of:

```text
Intent condition
→ realization/work
→ session/handover/commit as relevant
→ validation/evaluator
→ evidence/observation
→ current result/freshness
```

This bundle should feed CLI/API/MCP/Cockpit/docs projections rather than be a separate document maintained by hand.

## Tests

Mandatory behavioral coverage:

1. unsatisfied initial state;
2. partial realization does not satisfy the Intent;
3. all work closed but missing milestone/Intent evidence → VERIFY/unsatisfied, not DONE;
4. required evidence becomes valid → satisfied;
5. regression makes it unsatisfied again;
6. stale evidence is detected;
7. explain output names exactly why satisfaction is false;
8. evidence cannot satisfy an unrelated condition;
9. governance obligation participates in satisfaction;
10. historical proof remains inspectable after current regression.
