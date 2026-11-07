# Phase 08 — Derive every surface and enforce invariants

Read `../EXECUTION-CONTRACT.md` and `../MASTER-PROMPT.md` first. Reconcile with CURRENT HEAD and skip already-landed equivalent implementation only after verifying it.

## Goal and implementation tasks
Trace canonical capability metadata through CLI/MCP/REST/OpenAPI/Swagger/Cockpit/LiveView/docs/landing/GitHub Pages, and use only actual supported surfaces. Expose peer graph, inbox/outbox, delivery transitions, route traces, filterable correlations and health without duplicate frontend inventories or manual generated copies. Avoid editable UI controls not wired to real backend. Subscribe to existing LiveView/event bus rather than build parallel websocket eventing where possible. Add minimal executable guide, examples, migration notes, threat model and operational runbook. Extend `.ai` canonical rules/doctrine/ADR/KB and programmatic validators: no governance bypass, no duplicate bus, no independent peer lists, no absent tests, no generated drift, no insecure default, no unbounded queues. Verify fresh peer auto-appears in relevant surfaces without per-consumer registration. Run usecase impact/coverage and generated-artifact drift gates.

## Hard acceptance / evidence
Evidence: cross-surface identity consistency suite, generator diff clean, real Cockpit interactions/screenshots if available, docs deploy verification where authorized, governance gate demonstrates deliberate violation detection.

## Phase completion report
Provide: changed owner modules; deleted legacy/duplication; new/changed canonical capabilities; schema/version implications; security and failure test results; cross-surface status; exact build/test commands/results; benchmarks if relevant; repo SHA/commit/PR/merge status; unresolved blockers with next executable action. Every claim must be OBSERVED/IMPLEMENTED/VERIFIED. Re-run affected canonical quality gates. Never auto-disable checks.
