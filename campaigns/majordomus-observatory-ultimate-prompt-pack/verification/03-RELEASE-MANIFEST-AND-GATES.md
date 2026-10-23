# Production release gates and evidence manifest

## Mandatory release gates
G0 Repo audit: actual checkout revision, currently implemented routes/capabilities, auth model, asset pipeline, available hardware. State unknowns, not guesses.
G1 Data proof: at least one real source event with source ID, canonical record and correct authorized projection.
G2 Pixels proof: that event reaches browser data collector, scene binding and displayed mark, with timestamp/cursor, trace and reproducible demonstration.
G3 Continuity proof: two art-quality scenes rotate; fairness/interrupt/cooldown assertions; idle and partial inputs truthful.
G4 Recovery proof: disconnect, restart, source gap, cursor expiration, context loss, scene failure, memory caps.
G5 Privacy proof: public data minimization, replay label, no private leakage in snapshot, delta, inspector, error or logs.
G6 UX/perf proof: screenshots 1080p + 4K, accessible controls, 30/60 fps targets tested on specified hardware; no unsubstantiated performance claims.
G7 Deployment proof: build, tests, canonical gates, documentation, version/bump as repo requires, permitted merge/release/deploy, endpoint and verification of deployed code.

## Failure behavior
Never mark a gate PASS without exact command, result, environment and evidence path. `NOT RUN`, `BLOCKED`, `FAIL`, `PASS` are distinct. A checked box cannot stand in for execution.

## Proposed manifest
```json
{
  "revision": "actual SHA, never invented",
  "routes": ["/ui/observatory", "/ui/kiosk"],
  "capture": {"type": "real-recorded-or-live", "source_id": null, "event_id": null, "sanitized": true},
  "evidence": {"lineage_report": null, "browser_trace": null, "screenshot_1080p": null, "screenshot_4k": null},
  "gates": {"G0": "NOT RUN", "G1": "NOT RUN", "G2": "NOT RUN", "G3": "NOT RUN", "G4": "NOT RUN", "G5": "NOT RUN", "G6": "NOT RUN", "G7": "NOT RUN"},
  "deployed_url": null
}
```
Replace placeholders with actual values via existing CI/reporting conventions. This is a *shape example*, not a fabricated report.

## Must-have tests
- source-to-mark provenance property/integration test;
- shuffled source enumeration -> same graph/order/scene result;
- duplicate and late events / clock skew / stream restarts;
- stream resume expired cursor -> consistent resnapshot;
- bounded scene/event memory, resource disposal under repeated transitions;
- display fallback and reduced motion;
- authorization revoked during SSE/WS with no leak;
- replay/capture with real event manifest + redaction;
- no false health for missing source; scene prerequisites accurate;
- route aliases redirect and share one render implementation;
- CI generated docs/schemas drift gate;
- measurable browser/perf integration fixture and longer optional soak.
