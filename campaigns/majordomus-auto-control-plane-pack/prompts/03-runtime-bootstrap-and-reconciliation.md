# 03 — Runtime Bootstrap, Supervision and Reconciliation

Implement the missing runtime lifecycle that turns repository entry into idempotent convergence.

## Required canonical behavior

Establish/reuse a command/domain operation equivalent to:

```text
runtime.ensure(repository)
```

It must reconcile desired vs actual state.

## Reconciliation algorithm

Implement explicit phases conceptually equivalent to:

```text
resolve repository/worktree identity
load desired service definitions
acquire startup/reconciliation lock
observe processes/sockets/endpoints
classify stale/live/foreign state
compute delta
start/stop/repair only what is necessary
wait/verify readiness within bounded policy
publish discovery metadata + snapshot
release lock
```

Exact implementation must fit current Rust/OTP/process architecture.

## Concurrency

Test simultaneous calls from multiple processes. Exactly one appropriate control-plane instance should own the project/runtime scope intended by architecture.

Avoid TOCTOU process races.

## Stale recovery

Handle safely:

- stale pidfile,
- stale socket,
- port collision,
- process alive but wrong identity,
- process alive but unhealthy,
- previous crash during startup,
- changed binary/protocol/schema version where relevant.

## Entry integration

Refactor `.envrc`, `AGENTS.md`, shell hooks and provider hooks to invoke canonical entry semantics only.

They must remain thin.

Do not put Rust/business logic back into shell.

## Performance

Healthy-path ensure must be very cheap.
No network.
No build.
No package-manager invocation.

If cold startup is delegated/non-blocking, preserve a reliable readiness contract for consumers that need the server.

## CLI/diagnostics

Expose status/ensure/doctor/explain through existing CLI architecture, derived from the same domain services.

## Tests

Mandatory:

- already healthy idempotence,
- concurrent ensure,
- stale state recovery,
- hard crash recovery,
- port conflict behavior,
- no-network assertion where feasible,
- shell entry integration test,
- clean shutdown behavior.
