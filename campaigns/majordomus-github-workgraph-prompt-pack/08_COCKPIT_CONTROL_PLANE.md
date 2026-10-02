---
id: github-workgraph-phase-08
phase: 8
depends_on: [github-workgraph-phase-07]
goal: interactive Cockpit control plane
---

# Phase 08 — Cockpit Work Graph and GitHub Reconciliation Control Plane

Read shared contract and prior handovers.

## Objective

Make Cockpit a real control plane for the canonical Work Graph and reconciliation engine, not a frontend that reconstructs its own task system.

## Data source

Consume existing API/application payloads.

Forbidden:
- frontend hardcoded issue categories;
- frontend-computed canonical readiness;
- frontend-specific reconciliation diff logic;
- duplicate status state machine;
- manually curated capability list.

Interactive sorting/filtering is allowed, but canonical default ordering/state comes from the backend model.

## Required views

Adapt to the existing Cockpit design system/navigation.

### Outcome/Milestone overview
Show:
- canonical identity;
- outcome;
- derived progress;
- issue counts by derived state;
- blockers;
- GitHub projection health;
- acceptance status.

### Issue/execution contract detail
Show:
- objective/scope;
- milestone/outcome;
- dependencies/dependents;
- derived lifecycle state + WHY;
- branch/worktree;
- sessions/claims if available;
- commits;
- PR;
- checks/reviews;
- acceptance criteria with evidence;
- GitHub mapping;
- drift/conflict.

### Graph
Provide a useful dependency/traceability visualization if existing UI stack supports it without excessive dependency weight.

At minimum support traversable relationships. Do not add a fashionable graph library just to render 11 circles.

### GitHub reconciliation
Show:
- observed snapshot status;
- planned operations;
- authority/provenance;
- conflict reason;
- dry-run preview;
- apply action invoking the canonical backend capability;
- post-apply verification.

### Orphans/drift
Surface:
- orphan branches/worktrees/PRs;
- missing remote projection;
- stale mappings;
- premature closes;
- ambiguous commit attribution;
- failed/absent evidence.

## Actions

Buttons invoke canonical capabilities:
- explain;
- refresh/observe;
- plan reconciliation;
- apply reconciliation;
- open associated work graph detail;
- run relevant verification where supported.

Do not implement hidden alternate mutation endpoints.

## Live behavior

If Cockpit already has WebSocket/SSE/live update infrastructure, use it for:
- reconciliation progress;
- check/evidence updates;
- collaboration/session state.

If not, do not broaden scope into a new realtime framework unless it is clearly required and consistent with existing roadmap.

## UX

Make state legible:
- canonical vs GitHub observed;
- drift severity;
- blockers;
- stale observation timestamp;
- destructive operations;
- no secrets.

Avoid "all green" dashboards that erase why something is green.

## Tests

Add:
- API/UI contract tests;
- rendering for empty/small/large data;
- drift/conflict cases;
- unavailable GitHub;
- permission failure;
- stale snapshot;
- dry-run no mutation;
- apply uses canonical capability;
- no duplicated frontend domain registries.

Use existing browser/E2E stack if present.

## Gate

A user must be able to open a milestone, drill to issue, inspect branch/commit/PR/check evidence, see GitHub drift, preview reconciliation, and invoke the same reconciler used by CLI/MCP.
