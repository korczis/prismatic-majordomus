# PROMPT 03 — Attention, priority, and resume engine

## Mission

Make landing useful by deriving what requires attention and what the developer can continue immediately.

## Attention model

Use canonical diagnostics/gates/runtime/task state to derive attention candidates.

Possible classes, only where supported by actual repository semantics:

```text
critical
needs_attention
blocked
review_required
stale
warning
```

Potential sources:

- failing mandatory gate
- broken CI
- merge conflict
- invalid generated artifacts
- failed deployment
- blocked issue
- review request
- stale claim/lease
- stale session/handover
- GitHub synchronization drift
- runtime/MCP/API degradation

Do not encode the same problem twice via different discovery paths.

## Prioritization

Create/reuse canonical priority semantics.

Prefer deriving priority from severity, task relevance, recency, dependency impact and explicit domain priority rather than maintaining a hand-written ranking list in UI code.

Every ordering needs deterministic tie-breakers.

## Resume targets

Derive useful continuation targets from existing sessions/workflows/tasks.

Examples:

- active development session
- interrupted workflow
- task waiting for review response
- recently active issue session

Resume target should include enough canonical metadata to explain:

- what it is
- why it is recommended
- last activity
- current state
- next canonical action

## Explainability

If architecture has explain/provenance support, integrate it.

A developer should be able to inspect why an item appears under Attention or Continue Work.

## UI

Landing should show highest-value attention and resume content first.

Do not swamp the page with every diagnostic.

Provide links/actions to the detailed canonical object.

## Tests

Test priority stability and ordering across permutations.

Test conflicts such as:

- same issue has failing gate and blocked dependency
- stale session tied to closed issue
- multiple resume candidates
- no attention items

## Acceptance

Opening landing immediately exposes the most consequential current work/problem without the user navigating through multiple pages.
