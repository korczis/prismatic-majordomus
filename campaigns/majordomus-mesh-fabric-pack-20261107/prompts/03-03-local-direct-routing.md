# Phase 03 — Local point-to-point live request/reply

Read `../EXECUTION-CONTRACT.md` and `../MASTER-PROMPT.md` first. Reconcile with CURRENT HEAD and skip already-landed equivalent implementation only after verifying it.

## Goal and implementation tasks
Build a working same-host direct channel on existing server or UDS with authentication bound to actual client session, routing table from live connection state, correlation IDs, request/reply, cancellation, deadline, bounded queue, reconnect and fair scheduling. Do not confuse MCP tool polling with unsolicited agent receive capability; expose an adapter with a supported delivery mechanism, and accurately mark receive capability unavailable otherwise. Demonstrate two independently running clients/processes exchange live messages, one requests a review and one returns a meaningful response. Retain old `peers.list/announce` as projections of canonical peer identity/state or compat shims with explicit deprecation only after migration. Preserve old tests, add two-process integration and e2e trace of route selection. No UDP broadcast for local message plane.

## Hard acceptance / evidence
Evidence: runnable two-peer demo script and captured real trace, p50/p95 local baseline, tool authorization test, no board polling as messaging. If AI unsolicited ingress unsupported, explicit alternative supported orchestration route with honest capability status.

## Phase completion report
Provide: changed owner modules; deleted legacy/duplication; new/changed canonical capabilities; schema/version implications; security and failure test results; cross-surface status; exact build/test commands/results; benchmarks if relevant; repo SHA/commit/PR/merge status; unresolved blockers with next executable action. Every claim must be OBSERVED/IMPLEMENTED/VERIFIED. Re-run affected canonical quality gates. Never auto-disable checks.
