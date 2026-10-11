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


# Prompt 00 — Master orchestrator: complete IDD end-to-end

## Mission

Own the complete delivery of Intent Driven Development v1 in Majordomus. Use the live repository to discover the exact implementation state, then execute the whole convergence until the canonical branch and deployed/public surfaces prove the feature.

Do not treat the phases below as documentation milestones. They are one closed implementation loop.

## Phase A — establish truth

Perform a repository-wide audit of all intent-like state:

- milestone problem/outcome/current_state/desired_state/scope/non_scope/acceptance/validation/evidence;
- issue objective/why/current_state/desired_state/scope/dependencies/acceptance/validation/evidence;
- task/session objectives and context;
- handover objective/current-state/next-action and inherited blockers;
- peer announcements/claims;
- evidence obligations and finish semantics;
- roadmap/work graph state;
- GitHub issue/milestone projection;
- claims, capabilities, docs/site and Cockpit views.

Produce a migration map that identifies canonical owners, duplicate semantics, current writers/readers and what must be reused, migrated or removed.

## Phase B — canonical Intent

Establish a first-class typed Intent semantic model. Reuse/evolve the closest existing canonical record rather than wrapping milestones 1:1 without value.

The model must represent or link to:

- stable identity;
- intent statement/outcome;
- rationale/problem;
- current and desired state;
- scope/non-scope;
- constraints;
- success/acceptance conditions;
- evidence obligations;
- explicit/derived/inferred/observed provenance;
- parent/child or strategic/execution/operational relationships where needed;
- realization lineage to milestones/issues/sessions/handovers;
- derived satisfaction and explanation.

No authoritative hand-written `status`/`satisfied` boolean.

## Phase C — intent compiler

Implement the smallest inspectable compiler/projection that can relate:

```text
Intent + observed current state + governance
→ gaps / obligations
→ linked or derived executable work
→ dependency graph / execution waves
```

Do not build a magical opaque planner. Every derived work item must carry provenance and lineage explaining why it exists. Preserve manual authorship when work is explicitly authored.

## Phase D — execution continuity

Bind work execution to Intent lineage across:

- task start;
- session lifecycle;
- context compilation;
- peer/mesh announcements;
- scope claims/conflicts;
- handovers;
- provider switches;
- worktrees/branches/commits.

A receiving session must be able to recover the Intent, relevant conditions, unsatisfied obligations, decisions/evidence and next action without reconstructing them from chat history.

## Phase E — satisfaction loop

Implement current-state evaluation:

```text
success conditions + governance obligations + current evidence/observations
→ derived satisfaction
```

Satisfaction must be re-evaluable. A previously satisfied Intent can become unsatisfied if current observable reality regresses or evidence goes stale.

Create actionable drift diagnostics for at least:

- closed/completed work with missing required evidence;
- satisfied Intent whose evidence is stale/false;
- runtime/docs/claims contradicting desired state;
- derived work orphaned from its Intent where lineage is required;
- handover/session state disconnected from active Intent realization.

## Phase F — projections

Project the same canonical semantics into all relevant existing surfaces:

- CLI and structured JSON;
- REST API;
- generated OpenAPI/Swagger;
- MCP tools/resources;
- Cockpit;
- generated docs/GitHub Pages;
- GitHub planning projection if it exists.

No per-surface semantic duplicates.

Cockpit must answer, from canonical backend data:

- what is the Intent?
- how does current reality differ from desired state?
- what work realizes it?
- which peers/sessions are active?
- what handovers occurred?
- what evidence exists?
- what remains unsatisfied?
- why does the system derive the current satisfaction state?

## Phase G — enforcement and tests

Use current rules/doctrines/gates to make these invariants non-optional. Extend canonical enforcement rather than creating a disconnected validator.

At minimum prove:

1. canonical Intent schema/provenance;
2. no authoritative manual satisfaction status;
3. work lineage to Intent;
4. handover continuity;
5. cross-provider continuation;
6. evidence-gated satisfaction;
7. satisfied → regression → unsatisfied re-evaluation;
8. cross-surface consistency;
9. automatic propagation of a newly added Intent/work entity without consumer registration;
10. generated artifact drift detection;
11. deployment/public projection verification where claimed.

## Phase H — dogfood

Use the current Majordomus task/milestone/issue/Intent mechanisms to track this implementation itself. If the system cannot use its own workflow to deliver IDD, fix that as part of the mission.

## Phase I — land and verify

Bring all in-scope branches/worktrees/PRs into a coherent canonical result according to repository workflow. Remove obsolete duplicate implementations. Run complete gates. Deploy the relevant site/Cockpit/runtime surfaces. Verify public URLs/endpoints and capture evidence.

Do not claim success from a local green test suite alone.

## Final acceptance proof

Produce one real E2E fixture/scenario demonstrating:

```text
Intent created
→ reality discovered
→ executable work derived/linked
→ provider/session A executes
→ handover
→ provider/session B continues
→ verification observes failure
→ Intent remains UNSATISFIED
→ remaining work/fix executed
→ verification observes success
→ Intent becomes SATISFIED
→ identical lineage/proof visible through canonical structured surfaces
```

## Final report

Report only evidenced state:

- root cause of pre-IDD fragmentation;
- canonical model chosen and why;
- migrations/removals;
- intent compiler behavior;
- execution continuity proof;
- satisfaction/re-evaluation proof;
- enforcement wiring;
- cross-surface proof;
- exact test/gate/deploy commands and results;
- public/runtime verification evidence;
- remaining limitations, if any.

Do not report aspirational behavior as shipped.
