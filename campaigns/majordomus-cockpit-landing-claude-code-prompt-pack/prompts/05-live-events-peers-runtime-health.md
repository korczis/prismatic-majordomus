# PROMPT 05 — Live activity, peers, runtime and synchronization health

## Mission

Expose current development/runtime state on landing using canonical live data.

## Recent activity

Use canonical event/session/activity infrastructure.

Show concise high-value activity such as:

- workflow stage changes
- tests completed
- reviews requested/completed
- commits/pushes
- issues synchronized
- gates changed
- peer claims/releases
- deployments

Do not build a second activity log specifically for the landing page.

## Active peers

Show active peer/actor state where it exists:

```text
identity
current assignment
session
action/stage
last activity
claim/worktree summary
blocker/review state
```

No credentials/provider secrets.

## Runtime health

Derive local service/runtime state from canonical runtime/environment/service registry.

Potential entries:

```text
Cockpit
API
MCP
Swagger/OpenAPI
GitHub synchronization
peer runtime
governance/validation state
```

Avoid synchronous remote network calls on every page render if architecture provides async/cached health.

Represent unknown/degraded/unavailable separately.

## Realtime

Wire event/peer/health updates through existing Phoenix Channels/LiveView PubSub/event mechanisms.

Do not introduce polling if canonical push mechanisms exist.

## Tests

Test:

- peer joins/leaves
- activity event arrives
- health state changes
- reconnect
- missing subsystem
- event burst behavior

## Acceptance

Landing visibly updates as project/runtime state changes without manual refresh and without frontend-owned truth.
