# Phase 17: Operations, Metrics, SLOs, Cost and Reliability

## Minimum telemetry
All observations derived from execution events or authoritative external receipts:
- active intents/executions/tasks by status, oldest blocked and oldest ready ages
- intent-to-first-plan, plan-to-first-dispatch, ready-to-start lead times
- PR open-to-merge and merged-to-deployed lead time
- end-to-end intent-to-verified-deployment and criterion satisfaction lead time
- attempts per successful task, retry rate, duplicate-prevention reconciliation count
- orphaned/expired claims, worker restart and lost heartbeat count
- scheduler queue depth, WIP, provider waiting and utilization
- stream gap/reconnect count, event latency, drift and stale projections
- evidence invalidation and blocked-by-missing-evidence rates
- release/deploy success, failure, rollback and time-to-recovery
- time/cost per intent, model/provider token estimates if supported

## Trace model
Correlation ID from UI/CLI user action is propagated to intent plan operation, scheduler decision, worker attempt, external effect, CI, PR and deployment.
Produce a machine-readable trace with event IDs and safe links. Never record raw bearer tokens.
Use existing diagnostics/explain infrastructure, avoid second tracing convention if present.

## Alert/health semantics
- `healthy`, `degraded`, `unavailable`, `unknown` and `stale` are distinct.
- An offline peer without fresh telemetry must be unknown, not healthy.
- Reconciliation lag must be measured, not guessed.
- Alert on unsafe repeated external effects, stuck integration queue, stale leases, missing journal cursor and failed release smoke.
- Operators must see owner, cause, attempted remedy, next safe action.

## Error budgets and target setting
Collect baseline distribution before enforcing latency SLO.
Define error budget by user-visible terminal results, not simply HTTP success.
No misleading claim of 99.9% from one demo.
Track sample size, environment and window with each published metric.

## Costs
- budget per intent, per provider and per worker; soft warning and hard cap.
- track expensive planning retries, parallel critique fanout and rework induced by conflict.
- scheduler can downshift/reduce concurrency while maintaining mandatory tests and security gates.
- publish estimate vs actual with provenance and confidence.
