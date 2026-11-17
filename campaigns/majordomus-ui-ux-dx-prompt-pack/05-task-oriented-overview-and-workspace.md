# Milestone 5: Task-Oriented Overview and Work UX

Transform the Cockpit from an entity browser into a task-oriented workspace.

## Product questions the overview must answer

- What is the current goal/intent?
- What is in progress?
- What is blocked?
- Why is it blocked?
- What evidence is missing?
- What changed recently?
- What should happen next?
- Who/what is working on it?
- Is completion currently possible?

## Avoid

Do not make the primary landing page a grid of unrelated counts.

Counts may support context but should not be the main product narrative.

## Derived overview

Create/reuse an `OverviewProjection` from canonical state.

Potential sections, only where supported by actual repository concepts:

- current intent
- progress/satisfaction
- blockers
- next actions
- active sessions
- recent handovers
- evidence gaps
- failing gates
- deployment/runtime status
- mesh activity

## Work area

Organize work around meaningful flows such as:

```text
intent -> milestone -> issue -> criteria -> evidence -> completion
```

or whatever the actual repository model supports.

The UI should expose relationships rather than forcing users to mentally join tables.

## Progressive detail

At first glance show concise operational state.

Allow drill-down into canonical entity detail without losing context.

## Empty states

Empty states must be actionable and derived from capability/project state.

Avoid generic "No data" when the system can explain what is missing or what to do next.

## Tests

Test representative scenarios:

- healthy project
- blocked intent
- no active session
- missing evidence
- failing validation
- newly initialized project
- non-code profile where developer-specific concepts are hidden

## Acceptance criteria

- Overview answers operational questions, not just inventory questions
- canonical derived state drives all cards/statuses
- blockers and next actions are explainable
- task flow relationships are visible without duplicated client joins
