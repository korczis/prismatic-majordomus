# Prompt 06 — Observability and Monitoring Control Plane

## Mission

Turn Cockpit into the place where a developer can see whether Majordomus and the repository are actually healthy.

No decorative dashboards. Every chart/table must correspond to canonical counters/events/health checks.

## Integrate these signals where supported

- server lease/status;
- process identity/version/build fingerprint;
- registry fingerprint;
- repository/index fingerprint;
- execution counts/states;
- handler invocations;
- cache hit/miss/eviction;
- phase timings;
- request counts/latencies if existing instrumentation supports them;
- MCP clients/peers;
- HTTP health;
- generated artifact drift;
- benchmark coverage;
- benchmark regressions;
- CI state;
- GH Pages deployment state;
- release/version state;
- branch/worktree health;
- unresolved diagnostics;
- stale contexts/handovers if canonically detectable.

## Timeline/activity feed

Create a typed event/activity projection if current execution events are insufficient.

Potential events:
- peer connected/disconnected/announced;
- task opened/finished;
- execution started/completed;
- validation failed;
- generation drift detected/fixed;
- branch pushed;
- CI result observed;
- deploy completed;
- server replaced/outdated.

Do not scrape log strings. Events need structured identity and provenance.

## Monitoring views

Provide:
- current health summary;
- actionable failures first;
- execution timeline;
- performance/caching;
- peer activity;
- deployment/CI;
- historical data only if the project already persists it canonically.

Do not invent a pseudo-time-series database for data that only exists in process memory.

## Reproduction

Every warning/error shown should carry:
- canonical diagnostic code;
- subject;
- evidence;
- deciding engine;
- remediation/reproduction command or capability.

## Acceptance

Tests must prove:
- Cockpit renders the same health decisions as CLI/API/MCP;
- no page recomputes expensive canonical state per request;
- repeated monitoring page loads do not rebuild registry/index/OpenAPI;
- live updates reconnect cleanly;
- stale/outdated server states render correctly;
- diagnostics are escaped and link to their canonical source.
