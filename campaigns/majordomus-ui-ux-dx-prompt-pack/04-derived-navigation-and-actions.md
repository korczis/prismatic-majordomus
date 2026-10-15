# Milestone 4: Derived Navigation, Command Palette, and Canonical Actions

Replace manually synchronized UI navigation/action catalogs with derived canonical projections.

## Target

A capability/route/action registry should drive, where relevant:

- desktop navigation
- mobile navigation
- breadcrumbs
- command palette
- contextual actions
- docs links
- optional CLI/MCP action exposure

## Navigation

Do not maintain separate arrays for:

- sidebar
- mobile menu
- breadcrumbs
- command palette

Create/reuse a canonical `NavigationProjection` or equivalent.

Navigation visibility should derive from:

- available capability
- route availability
- project state
- authorization/permissions if applicable
- persona/profile where appropriate

If a project lacks a capability, hide the feature instead of rendering dead empty sections unless the product intentionally teaches/setup-guides that capability.

## Actions

Model actions semantically and canonically.

Examples:

- start session
- run check
- inspect blocker
- explain status
- open intent
- create issue
- view graph

Each action should have stable identity, label, availability/disabled reason, and invocation target.

Do not duplicate action availability logic in templates.

## Command palette

Implement a keyboard-first command palette (`Cmd/Ctrl-K` or existing equivalent) derived from canonical actions/navigation.

It should support:

- fuzzy/filterable discovery
- keyboard navigation
- current-context actions
- global navigation

Use Alpine.js for ephemeral palette UI state only.

## Tests

Prove that:

- a new canonical nav/action entry appears in all intended consumers without manual duplication
- unavailable capabilities do not leak as dead nav
- disabled actions have canonical reasons
- desktop/mobile/palette remain consistent

## Acceptance criteria

- one canonical navigation/action source
- command palette implemented or integrated
- duplicated route/action inventories removed
- zero-registration propagation demonstrated with a representative feature
