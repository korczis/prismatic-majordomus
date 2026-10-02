# PROMPT 07 — LiveView interactivity and realtime behavior

## Mission

Make landing genuinely interactive without turning frontend code into a second application architecture.

## Use LiveView idiomatically

Prefer server-driven canonical state and LiveView components.

Use JS hooks only for behavior that genuinely belongs client-side.

## Interactive requirements

Support, where canonical backend functionality exists:

- expand/collapse detail
- inline gate inspection
- live progress updates
- session resume
- workflow/action invocation
- filtered activity
- task/milestone quick navigation
- command palette
- resilient reconnect

## Loading/degraded states

Do not block the whole landing page on slow optional data.

Render critical canonical data first and stream/enrich optional portions if architecture supports it.

## Failure handling

Action failures must surface canonical diagnostics and preserve context.

No swallowed errors and no generic "something went wrong" when structured failure exists.

## Event consistency

Live updates must resolve back into the same projection semantics used by initial page load.

Do not maintain divergent incremental frontend state that eventually disagrees with backend state.

## Tests

Use existing LiveView test conventions to test:

- initial render
- updates
- reconnect
- action success/failure
- concurrent update
- stale action
- rapid event stream

## Acceptance

Landing behaves as a live development console, not a static page with decorative refresh icons.
