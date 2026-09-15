# Phase 10 — Hardening, End-to-End Testing, Performance, Migration, and Release Readiness

Use the master contract and completed RKS feature set.

Treat this phase as release engineering, not feature brainstorming.

## Goal

Make RKS rock-solid enough to ship as a public Majordomus capability.

## Full-system audit

Review implementation for:

- duplicated registries
- hardcoded taxonomy in multiple layers
- manual docs drift
- unstable IDs
- unversioned schemas
- caches accidentally committed
- worktree state leakage
- network dependence in normal checks
- unsafe remote LLM processing
- non-idempotent bootstrap
- destructive external doc reconciliation
- unexplained confidence values
- API/CLI/MCP inconsistencies
- missing Cockpit states

Fix architectural debt now rather than documenting it away.

## End-to-end matrix

Build a high-value E2E suite across realistic fixtures.

### Scenario A: clean greenfield-ish repo

- bootstrap
- status current
- CLI/API/MCP consistent

### Scenario B: brownfield documented repo

- existing docs registered external
- no duplicates
- baseline created
- protect mode tolerates existing debt

### Scenario C: repository change

- edit relevant file
- impact detects specific nodes
- check reflects new debt
- reconciliation restores state

### Scenario D: conflict

- declared doc contradicts deterministic evidence
- conflict shown in CLI/API/MCP/Cockpit
- no silent resolution

### Scenario E: worktrees

- two worktrees with different changes
- impact/state isolated
- base baseline unchanged

### Scenario F: semantic provider disabled

- deterministic RKS fully operational
- semantic commands degrade gracefully

### Scenario G: remote processing forbidden

- protected evidence never leaves process / mock provider never receives payload

### Scenario H: schema migration

- load an older fixture schema
- migrate to current
- no information loss in supported fields

## Performance budgets

Measure and document realistic performance.

Targets should align with current project scale; use these as starting expectations, not dogma:

```text
knowledge status (warm)      ~ <100 ms
knowledge impact (small diff) <300 ms typical
no-op validation             fast enough for normal CLI/CI use
```

Heavy semantic operations may be slower but must be explicit and cached.

Add benchmarks/regression tests where repo tooling supports them.

## Large repository behavior

Use a synthetic or real larger fixture to ensure:

- graph endpoints can paginate/filter
- Cockpit does not render every node by default
- search and impact remain bounded
- memory usage is reasonable

## Cache correctness

Test:

- cache key includes schema/extractor version
- changed evidence invalidates dependent cache entries
- cache is safe across worktrees/revisions
- stale cache does not produce `current` false positives

## Migration/versioning

Finalize:

- current schema version declaration
- migration path skeleton
- compatibility tests
- diagnostics for unsupported future versions

## Release UX

Ensure:

- `majordomus doctor` or equivalent reports RKS diagnostics
- `majordomus knowledge status` is useful immediately after init
- errors include remediation
- help text/examples are current
- generated docs match released CLI

## CI

Wire the full deterministic test matrix into appropriate workflows without destroying CI duration.

Use caching and test partitioning already present in the repository.

Heavy browser/semantic suites may run in dedicated jobs if current workflow architecture supports it.

## Supply-chain / dependency review

Review new dependencies for necessity, maintenance quality and binary impact. Prefer existing dependencies or lightweight crates.

Do not introduce a heavyweight parser/indexer merely to avoid writing a small deterministic adapter.

## Observability

Use current Majordomus logging/tracing. Add structured spans/metrics for:

- scan/bootstrap
- extractor execution
- impact computation
- reconciliation
- provider semantic operations

Never log sensitive evidence payloads by default.

## Release documentation

Prepare release notes/changelog entry describing:

- brownfield bootstrap
- evidence/provenance
- drift detection
- impact/reconciliation
- CLI/API/MCP/Cockpit
- limitations

## Acceptance criteria

- Core E2E scenarios pass.
- Worktree isolation is proven.
- Schema migration is tested.
- Performance is measured and acceptable.
- Cache semantics are safe.
- CI remains practical.
- Release docs/help are synchronized from canonical sources.
- No critical TODOs or known silent-drift paths remain.
