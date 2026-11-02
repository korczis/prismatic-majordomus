# Phase 10: Durable Scheduler and Worker Execution Semantics

## Architecture decision
Scheduler owns readiness and assignments; existing mesh owns authenticated peer cooperation and claims; worker adapters invoke existing approved capabilities; persistence owns events; UI projects observability and commands. Do not make mesh discovery a global task queue and do not give the frontend scheduling authority.

## Scheduler inputs
- latest approved plan revision and issue dependency DAG
- canonical issue priority, unblock potential, acceptance criteria, expected resource costs
- peer/capability inventory and health with freshness bounds
- exclusive/advisory claims and lease expiry with fencing epoch
- worktree, branch, touched paths and collision reports
- policy-defined budgets: WIP, cost, attempts, concurrency, deployment reach
- existing delivery queue age and PR backlog; prioritize integrating WIP
- provider capacity, rate limits and circuit-breaker state
- user pause/stop/cancel actions

## Outputs
- deterministic `SchedulingDecision` with selected issue, worker, rationale, rejected candidates
- work lease/claim reference, expiry, fencing token, attempt ID and plan revision
- execution envelope with bounded context (not full repo transcript)
- recovery strategy if worker disappears
- canonical event `assignment_proposed/claimed/started/...`

## Scheduling algorithm contract
1. Reject all tasks with unsatisfied dependencies or missing acceptance criteria.
2. Reject incompatible scopes and exclusive claims; treat advisory claims as warnings.
3. Enforce bounded WIP; if integration backlog exceeds policy cap, favor PR drain over creating new work.
4. Prefer critical-path and unblock potential; weigh age, risk and existing partial results.
5. Match worker capabilities, provider availability, trust, explicit authorization and cost.
6. Choose deterministic tie-breakers so identical snapshots produce identical decisions.
7. Atomically reserve local decision/lease using existing persistence primitives.
8. Claim mesh scope with its existing authenticated mechanism.
9. Re-check lease/authorization before actual effect.
10. Record why candidates were rejected.
11. Expose `dry-run`, `explain`, and `simulate`; do not launch work from list-only commands.

## Worker lifecycle
`offered -> accepted -> leased -> context_loaded -> executing -> checkpointing -> verifying -> handed_over|completed|failed|cancelled`
Use existing state names where they already exist. Worker IDs must be stable across reconnect as supported by actual mesh model; never persist transient p1/p2 display slots as durable identities.

## Agent adapters
- Discover existing Claude/Opus/Fable, Codex, Gemini, local execution integrations.
- No assumption that every provider supports unattended startup, streaming or cancellation.
- Normalize supported: launch, observe, progress, checkpoint, pause, cancel, resume, artifact export, return structured result.
- Explicit capability negotiation and limitations; unavailable adapter is a typed block, not a fabricated session.
- Every adapter inherits `.ai` rules and current scoped context; do not push private local history across machines.
- Agent output is an untrusted proposal, not a proof, review or authorization.

## Backpressure & fairness
- Per-project WIP limit, per-worker concurrency, per-capability rate/cost budget.
- Exponential backoff with jitter bounded by policy, but retain deterministic test hooks.
- Circuit breakers for repeatedly failing providers and deploy targets.
- Prefer cleanup/verification of existing attempts before creating fresh issues.
- Fairness: no starvation of low-priority ready work; add ageing.
- When the GitHub PR queue exceeds threshold, pause spawning tasks that overlap integration hotspots.
- Per-intent budget exhaustion moves to `blocked`, not silent infinite retry.

## Failure matrix
- Worker crash before acceptance: re-offer without effect.
- Worker crash after launch: reconcile checkpoint/external state, then resume or safe retry.
- Mesh disconnect mid-claim: don't assume release; honor expiry and epoch semantics.
- Remote claim conflict: block with link to owner, no forced stealing.
- Duplicate heartbeat: accept idempotently.
- Heartbeat from old epoch: ignore and record stale worker.
- Slow provider: emit alive/progress events; no false done.
- Tool timeout with possible side effect: mark uncertain, reconcile, do not blindly retry.
- Partial context corruption: fail closed with actionable diagnostic.
- Concurrent plan revision: existing executions remain pinned until controlled replan.
- Cancellation during PR merge/release: cannot pretend rollback of irreversible external effect.

## Prove
Simulate 3 workers, 2 conflicting scopes, dependency chain, network partition, forced process kill, stale worker heartbeat and duplicate event. Require zero duplicate side effects and no lost ready tasks under supported test assumptions.
