# Prompt 03 — Cockpit Information Architecture and IDE Workspace

## Mission

Turn the existing Cockpit into a coherent development workspace while preserving its current projection architecture and progressive enhancement.

Do not replace the current Cockpit with an SPA unless the repository's measured architecture requires it. Existing complete server-rendered HTML, deep links and optional JavaScript are valuable constraints.

## Required top-level experience

A developer opening `/cockpit` should immediately see:

- repository identity;
- active branch/worktree and dirty/ahead/behind state;
- active issue/milestone/task/session if known;
- attached peers and claims;
- governance status;
- health status;
- generated artifact drift;
- CI/release/deploy state when available;
- current/recent executions;
- failures needing action;
- shortcuts to develop/test/check/docs/release/deploy workflows;
- canonical links to API/Swagger/docs/GH Pages;
- current server/version/build identity.

Every datum must link to the capability/source that produced it.

## Workspace sections

Implement or evolve derived navigation for:

1. Home / status
2. Develop
3. Executions
4. Capabilities
5. Workflows
6. Repository
7. Git / worktrees
8. Issues / milestones / planning
9. Peers / sessions / handovers
10. Governance
11. Knowledge / ADRs
12. Graphs
13. Health / diagnostics
14. Observability / performance
15. API / Swagger
16. Docs / site
17. Release / deployment

Do not hardcode this list blindly. Map it onto existing canonical module/kind/domain metadata. If information architecture requires a new presentation taxonomy, define it once as canonical metadata and derive all nav/help/docs projections.

## Layout

Provide a durable IDE-like shell:

- left navigation;
- top context bar;
- central workspace;
- optional right inspector;
- bottom execution/activity drawer if appropriate;
- keyboard-accessible command palette;
- responsive/mobile fallback;
- no-JS functional baseline;
- deep-linkable state;
- browser back/forward correctness.

## Search and palette

Extend the palette into a universal command/resource launcher derived from:
- capabilities;
- workflows;
- objects;
- graphs;
- repository files if canonical file indexing exists;
- issues/milestones if supported;
- executions;
- docs.

Results should carry type, provenance and available actions.

No manually registered palette entries.

## Context persistence

Filter/view/panel state should be:
- URL-addressable when shareable;
- local preference only when genuinely personal;
- never canonical repository truth.

## Accessibility and security

Preserve:
- strict escaping;
- CSP;
- no remote JS dependency;
- keyboard navigation;
- focus management;
- reduced motion;
- semantic HTML;
- usable no-JS pages.

Add tests for all relevant guarantees.

## Acceptance

Prove:
- all navigation is derived;
- no catalogue duplicates canonical registry data;
- pages use capability execution/read paths;
- refresh preserves deep-link state;
- no-JS baseline works;
- keyboard-only critical workflows work;
- representative added capability/workflow/object appears automatically.
