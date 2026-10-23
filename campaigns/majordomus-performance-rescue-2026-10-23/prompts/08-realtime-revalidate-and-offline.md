# Phase 08: Make live refresh reliable without repeated full reads

**Train:** Majordomus P0 CLI + `/ui` + `/cockpit` Performance Rescue  
**Order:** 08 of 18; prerequisites are current repo governance, scope coordination and prior phase evidence.

## Mission and implementation

Inspect `src/live.rs` generation watching, `src/capability/readings.rs` memory/freshness lifecycle, `/events`, websockets, `share/ui/ui.js`, `share/cockpit/socket.js`, browser refresh intervals and panel TTLs. Trace one canonical change (`git change`, capability completion, mesh peer, PR observation) through invalidation -> async computation -> event -> client DOM. Measure event-to-paint and stale duration with network throttling. Identify unnecessary polling that revalidates every source once per second and causes expensive recomputation or repeated source reads across multiple tabs.

Reuse current event bus and transport. Debounce/coalesce only where correctness unaffected; preserve monotonic generation/order, reconnect backoff, last-event cursor and dedupe semantics. When live channel is unavailable, show `reconnecting/offline/stale/polling/snapshot` truthfully and bound polling. Avoid broadcasting secrets or privileged cross-repo state. Do not invent a second pubsub or browser state management layer.

## Proof / exit criteria

Tests must verify: concurrent 2/10/50 clients, reconnect storms, events arriving out of order, duplicate events, stale answer that later becomes current, no forever spinner, inactive pages stop consuming work, no leaked websocket, no polling avalanche, and human-visible update timing with measured browser clock. Mark cases skipped for unavailable browser or network rather than passed.

### Universal instructions for this phase

- **Follow `MASTER-PROMPT.md` and the actual repo's `AGENTS.md` / `.ai/**` governance.** This file is one executable stage of the same train; do not create an isolated project or new catalog. Refresh HEAD, mesh board, scopes and recent peer work before any mutation.
- **Inspect before patching.** File paths and issue numbers are leads from the 2026-10-23 review, not proof of what is currently merged. Test the actual checkout and name exact SHA.
- **Measure before/after on comparable samples.** Treat cold, warm, browser and hosted separately; do not invent times, regressions or green test verdicts. No destructive changes or bypassed gates.
- **Leave a reviewable change and evidence** in the canonical task/issue/execution graph; wire tests, docs and generated projections once through the existing mechanism. Capture blockers precisely.

## Required phase handoff

At completion record: starting SHA, ending SHA, touched files, active issue/claim, tests run (exact argv and exit status), baseline evidence path, measured cold/warm deltas, failing/blocked cases, next owner/dependency. If evidence is missing, mark the phase NOT_VERIFIED rather than complete.
