# Phase 09: Canonical Domain, Event, State Machine and Storage Contracts

## Mission
Inspect existing Rust domain definitions before changing any type. Do not adopt the following names literally if the repository already has equivalent types. Establish precise ownership and state transitions for intent, plan, execution, attempt, task, claim, PR, release, deployment and evidence. Canonical facts must survive process restart and must be explainable.

## Mandatory reconnaissance
- Find the canonical intent types, criterion/evidence records, milestone/issues, readiness/dependency graph, execution model, event journal and persistence repository.
- Identify each state field's writer and every derived reader. Flag multiple writers or fields that represent derivable aggregates.
- Determine what is globally durable vs per-checkout local vs per-mesh-replica. `.ai/repo` is NOT a global operational database; `.ai/local` is NOT public policy.
- Find DB/schema migrations, concurrency model, transactional guarantees, sequence/version semantics and sync conflicts.
- Record authoritative GitHub IDs/URLs, deployment IDs and runtime IDs; distinguish stable IDs from peer board display slots.
- Identify the exact existing capability/projection invariants and tests.

## Reference entity model (adapt to reality)
Intent:
- stable canonical ID, project/repo identity, authored statement, source/provenance, scope
- invariant set, measurable criteria, acceptable evidence predicates
- policy reference, requested autonomy tier, authorization reference
- relationship edges to plan revisions, milestones, issues, executions and evidence
- revision / optimistic concurrency token; never mutable free-form status as authoritative truth

Plan revision:
- revision ID, parent revision, proposed by, critic assessment, source intent version
- diff/impact summary, milestone graph, issue graph, dependency graph
- risk, effort/cost/time budgets, assumptions, confidence, alternative options
- approval provenance, effective policy, frozen execution contract

Execution:
- execution ID, intent ID, plan revision ID, initiating actor, authorization decision
- ordered event cursor, attempt and child-operation references
- current projection: ready, blocked, running, reconciling, paused, completed, failed, cancelled
- end condition and evidence references, never inferred from an agent merely reporting "done"

Attempt:
- identity distinct from task and execution, stable attempt sequence, lease epoch/fencing token
- worker identity, checkout/worktree identity, capability/tool version, environment digest
- start/end observations, heartbeat monotonic lease times, output artifact URIs
- retry classification and reason, outcome, external side-effect receipts

External effect:
- deterministic operation identity and idempotency key
- target identity (GitHub PR, release, deployment, etc.)
- request intent, command/capability signature, actor, policy decision
- observed provider receipts, reconcile status, explicit ambiguity state
- repeated requests must re-check target state before acting

Evidence:
- stable ID, criterion ID(s), source kind, producer identity, immutable artifact digest
- subject commit SHA, target environment/version, timestamps with origin
- verification method and freshness window, trust tier and revocation/supersession chain
- signed attestation if infrastructure supports it
- evidence is not automatically valid merely because an AI agent created it

## Event conventions
Events have `event_id`, `stream_id`, `stream_seq`, `schema_version`, `type`, `occurred_at`, `recorded_at`, `correlation_id`, `causation_id`, `actor`, `origin`, `payload_digest`, and typed payload.
Use current event infrastructure; extend minimally.
Define optimistic concurrency using expected stream version, append-once semantics and replay.
Reducers must be pure and deterministic. Event serialization must be schema-versioned.
Store observed external receipts rather than claiming atomic transactions spanning GitHub and local state.
Support idempotency across duplicate delivery, out-of-order arrival and repeated notifications.
No secret/token/raw prompt in event payload.

## Execution transition contract
Proposed statuses:
- `draft -> proposed -> reviewed -> authorized -> queued`
- `queued -> running` only after policy preflight and valid lease/claim
- `running -> paused` by authorized command, pause at safe checkpoint
- `running -> blocked` on dependencies, missing auth, health or proof
- `running -> reconciling` on uncertain remote effect or restart
- `reconciling -> running|blocked|failed|completed` after state reconciliation
- `running -> completed` only after terminal plan tasks and required evidence checks
- `any nonterminal -> cancelled` after bounded best-effort drain/compensation
- `completed` is NOT synonymous with `released`, `deployed` or `intent satisfied`.
Adapt states to existing contracts; do not introduce duplicate enum families.
Every transition carries actor, policy, reason, correlation and evidence.

## Invariants
1. An event is never overwritten to erase failure; compensating/superseding event is emitted.
2. Only one effective active attempt can commit the same side effect for one task/epoch.
3. Lease expiry alone does not prove an external operation did not complete.
4. An `at-least-once` delivery channel is never described as `exactly-once`.
5. Derived completion is monotonic only when its source and freshness policy permit; revoked evidence can unsatisfy intent.
6. A deployment cannot be marked verified by merely creating a GitHub release.
7. A reconciler always checks external reality before replaying side effects.
8. Scope and authorization are checked at time of effect, not only at intent creation.
9. An old worker epoch cannot complete a newer worker's lease.
10. Every state visible in the UI has provenance/explain link.

## Tests
- reducer replay from shuffled delivery while preserving canonical sequence
- duplicate delivery / duplicate persisted notification
- schema migration forward and, where supported, compatibility readers
- optimistic locking conflict
- stale worker fencing
- lost acknowledgement after successful remote call
- interrupted writes and journal replay
- cancelled task receiving late completion
- evidence stale, revoked, superseded, wrong commit/environment
- startup reconciliation against GitHub and deployment provider
- deterministic structured snapshots before/after restart

## Exit gate
A single typed execution object can be reconstructed from persisted events and links to immutable evidence. A human can explain why every transition occurred using CLI/API/UI. Do not claim an exactly-once system without atomic proof.
