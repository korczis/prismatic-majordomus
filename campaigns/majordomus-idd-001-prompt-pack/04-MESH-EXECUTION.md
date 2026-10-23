# MISSION 04 — MESH-NATIVE EXECUTION, NO SHADOW SCHEDULER

Inspect existing mesh discovery, peer metadata, coordination/broker, execution engine, worktree management and PR-drain. Complete integration via canonical capability and typed event interfaces. A scheduling decision must be explainable, authorized, capability-compatible and stable under concurrency.

Peer eligibility: advertised capabilities, version/protocol, repository/worktree availability, provider constraints, health, concurrency slots, required trust policy, workload and observed budget. Never infer availability from an old heartbeat without TTL. Assignment needs atomic ownership, bounded lease, renewal, fencing token/generation and retry/cancellation semantics. Do not promise exactly-once distributed side effects. On disconnect classify running work as pending reconciliation, not instant success/failure. Track attempts separately from logical issues and from tasks.

Ensure git/worktree isolation for multiple writers; detect conflict, do not overwrite uncommitted changes. Existing merge train and CI remain authoritative. Agent cooperation: discover active sessions and intentions, reconcile unfinished work, avoid claiming same issue twice; record handoff and provenance; support integration, reviewer, tester peer roles when capabilities actually exist.

Tests: double claim, lease expiry race, stale worker completion, duplicate dispatch, restart, peer rejoin, lost cancel, incompatible peer version, worker crashes, worktree write conflicts, missing provider, unavailable mesh. Integration tests with two real local peers where feasible, otherwise explicit mocked protocol tests labelled simulated. Metrics: idle time, queue delay, claim contention, retries, peer utilization, verified completion per unit cost.

Success: user-initiated issue in `/ui` is assigned and tracked across eligible mesh peer(s), with canonical execution events, recovery logic and no duplicate authoritative task states.
