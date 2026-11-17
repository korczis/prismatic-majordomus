# Milestone 05: One state, useful UI/CLI/MCP projections

Review current `majordomus ui`, Cockpit, web server, REST, OpenAPI, MCP, mesh/peers, command registry, UI design tokens and mobile patterns on live master and open PRs. Do not assume old concept discussions are implemented. The high-level UI must be a *derived projection* over canonical evidence and command capabilities. No second JS scheduler, no manually maintained PR list and no backend logic forked into frontend.

## Issue 05.1: Typed read views

Expose from existing canonical registry a concise projection with: desired outcome, pending issue/task, dependency blockers, running peer/worker if known, PR readiness, CI verdict, derive critical path, cache hit/miss, release/deploy proof, and last update. Every visible fact has source/age/provenance. Never label absent data as healthy. Extend existing REST/OpenAPI/MCP introspection types, not separate bespoke endpoints unless truly necessary.

## Issue 05.2: Command/action projection

Derive available actions, authorization prerequisites, validation and conflict constraints from existing capability/command model. Use same action through CLI, MCP and UI (and API if offered). Mutating actions must provide dry-run where meaningful, scope preview, idempotency key, evidence and failure reporting. No ad hoc shell string building in browser; no undocumented bypass of lifecycle.

## Issue 05.3: High-level usable UI

`majordomus ui` should prioritize outcome, progress, obstacles and next safe action, with detail drilldowns to repo, intents, milestones/issues, worktrees, PRs, peers, mesh, CI, derivation costs, release and evidence. Responsive/mobile-first keyboard, touch, drawers, focus order, loading/error/empty states, clear action confirmation for destructive actions. Use the existing design system and actual UI stack discovered. Render a coherent product, not low-level debugging dashboards as landing screen.

## Issue 05.4: Streaming and state freshness

Reuse existing server/mesh event delivery where present; otherwise use simplest existing viable refresh. Handle disconnected server, stale snapshots, backpressure, concurrent agents, reconnection and permissions. Explain state freshness to user. Never show old green as current success. Web UI cannot become its own authority.

## Issue 05.5: Cross-surface contract tests

Shared fixtures assert canonical IDs/order/status across CLI JSON, API/OpenAPI, MCP and UI payload. Playwright/browser tests if project uses it (otherwise current UI test framework): responsive phone widths, drawer operation, keyboard accessibility, real action and error return, no client-side copied catalogs. Test late-added capability automatically propagates to all appropriate surfaces. Document derived architecture; verify deployed UI if authorized and available.

Acceptance: a new capability requires no manual UI/MCP/CLI registration, and user can diagnose a slow/stuck delivery from the same evidence across all surfaces.
