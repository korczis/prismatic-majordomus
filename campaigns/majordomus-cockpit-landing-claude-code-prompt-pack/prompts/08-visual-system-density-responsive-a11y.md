# PROMPT 08 — Visual hierarchy, information density, responsiveness, accessibility

## Mission

Make the landing page visually strong, dense, readable and aligned with the existing project design system / GitHub Pages visual language where repository doctrine requires it.

Do not invent a separate Cockpit brand.

## Desired character

The UI should feel like a development operations console:

- high information density
- clear hierarchy
- strong typography
- compact but breathable spacing
- monospace where technical data benefits
- consistent status language
- minimal ornamental chrome
- excellent dark/light mode if supported
- useful hover/focus states
- subtle live-state transitions only

## Layout hierarchy

Prioritize roughly:

1. critical attention
2. resume current work
3. active development/runtime
4. milestones/ready work
5. activity/peers
6. secondary inventories/telemetry

Adapt to actual data and design system.

## Avoid

- huge empty hero areas
- marketing copy
- enormous stat cards
- meaningless metric tiles
- duplicated borders everywhere
- decorative gradients without semantic purpose
- low-contrast status chips
- horizontally overflowing desktop-only layouts

## Responsive behavior

Design explicit behavior for:

- wide desktop
- laptop
- narrow desktop/tablet
- mobile

Reorder information by priority rather than merely stacking arbitrary columns.

## Accessibility

Verify:

- semantic headings/regions
- keyboard navigation
- focus visibility
- status not conveyed only by color
- sufficient contrast
- live region behavior where appropriate
- reduced motion if project supports it

## Shared design system

Reuse canonical components/tokens.

If landing reveals gaps in the shared component system, improve the shared system rather than adding page-specific styling fragments.

## Visual regression

Use existing screenshot/component visual testing if available.

Do not introduce brittle pixel snapshots without repository precedent.

## Acceptance

Landing should look intentional and operationally dense at normal laptop resolution while remaining usable on smaller screens.
