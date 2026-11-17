# Milestone 50: Reconcile GitHub Issues and Milestones with Canonical Intents

## Mission

Make GitHub a synchronized projection/work surface of canonical intent planning state, not a second planning universe.

Use the repository's existing GitHub integration, issue sync, milestone sync and conflict policy. Extend them; do not build a fresh GitHub client layer.

## 1. Audit current mapping

Document current canonical relationships among:

- intent;
- internal milestones;
- internal issues/work items;
- GitHub milestones;
- GitHub issues/PRs;
- labels/metadata/backlinks;
- synchronization provenance.

Identify drift cases and ownership rules.

## 2. Identity and mapping

Ensure stable typed mapping exists so reconciliation never depends only on names/titles.

If the repository already has canonical IDs embedded in GitHub bodies/labels/metadata, reuse them.

## 3. Reconciliation semantics

Implement/complete deterministic behavior for:

- canonical internal milestone → GitHub milestone;
- canonical issue → GitHub issue;
- GitHub closure → internal projection/event where permitted;
- canonical rename/update → GitHub update;
- deleted/obsolete work → explicit reconciliation policy;
- external GitHub edits → import, reject or conflict according to repository ownership policy.

Never let a GitHub "closed" checkbox satisfy an intent criterion by itself.

## 4. Intent-aware milestone semantics

A GitHub milestone associated with an intent should expose useful intent linkage, but its closed state is only a project-management projection. Canonical intent satisfaction still comes from evidence.

If the system automatically closes GitHub milestones when an intent completes, make that a derived reconciliation action with provenance and tests.

## 5. Drift diagnostics

Detect at least:

- missing GitHub projection;
- duplicate projection;
- wrong milestone assignment;
- issue mapped to wrong intent/criterion;
- GitHub closed while internal required work remains open;
- internal completion while GitHub projection is stale;
- title/body/metadata drift according to ownership policy.

Provide dry-run/explain output if existing sync architecture supports it.

## 6. Idempotency

Reconciliation must be safely repeatable. Two runs over unchanged state should produce no additional mutations.

## 7. Offline/credential behavior

Core intent semantics must not depend on GitHub availability. Reconciliation should fail/queue/report cleanly without corrupting canonical project state.

## 8. Tests

Use fixtures/fakes consistent with repository practice to test:

- create/update mapping;
- idempotent repeat;
- GitHub closure does not satisfy criterion;
- completed intent projects correct final state;
- external drift detected;
- duplicate mapping rejected;
- missing credentials/network does not invalidate local intent state;
- conflict policy is deterministic.

## 9. Documentation

Explain clearly which side owns which fields and how intent completion relates to GitHub milestone/issue closure.

## Acceptance criteria

- canonical intent plan and GitHub projection can be reconciled deterministically;
- GitHub never becomes satisfaction authority;
- drift is visible/actionable;
- sync is idempotent and tested;
- local intent workflow remains functional offline.
