---
id: github-workgraph-phase-10
phase: 10
depends_on: [github-workgraph-phase-09]
goal: legacy migration and safe backfill
---

# Phase 10 — Legacy Migration, Adoption, and Backfill

Read shared contract and prior handovers.

## Objective

Bring existing repository/GitHub reality under the new model without destructive guesses.

This is not optional cleanup. A new invariant that ignores all existing objects is theater.

## Inventory legacy state

Collect, through safe read-only mechanisms:
- canonical/local issue/milestone metadata;
- GitHub milestones/issues;
- open/closed PRs relevant to current history;
- feature branches;
- worktrees;
- mappings/markers currently used;
- generated docs/indexes;
- old status fields;
- scripts/tools that encoded relationships.

## Classify objects

Each legacy object should become one of:
- confidently mapped;
- canonical-only needing projection;
- GitHub-only adoption candidate;
- unmanaged/external;
- ambiguous;
- obsolete;
- invalid requiring operator action.

Never map solely by fuzzy title similarity.

## Deterministic mapping signals

Use strong evidence in descending reliability:
- existing canonical IDs/markers;
- explicit stored external IDs;
- existing branch/worktree issue associations;
- PR closing-link metadata;
- repository-defined legacy mapping files;
- stable external node IDs;
- deliberate operator mapping.

Weak hints can be shown, not silently accepted.

## Migration planner

Like reconciliation, migration should have:
- dry-run;
- structured operations;
- reasons/confidence;
- conflicts;
- no mutation until apply;
- idempotence;
- post-apply verification.

Potential operations:
- create missing canonical external mapping;
- add canonical marker to managed GitHub object;
- normalize projected body/labels;
- associate milestone;
- register legacy work item in canonical form through the existing schema;
- retire obsolete duplicate metadata.

## Status migration

Do not import a stored legacy status blindly.

Recompute derived state from graph + Git + GitHub observation + evidence.

Report meaningful differences.

## Cleanup

After successful migration:
- remove obsolete duplicate registries/files/code paths;
- replace manual docs indexes with generation;
- remove deprecated scripts where canonical Rust capability replaces them;
- retain compatibility shims only with explicit deprecation and tests.

## Safety

No bulk destructive close/delete.

Default legacy adoption should be conservative.

If remote object ownership is unclear, classify `unmanaged/ambiguous`.

## Tests

Build fixtures representing:
- clean legacy mapping;
- duplicate titles;
- stale external ID;
- deleted issue;
- closed issue with incomplete acceptance;
- merged PR;
- orphan branch;
- historical branch no longer active;
- multiple GitHub remotes/fork if relevant.

## Gate

Run migration in dry-run against the real repository if credentials safely allow. Apply only under repository lifecycle/policy. Post-migration validation must report no unexplained in-scope drift.
