# Milestone 11: UI/DX Documentation, Onboarding, and Extension Guidance

Document the browser UX architecture so future humans and agents extend it correctly.

## Document

- presentation architecture
- projection layer
- design system
- Flowbite policy
- Tailwind semantic tokens
- Alpine.js state boundary
- Cytoscape graph contract
- p5.js usage boundary
- navigation/action derivation
- explainability model
- persona/progressive disclosure model
- testing/enforcement

## Extension guide

A contributor should know how to add a new feature/entity and have it automatically surface in intended UI areas.

Show the canonical path:

```text
add canonical capability/entity
 -> projection derives it
 -> nav/action/detail/graph surfaces update where applicable
 -> tests prove propagation
```

Do NOT teach contributors to edit multiple synchronized lists.

## Onboarding

Improve user-facing onboarding so a new user can understand the product from the browser.

Include:

- what Majordomus does
- current project state
- next action
- where to inspect goals/work/evidence
- how to open advanced system details

## Examples

Use real derived examples from fixtures or repository state where possible.

Avoid manually maintained screenshots/data that immediately rot unless the repository has a robust visual-test generation path.

## ADR

Follow existing ADR policy. If the projection architecture/design-system decision qualifies, record it using repository conventions.

## Acceptance criteria

- future AI workers receive clear architectural rules
- UI extension path is documented
- onboarding is browser-first
- docs contain no competing hand-maintained inventories
