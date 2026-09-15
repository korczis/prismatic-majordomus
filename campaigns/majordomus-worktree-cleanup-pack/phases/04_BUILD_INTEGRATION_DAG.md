# Phase 04: Build the Feature Integration DAG and Decide What Should Actually Merge

## Goal

Turn a pile of branches into a deliberate integration plan.

No branch should be merged merely because it is old or dirty.

## 1. Refresh baseline

Fetch remote state and verify canonical integration base.

Record exact OID of `origin/master` or repository equivalent used for planning.

If local integration branch contains unique unpushed commits, classify them before proceeding.

## 2. Compute structural relationships

For each candidate branch, determine:

- merge-base with integration branch,
- merge-base with overlapping candidate branches,
- unique commits,
- touched paths,
- rename/copy patterns,
- patch-id equivalence where useful,
- test/docs/schema surfaces affected,
- generated files affected,
- whether it depends on files/types introduced elsewhere.

## 3. Determine semantic relationships

Use commit history, code, docs, tests, prompts, issues, ADRs, handovers, and session context to classify dependencies.

Build a DAG, not just alphabetical order.

For each node, record:

```text
branch
purpose
foundation_dependencies
semantic_dependencies
overlap/conflict candidates
supersedes
superseded_by
must_integrate
may_drop_if_equivalent
risk
validation_scope
```

## 4. Detect supersession

A branch may be unnecessary if:

- all patch content already exists on another branch/master,
- a later branch rewrote the same feature more completely,
- it contains only generated outputs already reproduced canonically,
- its intended behavior is now implemented elsewhere with equivalent tests.

Prove this through diffs/tests, not commit count.

## 5. Detect decomposition opportunities

If a branch mixes independent concerns, consider reconstructing clean integration units from its commits/diffs rather than merging the branch wholesale.

Examples:

- shared refactor + unrelated UI polish,
- schema migration + generated docs noise,
- feature code + stale vendor/build artifacts.

Preserve original branch until reconstruction is validated.

## 6. Choose integration strategy per branch

For each branch choose one:

- `rebase-then-fast-forward`
- `rebase-then-merge`
- `merge-with-history`
- `reconstruct-clean-commits`
- `cherry-pick-subset`
- `drop-as-superseded`
- `retain-not-ready`
- `quarantine-needs-human-domain-decision`

Prefer the repository's normal policy.

## 7. Define deterministic order

The integration order should minimize conflicts and keep tests meaningful.

Typically favor:

1. infrastructure shared by multiple branches,
2. canonical schema/domain model changes,
3. migrations/refactors,
4. feature implementations,
5. API/MCP/UI adapters,
6. docs/generated artifacts,
7. cleanup/enforcement.

But derive actual order from the graph.

## Acceptance criteria

Do not proceed until:

- every candidate branch has an explicit disposition,
- dependency cycles have been investigated and broken intentionally,
- superseded branches have proof,
- integration order is deterministic,
- validation scope is defined per integration unit,
- branches that should not merge are clearly separated from those that should.
