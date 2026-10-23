# Executable acceptance protocol (V2)

## Definition of done: no green checkbox without artifact
Each gate requires reproducible command, environment/reference hardware, raw output location, status and linked failing issue if not satisfied. If no live source available, report `BLOCKED_UNAVAILABLE_SOURCE` and show truthful no-signal rendering; do not mark live integration proven with fixtures.

### G0 canonical audit
Evidence of actual source code locations, available capabilities, route, event semantics, permission model, design tokens. Produce `audit/source-map.md` with claim vs observed code, cited file path/line/commit. Prior discussions are hypotheses, not verified implementation.

### G1 real source → pixels
Choose one genuine event from working Majordomus. Record canonical ID and event provenance. Show ingestion, transformation, network transport and render instrumentation. Inspector links to source where authorized. If impossible, block instead of inventing traffic.

### G2 reliable director
Run virtual-time deterministic unit tests for scheduling; scene fairness, priority interrupt/cooldown, readiness-driven skipping, manual lock and recovery. Confirm each scene initialized/disposed exactly once per lifecycle and no multiplied stream subscriptions.

### G3 stream failure
Automated browser integration: reconnect with persisted cursor, duplicate event, cursor expiration, overflow, server restart, source disabled, malformed envelope, authorization change; correct freshness/error semantics and zero fabricated event pulses.

### G4 privacy and provenance
Server-side property tests for redacted snapshot/delta/inspector/replay/cache/error pathways. Public policy must survive nested and newly added fields, not merely a blacklist of known strings. Prefer explicit allowlist schemas. Verify authorization on drilldown independently.

### G5 24/7 readiness
Run reproducible shorter soak test in CI and optional 24h physical kiosk soak on specified hardware. Report fps p50/p95, frame time, heap and GPU resource drift, CPU, reconnect count, scene churn, leaks and errors. Long-run PASS only after actual run.

### G6 art and hall UX
Independent visual acceptance: legibility from distance, composition, live/replay/partial labels, scene differentiation, clarity under inactivity, smooth scene changes, high-DPI screenshots from running system. These are manual signoffs with evidence, never automated aesthetic claims.

### G7 integration & shipping
Existing quality gates, generated schema/docs/UI drift checks, reproducible build, security audit, branch/release protections. Any deployment claim contains a real URL and a postdeploy smoke check; permission blocked = explicit blocked result.

## Evidence manifest structure
```yaml
schema_version: '1'
repository_commit: '<actual hash>'
source_audit: 'audit/source-map.md'
gates:
  - id: G1
    status: PASS|FAIL|BLOCKED|NOT_RUN
    command: '<actual command>'
    evidence_paths: []
    observed_at: '<actual timestamp>'
    hardware: '<device/browser/version where relevant>'
    limitations: []
```
Do not substitute fictional manifest values or generated pass counts. If files are produced, verify existence. Have a final matrix of all gates with PASS/FAIL/BLOCKED/NOT_RUN and links.
