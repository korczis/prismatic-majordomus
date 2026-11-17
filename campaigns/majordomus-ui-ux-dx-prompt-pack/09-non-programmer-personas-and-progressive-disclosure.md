# Milestone 9: Non-Programmer Personas and Progressive Disclosure

Make Majordomus usable for long-running AI-assisted work that is not primarily software development, without forking the product into a second application.

## Core rule

Same domain truth. Different projections.

Do not duplicate storage/models for "non-code mode" unless domain semantics genuinely differ.

## Persona/profile concept

Reuse an existing profile/execution-profile/capability model if available.

Otherwise introduce the smallest canonical concept needed for presentation preferences.

Potential profiles:

- simple
- developer
- researcher
- operator

These are examples, not mandatory names.

## Non-programmer primary language

Prefer concepts like:

- project
- goal
- work
- decisions
- sources
- evidence
- progress
- blockers
- agents/people
- outcomes

Developer substrate such as:

- branch
- commit SHA
- MCP internals
- schemas
- provider diagnostics
- runtime endpoints

should be available through advanced/system views when relevant, not dominate the default UX.

## Important constraint

Do not merely rename Git concepts while keeping software-specific assumptions in the core domain model.

Audit where repository/code semantics leak into generic work concepts and document genuine architectural boundaries.

Fix only what is necessary and appropriately scoped; do not explode this milestone into a total generic-work rewrite unless the repository clearly supports that direction.

## Onboarding

A new user should understand:

- what Majordomus is tracking
- what the current goal is
- what to do next
- where durable project memory lives
- how AI sessions hand over work

without reading CLI documentation first.

## Acceptance criteria

- a non-programmer profile/view is viable
- advanced developer detail remains accessible
- no duplicated backend truth is introduced
- software-specific leakage is identified and reduced where practical
