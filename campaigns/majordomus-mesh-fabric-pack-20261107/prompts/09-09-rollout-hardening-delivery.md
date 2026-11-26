# Phase 09 — Production hardening, integration and proof

Read `../EXECUTION-CONTRACT.md` and `../MASTER-PROMPT.md` first. Reconcile with CURRENT HEAD and skip already-landed equivalent implementation only after verifying it.

## Goal and implementation tasks
Perform full adversarial sweep: concurrency model, deadlocks, process crash, slow receiver, partial network, flood/DoS, TTL, cancelled requests, lease expiry, multi-worktree/repo isolation, version skew, disk-full, artifact corruption, stale policy, disabled telemetry, secret redaction and CI runner constraints. Run controlled soak and reproducible benchmarks; inspect races, memory growth, lock contention, CPU, reconnect. Remove old duplicate code only after compatibility tests pass. Release behind documented feature gates with rollback path, verify behavior from fresh checkout and no network-on-direnv path; confirm build, canonical gates, docs/Pages if applicable. Integrate as few coherent PRs as justified, track version according to policy and verify merge SHA on master; if deployment cannot be performed state exact blocker. Produce machine- and human-readable release evidence, regression baseline and operator troubleshooting.

## Hard acceptance / evidence
Evidence: clean checkout smoke test, test matrix, threat-model decisions, measured perf budgets, exact release/merge refs, master SHA and demonstrated rollback. No claims of deployed unless verified.

## Phase completion report
Provide: changed owner modules; deleted legacy/duplication; new/changed canonical capabilities; schema/version implications; security and failure test results; cross-surface status; exact build/test commands/results; benchmarks if relevant; repo SHA/commit/PR/merge status; unresolved blockers with next executable action. Every claim must be OBSERVED/IMPLEMENTED/VERIFIED. Re-run affected canonical quality gates. Never auto-disable checks.
