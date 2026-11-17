# Prismatic Majordomus — Profile System v2

**Execution target:** Claude Code Opus / Fable, working against the *current* `korczis/prismatic-majordomus` repository.  
**Mission:** Extend the existing repository profiles into typed, composable, enforceable, inferred execution policy; derive all surfaces from one canonical resolver.  
**Delivery mode:** Eight sequential, independently reviewable prompts. Do not claim a phase complete without evidence.  
**Source note:** A repository snapshot dated 2026-09-11 contains `.ai/repo/profiles/{routine,implementation,debugging,deep-work}.yaml`, `share/schemas/majordomus/profile/profile.v1.schema.json`, `share/allow/profile.txt`, `test/cases/16_profiles.sh`, profile requirements rules, and claims `docs/claims/profile-axes.md`, `docs/claims/profile-validate.md`. These are historical observations, not assertions about the repository's current HEAD. Current HEAD wins.

## Global contract: apply to every prompt

You are implementing Majordomus by *extending existing Majordomus architecture*, not creating a second configuration, agent, workflow, or execution plane.

Before each phase, inspect `AGENTS.md`, the relevant `.ai/**`, repository doctrines/rules, policy, schemas, existing profile model, CLI implementation, derived registries, intent/plan/execution/session state, MCP, API, mesh, UI/cockpit and tests as they actually exist at HEAD. Respect the current implementation language and module structure; the historical snapshot used shell in some areas, but do not assume it is still authoritative. Prefer typed Rust/domain infrastructure if already present and appropriate, without a gratuitous rewrite. Preserve `routine`, `implementation`, `debugging`, and `deep-work` wherever backwards compatibility requires it. Reuse project-native generators, commands, versioning, CI, docs publishing, release and deployment rather than building a bespoke parallel system.

Invariants:

1. **One truth**: profile definitions and effective resolution share one typed canonical representation; CLI, API, MCP, UI, generated docs, schemas and evidence are derived consumers.
2. **Orthogonal concerns**: Profile (execution strategy), Intent (goal), Policy (allowed/mandatory constraints), Capability (available powers), Peer/Worker (actor), Plan (proposed implementation), Evidence (proof). Never conflate them.
3. **Non-bypassable governance**: repository/doctrine/security policies and intent invariants are hard constraints. An override may tighten, never silently weaken, them. External approval cannot be minted by a profile.
4. **Traceability**: every effective field exposes source, precedence/constraint decision, fingerprint and any fallback/degradation reason; decisions are linked to intent, issue, execution and evidence where these concepts exist.
5. **Determinism**: identical inputs and canonical policy versions produce byte-stable effective outputs even with concurrent discovery.
6. **Actual enforcement**: distinguish `requested`, `resolved`, `attempted`, `observed`, `verified`, `unobservable` and `denied`; never present worker-instruction hints as proven runtime guarantees.
7. **Least privilege**: no profile can expand credentials, tool scope or mutation permissions beyond actual grants. Review and CI should be bounded by default; branch protection, required checks and approvals remain authoritative.
8. **Execution economy**: no network/build at interactive resolution time; use incremental, content-addressed, scoped invalidation if existing infrastructure supports it. Avoid repeated full scans.
9. **No needless surface area**: no duplicated profile tables, no hardcoded frontend semantics, no YAML explosion, no profile proliferation, no hand-maintained client DTOs when generated DTOs exist.
10. **Prove change**: tests, docs, gates, versioning and release claims must be supported by executed commands, logs and externally observable state; do not claim deployment if external access blocks it.

Use small, independently mergeable commits/PRs following repository workflow. Run the fastest relevant targeted tests on every edit, broader gates before merging. Do not weaken checks to go green, do not indiscriminately rebase other people's dirty worktrees, do not auto-merge despite required gates. If blocked, document the exact blocker, evidence and safe next action.

### Intended user outcomes

- `majordomus profile list`, `show`, `resolve`, `explain`, `validate`, where these commands do not duplicate existing equivalent commands.
- `majordomus ui`: high-level profile chooser displaying effective constraints, changes and provenance, attached to canonical intent/session/execution state.
- Policies compose and remain deterministic under intent overrides and runtime constraints.
- Session and mesh executions enforce resolved limits at the actual dispatch/adapter boundary, and expose truthful observational evidence.
- Profiles are discoverable in existing API/MCP capabilities without manual duplicate registration.
- Defaults are safe, fast and ergonomic. The user can work interactively, request autonomous execution, perform adversarial review, integrate safely and release only through protected gates.

---

# PROMPT 00 — Reality audit, compatibility matrix and architecture decision

**Goal:** Map the implementation at current HEAD. Decide where the existing profile subsystem belongs in the current control plane. Output an implementation-ready architecture contract; make only necessary preparatory code changes.

1. Inventory all profile entry points and configuration: `.ai/repo/profiles`, `share/skeleton/profiles`, allowlist/schema, repository policy default, `init`, `update`, `start --profile`, `check --explain`, `context`, `finish`, `doctor`, generated docs/claims, CI, tests. Check which remain current; identify migrations in place.
2. Inventory current intent, plan, critique, milestones/issues, execution scheduler/dispatch, mesh peers, capabilities, model provider routing, approvals, API, MCP, UI projections, derived registries, cache and events. Record file/module paths and actual supported operations.
3. Trace a profile from stored YAML through validation -> effective task state -> worker/session context -> dispatch -> observable enforcement -> finish/verification -> UI. Mark each link: proven, advisory, missing, duplicated or stale.
4. Audit existing enforcement and definitions: capability, reasoning effort, verbosity, context selection, verification, checkpoint interval and output contract. Do not erase these independent axes.
5. Establish a compatibility/migration matrix for existing profiles, configs, CLI commands, schemas, session records, event types, consumers and published documentation. Identify schema-version upgrades and fallbacks.
6. Identify hard policy authorities, order of evaluation, genuine human approvals and privileged mutations. Mark where no trusted authorization mechanism exists.
7. Benchmark current profile read/validate, start and UI profile view, if available. Report cold/warm p50/p95 with reproducible fixture and measurement technique.
8. Write a focused ADR or contract document *using repo's ADR convention* that names the owner of the profile resolver, persistence model, decision provenance, resolver invariants, schema evolution and boundaries between profile intent and runtime enforcement.
9. Produce a realistic implementation sequence. List surprises and delete redundant assumptions from subsequent phases if current HEAD already implements them.

**Deliverables:** audited evidence table (path, current behavior, owner, consumer, gap); concrete effective-resolution contract; backward-compatibility plan; benchmark baseline; issues/PR boundaries with acceptance criteria. Do not claim historical snapshot facts still hold without checking.

**Exit gate:** Audit is supported by exact repository paths and executable demonstrations, with a selected canonical integration point and no speculative greenfield subsystem.

---

# PROMPT 01 — Typed canonical ProfileResolver v2

**Goal:** Add or extend one resolver producing a deterministic, constraint-safe EffectiveProfile; integrate it with the existing type/schema/projection model.

**Conceptual model, adapt names/types to HEAD:**

```rust
struct EffectiveProfile {
    schema_version: SchemaVersion,
    definition_id: ProfileId,
    definition_revision: ContentDigest,
    intent_id: Option<IntentId>,
    session_id: Option<SessionId>,
    axes: ProfileAxes,
    execution: ExecutionStrategy,
    constraints: EffectiveConstraints,
    provenance: Vec<ResolutionDecision>,
    diagnostics: Vec<Diagnostic>,
    fingerprint: ContentDigest,
}
```

Prefer existing types. `ProfileAxes` must preserve legacy effort, capability class, verbosity, context, verification, checkpoint and output contract. Additional axes may represent execution strategy, allowed actions, approvals, budgets, routing preferences, concurrency/retry parameters, isolation or observability **only where a current consumer exists or the work requires it**.

Inputs: canonical repo policy/doctrines; profile definition; intent invariants/requirements; session/task overrides; runtime peer capabilities, budgets, hard limits and available permissions. A task-scoped override cannot make a forbidden action legal. Formalize resolution as constraints/intersections for permissions and limits; use precedence only for soft preferences and defaults.

Implement:

- Typed parse + validation and schema generation/reuse; reject unknown, conflicting, unresolvable values. Preserve or explicitly migrate legacy `profile.v1` inputs.
- Stable deterministic resolution with sorted keys, normalization and full final tie-breakers. Fingerprint must include relevant policy/profile versions, canonical input content and resolver schema, but not volatile clocks.
- A field-level explain trace: raw input source -> normalization -> constraint intersection -> winner/default -> effective field. Include reason codes and structured diagnostics, not prose-only decisions.
- Explicit handling of missing worker capability, stale runtime state, unavailable provider, insufficient budget, absent approval and conflicting hard constraints. Fail closed where required; degrade only when a policy-authorized fallback exists.
- Resolver output serialization reused by CLI/API/MCP/UI; generated schema and wire-compatibility tests.
- Cache and invalidation keyed by immutable inputs. Runtime-changing constraints must not be hidden by a stale cache. Never expose secret values in snapshots or trace.
- Cycle/ambiguity detection if inheritance/composition is introduced. Avoid multiple inheritance until justified; prefer independent axis composition over dozens of named combinations.

**Tests:** shuffled enumeration property tests, conflict matrix, policy-weakening negative tests, legacy round-trip, stable fingerprint, cache hit/miss equality, secret-redaction, unknown axes, missing profile, missing capability, runtime constraint change, branch/worktree isolation and schema migration.

**Exit gate:** A single tested typed resolver exists, can explain every effective constraint and preserves legacy usage. No UI/CLI consumer invents a second version of resolution.

---

# PROMPT 02 — Canonical definitions, defaults and migration

**Goal:** Migrate the existing profile catalog, preserving legacy behavior where valid, and introduce a small orthogonal vocabulary rather than duplicating profiles combinatorially.

Start from current real profiles (`routine`, `implementation`, `debugging`, `deep-work` in the historical snapshot) and existing `.ai/repo/policy.yaml` profile default. Determine their true meaning and do not silently redefine them. Provide compatibility aliases only if migration requires them and show deprecation with a timeline.

Proposed strategy dimensions (names illustrative, not an instruction to create all files):

- Work mode: `interactive`, `autonomous`, `review`, `integration`, `release`, `ci`, `exploration`.
- Autonomy grant: `supervised`, `propose`, `execute`, `integrate` or the canonical vocabulary already in repo.
- Verification: reuse existing task profile verification axes; choose test set dynamically from changed scope/intent risk and constraints.
- Performance/resource class: `fast-feedback`, `balanced`, `thorough` where needed, governed by hard budgets.
- Routing preferences: model capability class, effort, context, verbosity, cost and availability; provider selection only when runtime actually supports it.

Never implement the Cartesian product as named profiles. Do not blindly create seven new YAML files. Treat modes as optional curated presets over a typed axis model; preserve legacy existing profiles. Define defaults by context only if derivable and explainable. No auto-escalation to `release` or privileged `integrate` merely because a task looks like release work.

**Migration steps:** discover legacy versions; read real schema and allowlists; write deterministic migrator if needed; preserve existing task/session records; version outputs if wire shape changes; keep bootstrap/init/update idempotent; update skeleton and repo-level profiles through canonical mechanism; fix drift in docs/generated site; ensure custom user profiles are not overwritten on update; support downlevel reads or clear migration errors as warranted.

**Proof:** init fresh project, upgrade old fixture, custom profile untouched, unknown profile rejected, profile filename mismatch rejected, missing default rejected, legacy commands work, new profile participates in derived inventory automatically, no manually maintained duplicate profile lists. Run existing `test/cases/16_profiles.sh` or its current replacement plus new negative tests.

**Exit gate:** Existing profiles continue working, extension is orthogonal, and users can opt into new semantics without creating conflicting copies of governance policy.

---

# PROMPT 03 — Bind profiles to intent, session, execution and mesh

**Goal:** Replace advisory-only execution claims with enforceable runtime semantics while truthfully reporting what cannot be observed.

Trace the current path: intent -> critique/preflight -> plan -> milestones/issues -> task/session -> execution request -> peer scheduler -> adapter/tool/provider invocation -> events/evidence -> verification/finish. Resolve and freeze the effective profile at the appropriate lifecycle boundary; model subsequent policy changes as invalidation, reauthorization or a versioned re-resolution, not silent mutation of active context.

Implement:

1. Attach effective profile ID, revision/fingerprint and resolution trace reference to the existing canonical intent/task/session/execution records, not a new competing database. Carry correlation IDs end-to-end.
2. Enforce allowed tool/action/side effect boundaries at the trusted dispatcher and actual adapter invocation. Explicitly distinguish tool advertised capability vs granted capability. Bound budget, timeout, max parallelism, attempts/retries and scopes wherever enforcement is actually supported. If runtime cannot enforce one axis (e.g. hidden model reasoning or output verbosity), mark it `requested/unobservable`; never report `verified`.
3. Runtime capability negotiation: choose eligible peers from existing mesh capability inventory; select policy-compliant fallback only if authorized. Handle late disconnects, lease expiry, duplicate dispatch, network partitions and delayed/out-of-order events. Prevent re-execution of non-idempotent mutations unless safely mediated.
4. Honor existing intent invariants and critique/approval gates before work starts and before integration/release side effects. Releases never bypass protected branch, check status, explicit required approvals or environment protections.
5. Route adversarial review to read-only or proposed-change mode by default. Separate reviewer from implementer where the existing trust architecture supports it; never manufacture an independent review when the same actor performed both roles.
6. Integrate verification with actual finish policy: required tests, evidence closure, impacted docs, change trace, failure diagnostics and stage transitions. A profile cannot excuse a mandatory check.
7. Emit structured events from existing event/ledger infrastructure: effective_profile_resolved, constraint_denied, execution_dispatched, budget_exhausted, approval_required, fallback_selected, capability_missing, verification_failed, and existing equivalent names. No new event bus.
8. Backward compatibility: old sessions must remain readable; historical advisory assertions remain advisory in evidence.

**Adversarial scenarios:** attempt unauthorized tool invocation through a worker; profile override weakens security; stale policy revision; offline peer; disconnected coordinator; approval missing; second job races on the same worktree; duplicate mutation; budget exceeded during parallel dispatch; audit log unavailable; provider claiming unsupported effort. Verify fail-safe behavior and honest telemetry.

**Exit gate:** A profile has at least one demonstrable, observable effect on dispatch and one reliable refusal at a trusted boundary; runtime evidence distinguishes aspirational instructions from verified enforcement.

---

# PROMPT 04 — Derived CLI, API, OpenAPI and MCP

**Goal:** Expose one profile subsystem consistently across existing surfaces without duplicate handlers, catalogues or semantics.

Discover first how current command registries, API projection generator, OpenAPI/Swagger generation and MCP tools/resources are built. Extend those paths, not a parallel RPC tree.

Desired CLI interaction (adapt to real commands and avoid collisions):

```text
majordomus profile list [--json]
majordomus profile show <profile> [--json]
majordomus profile validate [--all | <profile>]
majordomus profile resolve [--intent ID] [--session ID] [--profile NAME] [--json]
majordomus profile explain [--intent ID] [--session ID] [--profile NAME] [--json]
```

`resolve` and `explain` are read-only. Mutation such as profile selection must use existing task/session/intent update commands and their audited permissions. Avoid globally mutable `profile use` unless its scope and persistence are explicit and policy-compliant.

Define canonical capabilities for profile discovery, effective profile reads, preview and, only when authorized, profile selection. Automatically project equivalent operations to CLI / API / OpenAPI / MCP / UI by existing generator. Explicitly document API errors for invalid profile, insufficient grant, policy collision, stale revision, unknown intent and unsupported peer. Tool schemas must derive from canonical input/output types and preserve authorization. No secrets in public snapshots. Test JSON output and REST/MCP equivalence on stable fixtures.

Use structured diff (`current` vs `candidate` effective profile) for previews with precise source/provenance and whether execution must restart/reauthorize. Avoid leaking low-level internal paths to public users while retaining diagnostic IDs for admins.

**Performance:** no remote call to list local definitions; resolver warm target p95 <= 50ms and cold target p95 <= 150ms for representative local fixtures as *targets subject to benchmark*, not unsupported claims. If targets are unrealistic, report measurement and isolate bottleneck; don't cheat benchmarks. Existing runtime endpoints should remain responsive with pagination and bounded streaming.

**Exit gate:** Same profile and same effective resolution fingerprint through CLI structured output, REST, MCP and UI data projection; a new custom profile appears without per-surface registration; OpenAPI is generated from canonical schemas.

---

# PROMPT 05 — Majordomus UI: high-level profile-driven control plane

**Goal:** Deliver a mobile-first interactive profile experience within existing `majordomus ui`, derived from canonical backend state, using existing framework, components and routing at HEAD. DO NOT rewrite the UI stack just to add a selector.

Read actual UI/cockpit architecture and `.ai/` UI rules first. Reuse its existing derived page conventions, router, cache, optimistic update patterns, stream transport and design tokens. If UI is currently Rust based, extend that framework; if it uses Alpine/Flowbite/Cytoscape/ECharts or other existing libraries, reuse only where useful. No separate frontend state machine storing the canonical profile or policy.

Required user workflows:

- Choose an intent/session/task; view currently *effective* profile and where it came from.
- Discover profiles and modes: descriptive, concise, searchable, keyboard-accessible, mobile usable. Differentiate profile names from autonomy grants and budget preferences.
- Preview what would change before activation: tool privileges, approvals, context, resource budget, verification, model-routing *requests*, task impact, policy conflicts and restart/re-authorization needs.
- Select an authorized profile only via existing canonical mutation/event/command path. Include expected revision for optimistic concurrency, show rejection on stale state and never claim success before canonical acknowledgement.
- Explain *why* a setting is effective (repo policy, intent requirement, definition, session override, worker capability, runtime safety bound). Provenance can be expanded without cluttering primary high-level UX.
- Observe live profile-linked execution: dispatch, peer, grants, budget consumption, checkpoint, attempts, evidence and verified vs unobservable guarantees. Subscribe to existing event stream; reconnect/resume using cursors if supported; show explicit stale state.
- Link to intent, plan, issue, milestone, peer, execution, test and release records via canonical IDs and established route projections.
- Protect mobile UX: touch targets, keyboard navigation, screen-reader labels, drawers/dialog focus trapping, horizontal overflow, loading/error/empty states, reconnects, long lists and reduced motion.

No manual frontend profile inventory. No local policy calculation, ad hoc `fetch` loops or JS duplicate of Rust/domain enums. Use SSR/skeleton/progressive enhancement and scoped caching if current design supports them. Prefer SSE/WebSocket stream already implemented; don't open a new transport.

**Acceptance flow:** create a test intent, see a default effective profile, preview a restrictive review profile, switch if authorized, watch derived event update across two clients, attempt a forbidden privileged profile, see precise policy rejection, inspect trace and execution evidence. Verify with browser/E2E tests on phone and desktop viewport; measure initial load and interaction latency vs baseline from Prompt 00.

**Exit gate:** One functional, fast, mobile-friendly profile workflow integrated with existing Majordomus UI and no shadow configuration state.

---

# PROMPT 06 — Enforcement, proof, DX and performance

**Goal:** Turn all v2 invariants into executable regression prevention. Tests must prove the architecture, not merely parse a YAML fixture.

1. Extend actual `.ai/` doctrines/rules/validators to prevent a profile weakening mandatory policy and prevent consumer-specific duplicate inventories/resolvers. Do not add Markdown-only doctrines disconnected from gates.
2. Add focused Rust/domain tests, property tests where appropriate, end-to-end commands, negative tests, API contract, MCP parity, UI E2E and mesh simulation/fault injection; reuse existing fixture/test architecture. Test deterministic output after source-order randomization and across process boundaries.
3. Exercise existing profile matrix (routine, implementation, debugging, deep-work), custom profile registration, missing default, unknown key, invalid schema, schema migration, invalid precedence, contradictory constraints, provider fallback, readonly review, protected integration/release, stale snapshot, secret leakage, delayed event, interrupted process, corrupted cache, cross-repo isolation and worker incapability.
4. Create a **truthfulness matrix** for each axis: configured, passed to worker, accepted by adapter, instrumented, externally verified, unknown. Explicitly document where tokens, effort, model choice, output verbosity or context consumption cannot be reliably observed. Do not turn model self-report into evidence.
5. Benchmark resolution, p95 API latency, UI load/interaction, cache invalidation and dispatch overhead. Compare before/after with same fixtures/hardware/commands; profile resolution must not invoke network, build or providers at shell-entry time.
6. Test concurrent mutations (CAS/version conflict), stale policy, clock changes, hashed revision changes, replay, no double dispatch, safe cancellation and crash recovery where supported.
7. Test zero-registration extensibility: add new fixture profile, assert derived CLI/API/MCP/UI/docs discovery without changing any consumer-specific list.
8. Gate fast tests incrementally (changed-scope selectors), reserve broad integration for appropriate CI gate. Optimize feedback time with existing cache, not disabled checks.
9. Validate user documentation, generated docs, site links, schemas and CLI help are derived and coherent. Add migration troubleshooting and explanation examples.

**Exit gate:** CI catches a deliberately injected unauthorized escalation, nondeterministic resolution, duplicate frontend catalogue, schema drift and inconsistent CLI/API/MCP output. Record red/green evidence. Quality gates are reliable and fast enough for rapid turns.

---

# PROMPT 07 — Dogfood, release, deployment and final evidence

**Goal:** Prove the system by using it to deliver a real but bounded Majordomus change, then ship safely through repository-native workflow.

1. Select a low-risk existing issue or a focused authorized dogfood task, linked to a canonical intent if the workflow supports it. Do not fabricate issue IDs or create public project work without authorization/context.
2. Resolve effective profile from real repo governance and task context. Show field-level provenance and fingerprint. Run the same task in interactive proposal mode and an authorized autonomous execution mode; compare action grants, verification requirements, model-routing requests and budgets. Perform a review profile pass and record real findings.
3. Demonstrate an integration profile *dry-run* (rebase/merge decision with gate checks); perform actual merge only when required review/check/branch protections are satisfied and action is authorized. Never force push protected branches or circumvent branch protection to satisfy this prompt.
4. Perform a deliberately denied action under a restrictive profile, capture structured denial and the corresponding UI/CLI/API evidence. Demonstrate observed actual dispatch controls, not worker assertions.
5. Run focused + relevant full tests, schema/generator drift gate, docs preview, CLI/MCP/API/UI E2E and cache/performance regressions. Include exact commands, exit codes, p50/p95 sample and limitations.
6. Update canonical docs, `.ai/` doctrine/rule enforcement, claims/ADRs when applicable; regenerate published site from the canonical source. Ensure secrets redacted.
7. Create reviewable PRs or follow repo merge train. Merge only behind required green checks; update version/changelog according to existing semantic version doctrine *if applicable*. Publish/deploy through existing mechanisms and verify observable live endpoint/version where access exists. Never assume deployment solely from successful local builds.
8. Close or update tracked work only after evidence is attached. Reconcile release and claims ledger. Clean up only owned, safely merged worktrees/branches.
9. Final report in this order: baseline and root cause; files/architecture changed; migrations; supported axes; enforcement matrix (requested vs observed vs verified); CLI/API/MCP/UI parity evidence; tests/CI; performance delta; PRs/commits/version; release/deploy verification; residual risks and blocked actions.

**Exit gate:** A demonstrable profile-governed development cycle, with genuine guardrails and honest evidence. No unsubstantiated `done`, `merged`, `published`, `deployed` or `verified` statuses.

---

# Cross-phase acceptance checklist

- [ ] Existing profile foundations were audited and reused, not replaced by a second system.
- [ ] Old profile definitions and historical sessions remain compatible or have a tested migration.
- [ ] Typed canonical EffectiveProfile resolves deterministically with field-level provenance and content fingerprint.
- [ ] Intent, session, execution and mesh carry the same effective profile identity.
- [ ] Hard policy/permission intersection cannot be bypassed through profile selection.
- [ ] At least one observable execution parameter is actively enforced; unsupported parameters are truthfully labelled.
- [ ] CLI/API/OpenAPI/MCP/UI derive the same canonical registry and wire types.
- [ ] UI previews profile differences and follows canonical live state, mobile-first.
- [ ] New custom profiles propagate without manual per-surface registrations.
- [ ] Property, negative, contract, mesh and E2E tests protect these invariants.
- [ ] Benchmarks are measured rather than advertised.
- [ ] `.ai/` governance is executable, site/docs generated, release/deploy claims evidenced.

# Paste-ready orchestration instruction

Execute PROMPT 00 through PROMPT 07 sequentially against the current repository. Treat each as a bounded implementation phase with its own tests, commits and evidence. Between phases re-read changed `.ai/` rules and any revised architecture contracts. Prefer existing canonical primitives and delete duplicated machinery. On contradictions, follow current repository facts and stronger governance constraints, and document deviations from the prompt. Do not stop after writing an architecture document if implementation is feasible; keep iterating through tests and integration. Never invent results or bypass authorization, required approvals or deployment protections. Deliver a final cross-phase evidence report that links every completed claim to actual repository state.
