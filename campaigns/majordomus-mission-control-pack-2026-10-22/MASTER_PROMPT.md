# Prismatic Majordomus: one-click Mission Control

Reference: https://github.com/korczis/prismatic-majordomus
Evidence baseline at audit: master 355a0cee79aab7030af2e9e4e39a617ffd47c5dd (2026-10-22T19:30:26Z). This is NOT necessarily current by execution time. Re-fetch origin/master, releases, CI, issues, PRs and public deployment on every run. No hard-coded counts/versions/SHAs.

## Non-negotiable engineering doctrine
- **Reuse and extend, never parallelize**: canonical .ai/repo records, existing plan/intent/execution/mesh/lease/journal/capability/http/mcp/ui/cockpit/CI/release/deploy infrastructure. Before adding a type/table/endpoint, find its existing owner, describe why reuse fails, and prefer extending it.
- Canonical typed models; Rust runtime and Rhai for policy extension if already supported; Python only when justified. Avoid new shell scripts; reduce existing shell when touching it if feasible and safe, without a distracting rewrite. No duplicate generated snapshot/store/status models.
- Generated, derived, inferred, dynamic and documented mean each output has declared inputs, provenance, deterministic generation, reproducible checks and drift enforcement. Inference never upgrades unknown to fact. Typed configuration + strict schemas, upgrades/migrations, compatibility.
- Implement the complete vertical slice. Do not stop at ADR, plan, PR, mocked demo or compilable stub. Test every claimed behavior with evidence against exact code and deploy revision.
- Protect secrecy and mutation. Remote public endpoint is not a local trust boundary. Mutations demand authenticated, authorized identities, permission scopes, CSRF/origin protection in web contexts, replay protection and auditable decisions. TLS with validated certs; no public untrusted mesh introspection. Safe fail-closed behavior for unauthorized mutations; local operation remains usable without public hub.
- No unbounded network or resource waits. Explicit end-to-end deadlines, bounded queues, cancellation, retries based on idempotency, leases/fencing, recovery, metrics and backpressure. Reconnection must replay events without silent holes and without duplicate execution.
- A CI success claim requires exact commit and mandatory gates; deployment claim requires served build SHA measured from endpoint and browser acceptance. A skipped/unconfigured deployment is *not* deployed. Do not bypass failures, loosen budgets, force-push master, disable tests or forge evidence.
- Owner-approved privileged operations require existing standing approval policy with narrow scope, expiration and audit; request human action only when credentials, user consent or account grants are truly required. NEVER print secrets.
- Public GH Pages is a generated, public-safe snapshot plus verified deep links; it must never silently impersonate a private interactive local UI.
- Docs in English and Czech using existing localization pipeline. Release notes and homepage demonstrate actual delivered capabilities only; include screenshots and real playbook outcomes, not invented live mesh data.
- Work incrementally in smallest green train compatible with branch protection; preserve full exact-tree validation and current production SHA. Never claim done until independent gates pass. Issue closure and satisfaction are separate from PR merge.

## Required working result
`majordomus serve` boots the local instance, restores its canonical runtime state, advertises its authenticated local UI, and attempts optional discovery/rendezvous (trusted peers, configured Gigalixir hub with TLS). Open UI, choose intent/milestone/issue, see derived readiness/dependencies/evidence, click **Implement**, see authorization, execution request and assigned worker, follow event stream, survive browser/server restart, verify outcome, optionally Deliver by policy through PR/CI/release/provider with served-SHA proof. Same flow works through CLI/MCP/HTTP and remains offline-local when hub unavailable. Every button either acts or is visibly unavailable with a grounded reason.

## Execution protocol
1. Discover real current source and dependencies. Write a concise authoritative reuse map and blocker matrix. If a proposed function already exists, reuse it. Verify current master before coding.
2. Establish one executable E2E vertical goal and tests first, including negatives, restart, auth, duplicate clicks, offline, remote, discovery and provider unavailable states.
3. Implement via existing capabilities and generated surfaces, stage work in isolated branches/worktrees; communicate/claim work via existing mesh when available; never race conflicting scope.
4. Run targeted tests after each slice and complete required integration CI, browser, deployment and installed-release smoke at the end. Correct root causes; do not mark incomplete work complete.
5. Land only green exact-head changes through existing approved integration policy, then release/deploy/verify. Record evidence per SHA, URLs, commands, timings, and actual browser results.
6. End with final factual report: shipped/verified/blocked, exact SHAs, public and local URLs, tests including failures/skips, provider status, security boundary, screenshots, outstanding work and next actions. If access prevents remote deploy, clearly label BLOCKED and leave a ready-to-run, documented workflow rather than making up success.

## Priority sequence: shortest reliable route to daily usability
P0 LOCAL: `serve -> /ui/idd -> choose issue -> Implement -> canonical request -> local worker -> live events -> verify`, with a full browser test and restart recovery.
P1 MESH: configured rendezvous + Gigalixir host, signed identities, trusted discovery, capability-aware assignment and remote worker, fencing/lease and WebSocket replay, remote deny tests, local degraded mode.
P2 DELIVERY: Deliver action invoking existing PR/CI/merge/release/provider with explicit stage status and separately proven outcomes, deploy SHA observed by HTTP and browser, homepage/docs/GH Pages derived.

Do not postpone P0 for P1 infrastructure. P1 cannot be labeled done until there is an actual tested running Gigalixir rendezvous reachable by an authenticated node. No unconsented public code execution via the hub.

## Definition of DONE
- User launches `majordomus serve` once, with a documented local UI URL and automatic best-effort mesh connection. All mandatory features have meaningful status.
- User selects a real issue from canonical plan, clicks Implement, receives one stable execution id; UI displays real events and can reopen after browser refresh/server restart and node failover. A double click never starts two mutating executions.
- A worker with explicit claim performs a testable change in isolated worktree. Missing worker, overlapping claim, invalid intent, lost lease and untrusted remote request are refused or queued with explicit reason.
- Verification reports independently reproducible evidence; no implicit task completion or intent satisfaction. Deliver executes only allowed stages, shows CI/release/deployment and observes served SHA.
- Production: local browser E2E, real mesh-node E2E, provider smoke, GH Pages snapshots and links; 320px and desktop accessibility, keyboard, reconnection; public snapshot honest. Exact SHA and version parity asserted.
- Complete PR/release/train records integrated on master, green mandatory CI at exact head and a browser-tested provider where configured. All remaining blockers have canonical issues with evidence and are **not** counted as shipped.
