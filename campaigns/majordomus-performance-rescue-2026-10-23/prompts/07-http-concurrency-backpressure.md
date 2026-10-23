# Phase 07: Fix resource contention without rewriting the HTTP stack

**Train:** Majordomus P0 CLI + `/ui` + `/cockpit` Performance Rescue  
**Order:** 07 of 18; prerequisites are current repo governance, scope coordination and prior phase evidence.

## Mission and implementation

Inspect `src/http/server.rs` (4 accept workers, up to 64 per-request handler threads at reviewed SHA), `src/http/deadline.rs`, `src/http/ws.rs`, `src/http/events.rs`, `src/commands/serve.rs`. Measure requests/s, accept delay, active handlers, queued work, thread counts, memory, 503 rate, connection caps and long-lived SSE/WS under static asset bursts, background UI warmer, slow plan requests and probes. Distinguish wire time from router handler time, including backpressure from clients who read slowly. A poll thread sleeping is not evidence of the actual bottleneck.

Make targeted changes if real saturation is found. Respect a bounded resource policy: no uncontrolled threads or per-source worker explosion; timeout slow/no-body clients; prioritize cheap health/read and human UI fairly without starving commands, but health must not claim ready when UI broken. Preserve existing websocket upgrade behavior, HEAD method, content-length, graceful shutdown, origin/security checks, and 503 semantics. No silent retry loops that turn failures into minutes of waiting. If a fixed pool is measurably necessary, integrate with existing server execution model and add deterministic bounds/tests, rather than a wholesale move to an unrelated async framework.

## Proof / exit criteria

Provide load/soak traces and resource usage, prove <= configured thread/memory caps, fast 503 where appropriate, no stuck `stop`, no leaked open channels, and UI p95 under adversarial slow handler pressure. Negative tests on slow readers, disconnected clients, WS/SSE, readiness while flooded.

### Universal instructions for this phase

- **Follow `MASTER-PROMPT.md` and the actual repo's `AGENTS.md` / `.ai/**` governance.** This file is one executable stage of the same train; do not create an isolated project or new catalog. Refresh HEAD, mesh board, scopes and recent peer work before any mutation.
- **Inspect before patching.** File paths and issue numbers are leads from the 2026-10-23 review, not proof of what is currently merged. Test the actual checkout and name exact SHA.
- **Measure before/after on comparable samples.** Treat cold, warm, browser and hosted separately; do not invent times, regressions or green test verdicts. No destructive changes or bypassed gates.
- **Leave a reviewable change and evidence** in the canonical task/issue/execution graph; wire tests, docs and generated projections once through the existing mechanism. Capture blockers precisely.

## Required phase handoff

At completion record: starting SHA, ending SHA, touched files, active issue/claim, tests run (exact argv and exit status), baseline evidence path, measured cold/warm deltas, failing/blocked cases, next owner/dependency. If evidence is missing, mark the phase NOT_VERIFIED rather than complete.
