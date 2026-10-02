# PROMPT 04 — Actionable issue, milestone, and session surfaces

## Mission

Turn landing cards/rows into useful operational controls.

Do not render static summary tiles unless they support navigation or action.

## Issue summaries

Where canonical issue data permits, show useful derived fields such as:

```text
status
milestone
priority
blocking/blocked-by summary
completion gate state
active session
review state
last activity
next available actions
```

## Milestone summaries

Show operational state such as:

```text
completion
ready work
blocked work
active work
critical blockers
active peers/sessions
```

Avoid meaningless percent-complete calculations unless repository semantics already define them.

## Session summaries

Show:

```text
task
actor(s)
workflow/stage
last activity
worktree if relevant
tests/gates summary
resume action
```

## Actions

Actions must be derived from canonical workflow/capability availability.

Do not hardcode separate action inventories inside cards.

Potential actions:

```text
resume
inspect
investigate
implement
review
run tests
open diff
finish
```

Only render actions actually available for the current entity/state.

## Navigation

Use stable canonical IDs/routes.

No fragile query-string-only identity when domain routes already exist.

## Tests

Cover action availability across task states.

Assert unavailable actions are not merely disabled decorative buttons but have correct semantics according to repository design.

## Acceptance

Every major landing entity either enables a meaningful action or earns its visual space by providing immediately operational information.
