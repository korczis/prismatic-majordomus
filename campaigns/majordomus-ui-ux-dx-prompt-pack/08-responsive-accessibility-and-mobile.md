# Milestone 8: Responsive, Accessible, Keyboard-First UI

Make the Cockpit genuinely usable across desktop, tablet and mobile while meeting sensible accessibility expectations.

## Responsive architecture

Desktop may use:

```text
sidebar + main + inspector
```

Tablet may collapse navigation.

Mobile should use purpose-built compact patterns, not squeezed desktop layouts.

Tables with many columns must have a deliberate mobile projection such as cards, grouped rows or drill-down lists.

Do not maintain separate domain data sources for mobile.

## Accessibility

Audit and improve:

- keyboard navigation
- focus states
- focus traps in modals/drawers
- labels
- headings hierarchy
- landmarks
- contrast
- status not conveyed by color alone
- reduced motion where relevant
- graph fallback
- screen-reader-friendly controls

## Alpine.js

Use it only for ephemeral interaction state and progressive enhancement.

## E2E coverage

Use existing E2E tooling or add the lightest suitable mechanism consistent with repository conventions.

Cover representative viewport classes and keyboard flows.

## Acceptance criteria

- primary flows work without mouse
- mobile is intentionally designed
- no horizontal-scroll catastrophe on core pages unless semantically unavoidable
- statuses have text/icon semantics beyond color
- dark/light contrast validated
- accessibility regressions are testable
