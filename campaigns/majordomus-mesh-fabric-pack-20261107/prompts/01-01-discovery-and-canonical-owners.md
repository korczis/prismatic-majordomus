# Phase 01 — Architecture reconnaissance and ownership

Read `../EXECUTION-CONTRACT.md` and `../MASTER-PROMPT.md` first. Reconcile with CURRENT HEAD and skip already-landed equivalent implementation only after verifying it.

## Goal and implementation tasks
Audit HEAD and active changes. Trace real data flow from MCP initialize to peer identity, board, existing bus/synth/monitor/event stream, task lifecycle, .ai, storage and execution. Inspect generated capability surfaces and quality gates. Identify all overlapping projects/branches/issues, avoid collisions. Produce machine-readable ownership matrix with path, owner, transport, state, consumers, persistence, security, tests, observed gaps, provenance and migration decision. Record 3 actual E2E user stories: same-host Claude↔Codex, multi-session request/reply, offline durable review. Decide whether local IPC should extend the running server's existing architecture rather than introduce an independent daemon. Check stale historical `src/peers.rs` assumptions. Deliver a minimal integration plan + current architecture graph + failing executable acceptance test or test harness demonstrating no direct request/reply, without breaking master. Add ADR only if doctrine calls for it.

## Hard acceptance / evidence
Evidence: current SHA, paths, exact commands, ownership matrix, collision audit and regression baseline. Reject phase if it merely renames a proposed bus without identifying canonical owner.

## Phase completion report
Provide: changed owner modules; deleted legacy/duplication; new/changed canonical capabilities; schema/version implications; security and failure test results; cross-surface status; exact build/test commands/results; benchmarks if relevant; repo SHA/commit/PR/merge status; unresolved blockers with next executable action. Every claim must be OBSERVED/IMPLEMENTED/VERIFIED. Re-run affected canonical quality gates. Never auto-disable checks.
