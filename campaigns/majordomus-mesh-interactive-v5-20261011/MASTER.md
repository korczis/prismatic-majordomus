# MASTER: Make `/ui/mesh` a live, interactive operations surface

You are Claude Opus/Fable operating within the current `korczis/prismatic-majordomus` checkout. The user will supply the complete conversation screenshots, including Mesh concept screens. Visual references specify composition and interaction intent; **they are NOT a source of mesh facts, peer names, counts, dates or operational status**.

## User outcome
The `majordomus ui` command must serve a coherent, navigable, responsive `/ui/mesh` view in which a user can select nodes and edges, inspect actual identity/trust/health/tasks/capabilities, follow per-node streams, and run only authorized canonical actions. Graph, table, details, filters, URL and realtime streams must stay consistent. No mocks, stubs, invented metrics, hardcoded inventories, duplicate backend, parallel business logic or standalone SPA.

## Architectural facts to verify, not assume
Previous repository inspection found a server-rendered Cockpit and canonical capability executor, an execution WebSocket with typed events, and mesh documentation describing discovery, an authenticated link handshake, signed cooperative journal and deterministic fold. `docs/MESH.md` states that mesh discovery does not grant authority; mesh does not itself provide remote execution or TLS. Search live code and all pending PRs, because those findings may have changed. The user's localhost port 8899 cannot be reached from your model's separate environment unless your own runtime can access it. Actually use a browser locally to capture a baseline.

## Procedure (mandatory)
1. Read AGENTS.md, .ai/** relevant rules/doctrines/ADRs, docs/MESH.md, docs/COCKPIT.md, runtime web topology, current /ui/mesh implementation, capability registry, current PRs and CI state. Determine actual paths from repo, not from examples.
2. Read `manifest.json` and the eight milestone prompts in dependency order; reconcile the pack to existing canonical intent/milestones/issues, attach IDs and evidence and do not duplicate previously completed work.
3. Map each planned UI field and action to exact canonical data/capability, identity and permission contract. Unknown or unsupported data must render explicit truthful states.
4. Work in validated vertical slices. Coordinate through existing authorized mesh if supported; avoid concurrent conflicting writes. Continue without routine 'proceed' requests while within authority, but never bypass required gates or credentials.
5. Keep every surface executable and tested. All actions go through the existing authorized executor and return real execution/evidence IDs. Preserve SSR + usable no-JS fallback.
6. Extend the existing live execution infrastructure, not an independent WebSocket backend; snapshot/cursor/replay and reconnect must be tested under fault injection. Live doesn't mean invented milliseconds or pretending another peer's local logs are observable when they aren't.
7. Update .ai schemas/rules/gates, generated docs and site projections from canonical sources; add regression tests with deliberate rule-breaking examples and fail-fast diagnostics.
8. After each accepted slice: run focused tests, schema/derive checks, version analysis, merge safely, release/deploy if authorized, verify real served route and live stream. Record links, SHAs, metrics and blockers. Do not claim deploy if not executed.

## Critical invariant
`mesh node identity → canonical entity → routed detail → permitted action → execution ID → typed live events → evidence` must be a **single explainable trace across UI, HTTP, CLI and MCP**. No front-end copy decides what a node may do.

## Definition of done
A test user opens the live route, discovers a node from an actual connected mesh, selects it with pointer or keyboard, sees current and provenance-bearing state, opens its canonical linked records, subscribes to real events, survives temporary disconnection and browser reload, launches an allowed action using the registry-backed executor, receives live progress and evidence, and observes the same execution ID through CLI/MCP/API. Unauthorized actions are blocked in a negative test. Empty/offline nodes and unsupported remote execution have accurate explicit states. CI and end-to-end browser checks are green; deployed URL and commit are recorded.

## Report
Give facts only: changed modules, canonical data sources, added/modified rules and gate proofs, node identity contract, routes, WebSocket event protocol, verified screenshots, automated tests, release/deploy links, performance findings, remaining blockers. Continue through the work graph until satisfied or genuinely blocked.
