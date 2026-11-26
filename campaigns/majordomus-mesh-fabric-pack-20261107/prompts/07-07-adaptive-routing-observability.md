# Phase 07 — Policy-driven routing and measurable economics

Read `../EXECUTION-CONTRACT.md` and `../MASTER-PROMPT.md` first. Reconcile with CURRENT HEAD and skip already-landed equivalent implementation only after verifying it.

## Goal and implementation tasks
Build route choice as pure deterministic policy with candidate transports discovered from actual capabilities and locality. Provide explicit reasons for route selection and rejection (security, RTT, queue, reliability, delivery guarantee, cost). Collect OTel metrics/traces reusing current telemetry: RTT, p95/p99, connection churn, queue depth, retries, dead letters, dropped ephemeral, byte sizes, delivery state, message-to-LLM latency, inferred token/cost correlations only when provenance exists. Introduce health hysteresis, circuit breakers and fallback without oscillation. Compare against board polling with reproducible workload. Experiment only behind reversible feature flag and documented budgets; performance-driven adaptive weights must never override security or delivery semantics. Do not create full-blown ML router without observed gain.

## Hard acceptance / evidence
Evidence: baseline/after results, route-explain command/endpoint, failure switching tests, telemetry cardinality/privacy budget and fallback rollback test.

## Phase completion report
Provide: changed owner modules; deleted legacy/duplication; new/changed canonical capabilities; schema/version implications; security and failure test results; cross-surface status; exact build/test commands/results; benchmarks if relevant; repo SHA/commit/PR/merge status; unresolved blockers with next executable action. Every claim must be OBSERVED/IMPLEMENTED/VERIFIED. Re-run affected canonical quality gates. Never auto-disable checks.
