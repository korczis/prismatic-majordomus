# Phase 02 — Typed MMP contract and capability derivation

Read `../EXECUTION-CONTRACT.md` and `../MASTER-PROMPT.md` first. Reconcile with CURRENT HEAD and skip already-landed equivalent implementation only after verifying it.

## Goal and implementation tasks
Implement or extend a versioned canonical message envelope and endpoint capability descriptors using established typed registry. Define identities (`peer`, `agent`, `session`, `repo`, `worker`) and handshake bindings, namespaces, payload schema/version, deadlines and provenance, message-type taxonomy, policy revision and recipient selection. Introduce exact lifecycle state machine: CREATED→PERSISTED/ACCEPTED→DELIVERED→PROCESSED/FAILED/EXPIRED, with explicit allowed transitions, ACK semantics and diagnostic errors. Ensure structured status is derived across implemented CLI/MCP/API, not maintained in separate hand-written lists. Add schema compatibility migration rules, fuzz/property tests, size limits and stable ordering for enumerable outputs. Document delivery class semantics. Forbid untrusted self-declared identity and arbitrary user-supplied executable payloads.

## Hard acceptance / evidence
Evidence: generated schema and cross-surface contract tests; bad input tests; no duplicate protocol definitions. Gate on backwards compatibility or explicit migration.

## Phase completion report
Provide: changed owner modules; deleted legacy/duplication; new/changed canonical capabilities; schema/version implications; security and failure test results; cross-surface status; exact build/test commands/results; benchmarks if relevant; repo SHA/commit/PR/merge status; unresolved blockers with next executable action. Every claim must be OBSERVED/IMPLEMENTED/VERIFIED. Re-run affected canonical quality gates. Never auto-disable checks.
