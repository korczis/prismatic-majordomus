# PROMPT 11 — Tests, contracts, determinism and drift gates

## Mission

Make the new landing page difficult to regress.

## Unit tests

Test canonical projection semantics independently from LiveView.

## Determinism

Randomize source enumeration/input order where possible and prove stable output for:

- attention
- resume targets
- milestones
- tasks
- peers
- health
- events
- actions

## Contract tests

Assert canonical IDs/status/gate semantics agree across relevant:

```text
backend model
CLI JSON
API
MCP
Cockpit payload/rendering model
```

Avoid maintaining six handwritten expected inventories.

## LiveView tests

Cover:

- useful normal landing
- no active work
- critical attention
- degraded runtime
- active peers
- active session
- many milestones/issues
- realtime update
- action invocation
- reconnect
- error states

## Drift tests

Detect:

- duplicated frontend capability lists
- duplicated workflow lists
- duplicated status lists
- OpenAPI drift
- docs/generated page drift
- stale schemas

## Performance

Measure/protect landing render/query behavior using existing performance conventions.

Avoid N+1 loading and expensive synchronous external calls.

## Acceptance

A developer should be able to change or add a canonical entity and have tests fail if Cockpit or another surface requires manual synchronization.
