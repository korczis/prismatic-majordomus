---
id: github-workgraph-phase-04
phase: 4
depends_on: [github-workgraph-phase-03]
goal: deterministic bidirectional reconciliation
---

# Phase 04 — Reconciliation Engine

Read the shared contract and prior handovers.

## Objective

Implement the heart of bidirectional integration: a deterministic planner that compares canonical desired state with observed GitHub state, classifies drift/conflicts, explains them, and only then applies safe mutations.

The reconciler is NOT "call create/update until errors stop".

## Architecture

Target separation:

`Canonical Work Graph + Projection Policy + GitHub Observation → Reconciliation Plan`

then optionally:

`Reconciliation Plan → GitHub Mutations → Re-observe → Verify`

The planning stage must be pure or close to pure and fully testable offline.

## Operation model

Represent planned operations explicitly, e.g. semantic equivalents of:
- create milestone;
- update milestone;
- create issue;
- update projected issue fields;
- attach/detach milestone;
- add/remove managed label;
- open/close issue according to policy;
- record/adopt external identity;
- no-op;
- conflict requiring explicit policy/manual action.

Every operation should include:
- target;
- reason;
- authority;
- expected precondition/observed version when feasible;
- human-readable explanation;
- machine-readable structured detail;
- whether it mutates remote state.

## Drift classification

Differentiate:
- benign formatting normalization;
- canonical-local ahead;
- remote-observed ahead on a mergeable field;
- unsupported remote edit;
- missing remote object;
- stale/deleted remote object;
- mapping conflict;
- semantic conflict;
- permission-blocked state;
- invalid canonical data;
- evidence/completion discrepancy.

## Conflict semantics

For every mergeable field, specify deterministic behavior.

Never silently "last write wins" without explicit timestamps/authority semantics.

For non-mergeable canonical fields:
- remote change should become drift/conflict and be repairable by projection.

For GitHub-authoritative fields:
- import observation to snapshot; do not overwrite from local semantics.

For true mergeable fields:
- implement a documented merge policy or require explicit resolution.

## Dry run

Provide a canonical dry-run capability that:
- performs observation;
- computes plan;
- emits structured and human output;
- makes no mutation;
- exits non-zero only according to clearly documented validation semantics.

This same plan must power Cockpit preview and automation, not a separate diff implementation.

## Apply

Application must:
- apply operations in dependency-safe order;
- be idempotent;
- tolerate already-applied operations;
- stop or continue on partial failures according to explicit policy;
- preserve enough result data for audit;
- re-observe affected objects;
- verify postconditions;
- never report success when verification still shows drift.

## Safety

Introduce protections:
- explicit repository scope;
- no cross-repository mutation due to stale mapping;
- remote object identity check;
- token permissions checked/reported;
- destructive operations classified and, if project policy requires, guarded;
- no mass closure/deletion based on an empty snapshot caused by API failure.

An API failure must never be interpreted as "remote has zero issues".

## Concurrency

Consider:
- two agents planning simultaneously;
- user edits GitHub between plan and apply;
- stale ETag/version;
- retry after partial apply.

Use preconditions/re-observation where provider supports it.

## Idempotence tests

Prove:
1. Plan A against state S creates operations.
2. Apply operations → state S2.
3. Plan against S2 is empty/in-sync.
4. Applying the same safe operation twice does not create duplicate objects.

Test create races and stale mappings where feasible.

## Explainability

Implement or integrate an explanation path:

`why is issue X drifted?`
`why would reconcile update field Y?`
`why won't Majordomus close issue X?`

Return facts + policy + derivation, not just strings like `state mismatch`.

## Gate

Reconciliation of fixture data must be fully deterministic, idempotent, dry-runnable and explainable before enabling broad remote mutation.

Update capabilities/use cases only through canonical mechanisms; document permission and safety semantics.
