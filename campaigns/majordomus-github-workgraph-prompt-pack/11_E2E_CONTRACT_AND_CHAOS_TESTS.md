---
id: github-workgraph-phase-11
phase: 11
depends_on: [github-workgraph-phase-10]
goal: end-to-end, contract, property and chaos testing
---

# Phase 11 — End-to-End Contract, Property, and Failure Testing

Read shared contract and prior handovers.

## Objective

Prove the architecture, not just individual functions.

Build the test matrix that makes future regressions expensive for the machine, not for the human who eventually notices a broken dashboard.

## Canonical lifecycle scenario

Create a deterministic test scenario:

1. create/load outcome M;
2. issues A, B, C where C depends on A+B;
3. A starts in canonical worktree/branch;
4. commits appear;
5. PR maps to A;
6. required checks/evidence pass;
7. PR merges;
8. A becomes accepted;
9. C readiness updates only when all dependencies satisfy policy;
10. GitHub projection converges;
11. reverse lookup from merge commit/PR returns A and M.

Execute as much as practical through public application/capability surfaces, not private helper calls.

## Cross-surface contract

For a shared fixture verify semantic equivalence through:
- direct domain/application;
- CLI JSON;
- REST;
- MCP;
- Cockpit backend payload;
- generated docs model where applicable.

## Property tests

Where appropriate prove:
- dependency topological invariance under insertion order;
- deterministic reconciliation plan;
- idempotent reconcile;
- canonical IDs stable;
- no duplicate external identity mapping;
- adding unrelated work does not mutate existing mapping;
- cycle detection always produces non-ready invalid state;
- completion implies every mandatory criterion has valid evidence.

## Chaos/failure cases

Simulate:
- GitHub unavailable;
- 401/403;
- rate limited;
- partial pagination failure;
- timeout;
- malformed provider response fixture;
- stale plan;
- remote edit between plan/apply;
- object deleted;
- duplicate mapping;
- branch rebased;
- PR squash merged;
- check passed on stale SHA;
- two agents reconcile concurrently;
- cache stale;
- network response empty due to failure.

Critical assertion: remote-fetch failure must never be interpreted as authoritative empty state causing destructive operations.

## Security tests

Assert:
- tokens redacted;
- auth headers absent from diagnostics;
- fixtures contain no live secret;
- UI/API does not expose secret config;
- audit events store safe metadata.

## Performance

Benchmark or at least guard:
- local Work Graph load;
- readiness derivation;
- large graph traversal;
- reconciliation planning over realistic issue counts;
- Cockpit/API payload generation.

Do not micro-optimize prematurely, but detect O(n²) accidents in common paths.

## Real-provider smoke test

If environment explicitly has safe credentials and repository policy allows:
- use read-only observation first;
- target the actual expected repository;
- never mutate production GitHub merely to prove tests;
- mutation test should use a dedicated disposable fixture repository only if such infrastructure already exists.

Default CI remains deterministic/offline.

## Gate

No major invariant should rely solely on mocks at the transport handler level. Domain and provider contract fixtures must meet in at least one integration path.
