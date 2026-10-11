# Milestone 3: Design System with Tailwind + Flowbite

Establish a coherent, enforceable design system for Majordomus Cockpit/web surfaces.

## Goals

- Tailwind as layout/utility foundation
- Flowbite as preferred component primitive library when appropriate
- semantic design tokens
- consistent light/dark behavior
- accessible interaction states
- reusable components without reimplementing domain logic

## Do not

- scatter arbitrary color classes everywhere
- encode Tailwind classes in backend domain models
- create multiple independent badge/button/dropdown systems
- replace existing working components gratuitously
- import huge client bundles for trivial interactions

## Establish semantic tokens

Cover at least:

- surfaces
- text
- muted text
- borders
- focus
- primary
- success
- warning
- danger
- info
- unknown/neutral

Map semantic domain concepts to presentation tokens in one canonical UI mapping layer.

Example conceptually:

```text
DiagnosticSeverity::Warning
 -> PresentationTone::Warning
 -> Tailwind/Flowbite implementation
```

Not:

```text
DiagnosticSeverity::Warning
 -> "bg-yellow-400 text-black"
```

inside domain code.

## Component policy

Audit and standardize:

- buttons
- badges
- alerts
- dropdowns
- tabs
- modals
- drawers
- tooltips
- forms
- tables
- cards
- pagination
- breadcrumbs
- navigation
- command palette surfaces

Prefer Flowbite where it fits.

Where custom components are necessary, document the reason and reuse them.

## LLM development policy

Add repository guidance for future AI workers:

- prefer existing design-system primitives
- prefer Flowbite primitive before custom equivalent
- never introduce new status-color semantics locally
- never duplicate domain inventories in templates
- preserve accessibility/keyboard behavior

Make this guidance enforceable where practical.

## Tests

Add appropriate tests/snapshots/lint checks for:

- dark/light parity
- semantic status rendering
- component consistency
- missing accessible labels where testable
- visual regressions if project tooling supports them

## Acceptance criteria

- design tokens are centralized
- component primitives are documented and reused
- Flowbite/Tailwind usage is disciplined
- status/color semantics are not duplicated across templates
- dark/light modes use the same semantic model
