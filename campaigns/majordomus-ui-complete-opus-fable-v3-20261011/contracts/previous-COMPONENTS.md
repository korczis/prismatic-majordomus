# Reusable UI component design and fidelity

## Shared shell (all screens)
- Desktop 1440–1680 canvas: left rail roughly 216–236px, top toolbar 56–68px, 12–20px content gutters, panel borders subtle blue-gray, radius 10–14px. No clipped horizontal viewport. Never bake dimensions in screenshots as hard constants if responsive constraints require adapting.
- Branding and typography are derived from canonical design tokens, local system font stack. Palette: midnight/navy surface hierarchy, quiet cyan-blue borders, restrained blue active state, green success, violet composition, amber warning, red blocking. Status color semantics must follow canonical tokens, not per-page styling.
- Hero uses visual mountain/prism asset **only if existing licensed brand assets permit it**; never use untracked or external copyrighted screenshot as a production background. Dynamic text/content derives from actual project context. No invented inspirational quotes attributed to Majordomus.
- Header: repo picker, search, keyboard shortcuts, docs/CLI/API, live connection/health, theme control. Render only options supported by available canonical capabilities and authorization.
- Sidebar: discoverable area registry with section counts bound to real source; active item by route; at mobile use focus-trapped drawer and accessible escape/close.

## Component contracts
- `AppShell`: resolved project context, nav links, account capabilities, status with source fidelity.
- `Hero`: localized headline/description curated as explanatory prose; project/release badges only from runtime.
- `MetricCard`: name, unit, optional measured value, window, delta computed from comparable windows, provenance/observed time, explicit unmeasured.
- `StatusBadge`: typed semantic state and label from canonical domain vocabulary; no arbitrary green from string matching.
- `EntityTable`: typed schema columns, stable identities, server pagination/filter support, row link to canonical details; accessible mobile list alternative.
- `DetailPanel`: selected object, linked entities, evidence actions, logs, loading/error, drawers on narrow screens.
- `ActivityTimeline`: actual event sequence with dedupe/reconnect semantics, absolute timestamp in title and relative time in display.
- `RelationshipGraph`: typed nodes/edges, canonical IDs and relations, Cytoscape only on graph pages, HTML table alternative; incremental graph updates and keyboard-selectable list.
- `TimeseriesChart`: ECharts only where evidence has aligned timestamps/windows/units; chart can say unmeasured. Screen-reader summary, data table and accessible filters.
- `GanttTimeline`: milestones with dates only where explicitly known, dependency-aware ordering; unscheduled items in separate honest list.
- `ActionRunner`: derived capability schema, validated input, server-side auth, explicit confirmations only for genuinely destructive/high-impact actions, durable execution ID.
- `ProofPanel`: test/claim/evidence provenance and freshness and distinguish verification from anecdote.
- `EmptyState`: typed reason and legitimate next action where available.

## Responsive behavior
- Desktop: 2–3 content columns, consistent grid and right rail when real content supports.
- 1024: minimize left nav; replace right rail by on-demand panel.
- 768: navigation drawer, charts/tables stack, graph height bounded.
- 375 and 320: one column, horizontally scroll table **within** container or transform into labeled card list; dialogs keyboard/focus safe; touch targets >=44 CSS px when possible.
- No-JS: SSR entity lists, status text, valid links and action forms where appropriate. JS graph/chart optional.

## Visual QA
Capture each page at widths 320/375/768/1024/1440/1680; compare typography, panel density, spacings, dark surface hierarchy, adaptive behavior and interaction semantics against concepts. Pixel likeness may not override truth, accessibility or mobile usability.
