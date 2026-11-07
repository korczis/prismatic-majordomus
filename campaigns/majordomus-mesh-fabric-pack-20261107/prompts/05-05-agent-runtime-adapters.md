# Phase 05 — Claude Code, Codex and provider-neutral integration

Read `../EXECUTION-CONTRACT.md` and `../MASTER-PROMPT.md` first. Reconcile with CURRENT HEAD and skip already-landed equivalent implementation only after verifying it.

## Goal and implementation tasks
Enumerate real supported hooks/MCP/app APIs for available agent runtimes from installed versions/docs; do not monkeypatch vendor internals. Implement a canonical `AgentEndpoint` capability model (push-supported, pull-only, supports-stream, can-delegate, needs-user-approval, offline) and at least one real runtime adapter E2E. Bind runtime sessions to authenticated peer identity with reconnection/lease semantics. Provide `ask`, `review`, `notify`, `delegate` where supported, with consent/approval gates; no silent tool execution from unauthenticated messages. Connect intent/task/session/ADR/KB references using context compilers/content-addressed refs, not huge replicated prompts. If a runtime cannot be prodded asynchronously, implement an honest on-demand tool or orchestrated runner and mark limitation; do not fake peer chat. Include governance read/resolve before execution, trace and human escalation only for genuine approval.

## Hard acceptance / evidence
Evidence: live provider-backed message/result recorded with runtime versions, correlation, provenance and actual tool invocation; negative receive-capability test; zero hardcoded provider routing.

## Phase completion report
Provide: changed owner modules; deleted legacy/duplication; new/changed canonical capabilities; schema/version implications; security and failure test results; cross-surface status; exact build/test commands/results; benchmarks if relevant; repo SHA/commit/PR/merge status; unresolved blockers with next executable action. Every claim must be OBSERVED/IMPLEMENTED/VERIFIED. Re-run affected canonical quality gates. Never auto-disable checks.
