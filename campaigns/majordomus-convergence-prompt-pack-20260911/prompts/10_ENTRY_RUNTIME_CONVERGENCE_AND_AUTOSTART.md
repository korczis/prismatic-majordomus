# Stage 10 — Repository Entry Runtime Convergence and Autostart

Make entering a managed repository reliably establish the intended Majordomus runtime state without turning `.envrc` into an operating system.

## Architecture

`.envrc` is a trigger/adapter only.

Canonical flow:

```text
repo entry
  ↓
majordomus env/entry command
  ↓
typed RepositoryEnvironment + RuntimeConvergence
  ├─ repo identity/config
  ├─ runtime executable/version
  ├─ server lease/endpoint
  ├─ provider adapter/session
  ├─ peer registration
  ├─ context readiness
  └─ diagnostics
  ↓
small env export + optional banner/status
```

## Requirements

1. Reuse the single lease parser/reader and canonical environment subsystem.
2. Detect missing/stale runtime binary/server/version/config.
3. Separate fast entry from repair/build work. Fast path must have a measured budget and must not unexpectedly compile or use network.
4. Provide an explicit convergence/ensure operation that can repair stale runtime through the repository-approved build/install mechanism.
5. Start/attach server idempotently and safely; prevent duplicate/conflicting server instances.
6. Register/refresh current provider/session/peer where adapter semantics support it.
7. Load/prepare context through canonical context subsystem.
8. Export shell environment from typed data. Do not parse banner text.
9. Expose status/explain/doctor via structured CLI/API/MCP/Cockpit.
10. Cache only canonical typed snapshots with correct invalidation/schema versioning.
11. Test cold start, warm start, stale executable, stale lease, dead PID, port collision, concurrent shells, unsupported provider, malformed config and slow component timeout.
12. Benchmark fast path; enforce budget in CI if stable across environments.

## Acceptance

- `cd`/direnv path is small and deterministic.
- No domain discovery logic is duplicated in shell.
- Runtime/session/context state converges or reports actionable diagnostics.
- Entry never silently claims healthy state when repair is needed.
- Lease-reader invariant remains single-source and gate-tested.
