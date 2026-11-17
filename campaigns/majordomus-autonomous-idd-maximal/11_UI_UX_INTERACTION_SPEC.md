# Phase 11: Majordomus UI Product and Interaction Specification

## Product definition
`majordomus ui` is a high-level, project-oriented intent development control plane, NOT a low-level diagnostic Cockpit clone. Cockpit/diagnostic views remain accessible via drill-down. Adopt actual `docs/UI.md`, `docs/COCKPIT.md`, `docs/DESIGN_SYSTEM.md`, `docs/PERFORMANCE.md` and routing rules before editing.

## Navigation information architecture
Adapt the existing navigation:
- Overview: project pulse, current intents, blocked interventions, recent verified deliveries, velocity.
- Intents: list/kanban, detail, criteria, evidence, plan revisions, launch/pause/replan.
- Milestones: dependency map, execution waves, remaining criteria and blockers.
- Work: issues and active attempts, assignment explanations, scope, tests, worktrees, PRs.
- Mesh: workers, nodes, health, claims, handovers, reviews, capability inventory, streams.
- Delivery: PR integration, release train, versions, deployments, health, smoke, rollback status.
- Evidence: claim provenance graph, validity/freshness, gaps, criteria, auditor view.
- System: policy, .ai rules, services, diagnostics, cost/budgets, performance.
The exact routes must be derived from canonical entity/router metadata. Don't create an independent static nav catalog.

## Global design contract
- Responsive/mobile-first from 320px viewport upward; no broken drawers/overflow, no hidden primary actions.
- Progressive disclosure: overview first, domain details on click; no 250-column tables.
- Every actionable entity displays linked canonical ID, human title, derived status, source, last update and explainable next action.
- Consistent status colors, icons and labels from existing design system; don't invent a new style system.
- Accessible keyboard workflow, focus management, semantic links/actions, labels, ARIA/live regions where appropriate.
- Avoid surprise mutations, destructive shortcuts, and merely decorative toggles.
- Skeletons with explicit stale/disconnected indicators; don't display old data as live.
- Server-side paginated/filtered views with stable cursors; no full journal fetched on every route.
- Separate data freshness from event time; handle out-of-order/replayed updates.

## Overview screen
- Intent progress as verified criteria / total, not optimistic task completion.
- Active executions with phase, elapsed time, last heartbeat and current blocker.
- Mesh availability health and uncertainty, with partial board indicator when not complete.
- PR integration backlog as count and oldest age, not just today's merges.
- Deployment evidence and currently verified target version.
- Contextual quick actions: create intent, inspect blocked, resume safely, open execution.
- Small performance-oriented charts derived from execution events and delivery timestamps.

## Intent creation wizard
Step A: natural-language target outcome, repo scope, timeframe/budget, environment constraints.
Step B: inferred invariants/criteria; mark which are measurable, missing evidence source or ambiguous.
Step C: agent-proposed plan and gaps with explicit references, alternative proposals, adversarial critique.
Step D: policy evaluation and exact authorization scope for side effects.
Step E: execute or save proposal.
Never auto-approve a user intent solely because the wizard reached its last screen.

## Intent detail
Tabs/sections: Summary, Criteria, Plan, Execution, Evidence, History, Decisions.
Criterion row: acceptance predicate, linked work, test/evidence source, freshness, verdict, reasons.
Plan revision diff: added/removed issues, dependencies, risk, resource impacts.
Execution controls: dry-run, start, pause, resume, cancel, replan, explain; controls disabled with real backend reason.
History: semantic event timeline, expandable structured provenance.
Deep links preserve filters and active entity identity across refresh.

## Mesh page
- Interactive graph via EXISTING graph tooling if present (Cytoscape only where fits).
- Nodes clickable with capabilities, last seen, health, checkout identity, active leases.
- Edges for authenticated links, status, observed latency if measured, reconciliation lag if measured.
- Claims view distinguishes advisory vs exclusive and provisional/stale vs confirmed.
- Peer streams: bounded redacted stdout/stderr/events, follow mode, reconnect resume cursor.
- Handovers and reviews: request/accept/reject controls with typed canonical capabilities.
- Graph is never sole access path; list/table accessible alternative.
- No fabricated peer state when one board unavailable.

## Milestone view
- DAG / waves showing READY, BLOCKED, IN PROGRESS, VERIFIED, STALE.
- Click milestone -> issues -> attempts -> PR -> release -> deployment -> evidence.
- Display why an edge exists; detect and reject cycles.
- Forecast is optional estimate with confidence, never invented certainty.
- Show plan revision provenance.

## Execution detail
- Status and immutable execution ID.
- Stream with heartbeat, logs, diagnostics, commands, tool calls (redacted), artifacts and attempt history.
- Show exact worker and scope, active lease and checkpoint.
- Differentiate `requested cancel` from `cancellation confirmed`.
- Display safe recovery action after crash.
- Link directly to each produced artifact, commit, test, PR, release and deployment.

## Delivery view
- PR stages: opened, checks pending, review pending, conflict, ready, merging, merged, blocked.
- Distinguish retryable mechanical conflicts vs semantic integration conflicts.
- Require standard protection rules; never UI button to bypass checks.
- Release card: version policy, source SHA, artifacts/signatures, changelog and provenance.
- Deploy card: environment, expected version, actual observed version, smoke, verification age, rollback eligibility.
- When rollout fails, show actionable, policy-approved rollback or safe halt.

## Design/prototype requirements
- Follow existing Flowbite/Alpine and Rust templating stack where available. Do not force rewrite to React or unrelated framework.
- Render server-side initial shell quickly; hydrate only interactive components.
- Use shared UI primitives, canonical route/entity resolution and shared error format.
- Design 320/390/768/1024/1440 viewport acceptance snapshots.
- Interactions must have feature/integration tests, not just screenshots.
- Cache expensive aggregates, invalidate from canonical event cursor; differentiate private/auth-sensitive caches.
- For graphs, paginate/cluster dense sets; do not render 50k nodes by default.
- ECharts only for measurable time series where actual records exist; no synthetic demo stats presented as current reality.

## Interaction test matrix
- create → edit → critique → authorize → start intent
- navigate intent -> criterion -> issue -> execution -> PR -> deployment -> evidence and back
- pause/resume idempotence; double-click start has single execution
- stale UI version causes concurrency error, not overwrite
- offline stream reconnect with cursor without duplication
- keyboard-only and screen reader basic flow
- mobile drawers, filters, long labels, graph accessible alternative
- forbidden actions invisible or disabled with reason; backend still denies if requested directly
- 401/403/409/429/500, missing peer, stale evidence, disconnected stream, partial board
- UI emits no secrets in HTML, event stream or browser storage
