# Phase 4 — Freshness, Incremental Impact, Conflict Detection, and Reconciliation

Use the master contract and the existing RKS domain/bootstrap implementation.

Implement the system that prevents knowledge drift.

## Core goal

A repository change must deterministically identify which knowledge may have become invalid, without regenerating the entire knowledge base.

## Dependency / invalidation model

Build or extend a graph that records dependencies from evidence to claims/nodes and from lower-level nodes to higher-order knowledge.

Support relationships equivalent to:

```text
Evidence → Claim
Evidence → KnowledgeNode
KnowledgeNode → KnowledgeNode
```

The graph must support reverse lookup:

```text
changed evidence
  ↓
affected claims/nodes
  ↓
transitive dependents
```

Avoid a naive “any source changed => regenerate everything” implementation.

## Fingerprints

Use semantically appropriate fingerprints:

- file hash when only file granularity exists
- structured entry hash when parsing a manifest/schema
- symbol/subtree hash where existing tooling makes it practical

Do not pretend to have symbol-level precision if the parser cannot guarantee it. Represent granularity explicitly if helpful.

## Freshness transitions

Implement deterministic transition rules, documented and tested.

Examples:

- unchanged evidence + successful validation => `current`
- weakly related evidence changed => `possibly_stale`
- directly supporting evidence changed materially => `stale`
- contradictory evidence => `conflicted`
- claim has insufficient evidence => `unverified`

Make the state machine inspectable and testable.

## Impact computation

Implement an impact operation that can compare:

- current working tree vs HEAD
- branch/worktree vs base branch where existing git tooling supports this
- explicit revision A..B

Return structured impact results including:

- changed evidence
- definitely affected knowledge
- possibly affected knowledge
- unaffected summary
- reasons/edges explaining the result

The result must be serializable and suitable for CLI/API/MCP/UI.

## Worktree isolation

Test that two feature worktrees can produce independent impact/reconciliation states without corrupting a shared canonical cache or baseline.

Use the repo’s enforced sibling `-wt` worktree model and existing abstractions.

## Conflict detection

Implement deterministic conflict detection where structured evidence contradicts declared knowledge.

Example fixture:

```text
README: Redis is only a cache
code/config: Redis-backed queue consumer exists
```

The system should produce a conflict record containing:

- claim
- supporting evidence
- contradicting evidence
- severity/confidence basis
- current resolution state

Do not silently choose one side.

Semantic/LLM conflict detection belongs later; keep this phase deterministic where possible.

## Reconciliation

Implement reconciliation semantics respecting ownership:

### Majordomus-owned

May be regenerated/updated automatically if deterministic and safe.

### External

Must not be rewritten automatically. Mark stale/conflicted and optionally produce a proposed patch or structured recommendation, depending on existing patch/proposal abstractions.

### Hybrid

Update only machine-owned regions/metadata if the repository already has a safe convention; otherwise propose rather than overwrite.

## Baseline comparison / policy

Implement logic that compares current knowledge health against the brownfield baseline.

At minimum support policy semantics for:

```text
observe
warn
protect
strict
```

Key invariant for `protect`:

> new debt must not increase relative to baseline unless explicitly accepted.

Define what counts as debt using typed categories, not a single opaque score.

## Explainability

Every impact/conflict/freshness result must be able to report:

- why it is affected
- which evidence changed
- which dependency path propagated the impact
- why the final state was selected

## Incremental performance

Use git diff/fingerprints to avoid full rescans when possible.

Add representative performance tests/benchmarks for:

- no-op status
- small local diff impact
- medium fixture reconciliation

Do not over-optimize prematurely, but establish regression budgets.

## Tests

Required:

- direct invalidation
- transitive invalidation
- unrelated changes do not invalidate
- conflict fixture
- baseline protect semantics
- worktree isolation
- external ownership not overwritten
- idempotent reconciliation after successful update
- stable structured impact serialization

## Documentation

Document:

- freshness state machine
- impact semantics
- conflict behavior
- reconciliation ownership rules
- baseline enforcement

## Acceptance criteria

- Drift cannot remain silently current after material evidence changes.
- Impact is incremental and explainable.
- External docs are not auto-overwritten.
- Conflicts are explicit.
- Brownfield baseline enforcement works.
- Worktree states are isolated.
- Structured results are ready for all interfaces.
