# PHASE E: MESH/FLEET: REAL COOPERATION, AUTO DISCOVERY AND RECOVERY

Role: distributed systems engineer and security reviewer.


## NON-NEGOTIABLE EXECUTION CONTRACT
- Work against the ACTUAL current repository HEAD. Read AGENTS.md, `.ai/**`, governance/doctrines, schemas, ADRs, plan/issue model, commands/capability registry, CI workflows, docs/CLAIMS.yaml, documentation generation, site/cockpit rules, and tests BEFORE proposing changes. Discover paths; do not invent them. The September 2026 bundled archive is historic context, NOT current truth.
- The source of truth is the existing canonical typed repository model and Git. Inferred/derived means no shadow YAML registry, hardcoded frontend inventories, manually synchronized docs or generated facts. Respect existing local shell/Rust boundary; do not reflexively rewrite.
- Establish the state of feature by repo evidence: existing, partial, missing, blocked, obsolete. Explicitly separate implemented, merged, released, deployed, and verified-in-production. No claims of completion by intention or PR description alone.
- Prefer the smallest safe change that corrects a systemic bottleneck. Reuse existing commands, gates, logging, event formats, diagnostics, ownership and effects model. New abstractions need at least two proven consumers or a compelling correctness reason.
- Avoid unsafe auto-merge, branch deletion, release, remote fleet actions, weakened security, ignored exit codes, bypassed branch protection, or suppressed CI. Use established permissions, policy and authorization; never leak credentials. Automated changes only when authorized by existing policy. If blocked, record blocker and continue independent work.
- Repository work must be tracked by intent -> criteria -> milestone -> issue/task -> evidence where supported. Create/fix missing bindings through existing machinery rather than fictional names. Granular PRs, preferably single-purpose; separate generated artifacts where feasible.
- Enforce via actual tests/CI, not merely via a Markdown doctrine. For each new invariant create negative/mutation proof that would fail before the patch; maintain portable adopter behavior. Determinism across machines, worktrees, process schedules and retries is mandatory.
- Do not claim an unknown measurement. Report reproducible commands, revision IDs, expected vs observed results, limitations and exact remaining work. Fail closed on uncertainty for completion, releases and remote writes, without breaking read-only diagnostics.
- Progress autonomously through justified tasks, but stop any destructive/high-impact action outside existing authorization. If a real external approval is required, state the exact blocker and proceed on safe independent tasks.
- Keep PUBLIC docs and UI claims honest and evidence-backed. Changes to contracts require schema/version compatibility review and migrations. Build, test, regenerate, document, review, integrate, release and verify deployment only where permissions and real evidence permit.

## Starting evidence
Open planning PR #913 stated six gaps: Tailscale discovery, trusted reachable server, init/enrolment, linked worktree server cooperation, bounded mesh state/peers response, review-request lifecycle; session auto-join and assigned execution were planned elsewhere. Fleet PR #900 claims live rollout 4/5 on v0.17.0 but is not proof of current convergence. Recheck all statuses.

## Deliver safely
1. Define trust boundary: unlisted clone does not open mesh sockets; only configured trusted peers exchange authenticated messages, with replay protection, correct runtime identity and secret redaction.
2. Discovery provider composition: only supported/consented mechanisms; LAN UDP, Tailscale and rendezvous should converge on one deduplicated peer identity with deterministic precedence, retry/backoff, TTL and health reasons. Never scan unapproved public networks.
3. Startup lifecycle: session, CLI server and MCP server auto-join only where canonical policy authorizes it; no magic manual `proceed using mesh`. Document opt-out and offline path.
4. Worktree repository server association, multi-machine peers, task claims, isolation, capability-based assignment, idempotency, conflict prevention, leases, durable journal and crash recovery. No two peers should be granted incompatible file scopes without warning/refusal.
5. Typed health: process alive != server ready != mesh healthy != task execution ready. Surface degraded/unknown distinctly.
6. Fleet rollout: opt-in authorized remote mutation, signed/verified release inputs, dirty/diverged checkout refusal, service manager idempotence, version checks, partial convergence reporting, rollback/remediation; never make production SSH changes merely because a prompt asks for automation.
7. Live or hermetic two-peer scenario: cold boot, discovery, task assignment, simultaneous edit collision, peer dropout, restart, journal replay, fence old lease, reconnect, complete task and inspect Cockpit/API/MCP. Mark what used stubs vs real machines.

## Negatives
Wrong runtime hello, unauthenticated external HTTP write, forged peer, expired invitation, duplicate event, torn journal, unreachable machine, split brain, journal corruption, repeated rollout, secret in logs. Each must have executable refusal/recovery proof.
