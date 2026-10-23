# Incremental release wave runbook

## Wave 0: Evidence & red-trunk repair
Pin HEAD, P0 classification, reproduce failures, fix narrowly, release green verified changes. No new architecture until basic release proof works.

## Wave 1: Canonical dispatch
Trace existing `capability!` model across CLI/API/MCP/peer, add differential tests, fix single specific duplicate, ship independently.

## Wave 2: Rhai + first external plugin
Read-only canonical host call under bounded Rhai, explicit manifest+grant, package installed outside source checkout, cross-surface projections and tests. Ship per slice.

## Wave 3: Graph intelligence
Focus first evidence and PR/mesh graph with stable URLs, selection, drill-down and measured performance. Preserve accessible complete fallback. Ship thin vertical slices.

## Wave 4: Domain shell retirement
Port highest-value verifiable shell behaviors using Rust/Rhai; preserve wrappers/exit semantics. Explicit exceptions for bootstrap/runners. Ship in batches with regression tests.

## Wave 5: Harden beta proof
Security adversarial, clean install, runtime/mesh error/reconnect, release provenance, docs public claims, telemetry economics. Publish only validated guarantees.

### Per-wave cycle
Discover HEAD → claim scope → reproduce → change source authority → generate → test → benchmark → open/update PR → CI → merge → release verdict → deploy → check remote SHA → update evidence → next wave.

### Timing semantics
10–20 min is an *aspiration* for useful eligible micro-releases. No deployment without green required gates. If suite requires more than 30m, record bottleneck and improve it in parallel, never cheat the gate.
