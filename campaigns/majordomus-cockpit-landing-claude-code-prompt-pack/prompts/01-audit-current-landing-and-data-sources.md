# PROMPT 01 — Audit current landing page and canonical data sources

Perform a deep audit before redesigning anything.

## Mission

Determine exactly why the current Cockpit landing page is visually poor, informationally weak, non-actionable, duplicated, stale, or disconnected from canonical runtime/project data.

Do not start with CSS.

## Inspect

Audit:

- current route/controller/LiveView for `/`
- all landing components
- current data loading
- current hardcoded cards/metrics
- current JS hooks
- current CSS/design system
- issue/milestone data
- sessions and session contexts
- peer/runtime state
- workflows/capabilities
- diagnostics and quality gates
- git/repository state
- CI/deploy state if locally available canonically
- synchronization state
- recent events/activity
- command registry / just integration
- existing API/OpenAPI/MCP representations of the same concepts

## Produce internal audit matrix

For every visible/current landing element and every potentially useful data source, capture:

```text
concept
current source of truth
current landing consumer
canonical owner
current problems
is duplicated?
is deterministic?
is actionable?
is live?
should remain/remove/replace?
```

## Identify current UX failures

Explicitly classify failures such as:

- empty visual hierarchy
- too much whitespace with too little information
- decorative cards with no next action
- counts without operational meaning
- stale/slow data
- disconnected sections
- hidden important state
- no attention prioritization
- no continuation/resume path
- no task/milestone context
- no real-time state
- frontend-specific interpretation
- inaccessible/poor keyboard use
- responsive failures

## Required output

Create or update a concise architecture/UX audit document in repository-standard location if appropriate.

Do not fabricate conclusions. Reference actual files/types/routes/components.

## Acceptance

No code redesign should begin until you can state:

- what currently owns the landing data
- which data are canonical vs duplicated
- which major operational signals already exist
- which signals are missing
- which reusable components/design primitives already exist
- where cross-surface drift currently exists

Then implement only the minimum enabling changes needed for Prompt 02.
