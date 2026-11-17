# Component Architecture & Visual Contract

## Global shell
- `AppShell`: dark navy canvas, 240px sidebar desktop, collapsible mobile offcanvas, sticky top global search and tools, keyboard focus semantics, clean 8px-based spacing. Content density scales; no forced side-scroll at 320px.
- `PageHero`: short title, summary, optional *derived* current release/status and subtle scenic artwork; pictorial imagery is visual only. Screenshots use mountain/prism, but production logo/brand assets must respect `brand/` + generated design tokens.
- `Navigation`: derived from canonical pages/capabilities with stable IDs, active route, counts from real sources, mobile and keyboard parity.
- `EntityLink`: stable route and href into actual Cockpit/source/GitHub, never `href="#"` or disabled-looking fake action.

## Component candidates (reuse existing if present)
`MetricCard`, `MetricDelta`, `FreshnessIndicator`, `StatusPill`, `ProvenanceTooltip`, `EntityTable`, `QueryFilters`, `TabStrip`, `SplitInspector`, `ActivityFeed`, `ExecutionTimeline`, `LogStream`, `GraphCanvas`, `GraphLegend`, `ChartCanvas`, `GanttTimeline`, `DependencyGraph`, `CoverageMatrix`, `EvidencePanel`, `MarkdownViewer`, `CodeDiff`, `SchemaRunner`, `ActionResult`, `EmptyState`, `PermissionState`, `StaleState`, `ErrorState`, `ResponsiveDrawer`, `SearchPalette`, `Breadcrumbs`, `CopyLink`.

- Component responsibilities: SSR semantic HTML; data projection owns domain semantics; client adapter owns transient UI state only; action invokes canonical executor. No component calls GitHub independently when an existing backend projection owns GitHub reconciliation.
- Avoid giant universal components with dozens of boolean flags; small composable partials based on existing Rust renderer idioms.
- Distinguish live stream from snapshot and from historical evidence. Embed `as_of`, source SHA or event sequence next to relevant content, with honest unknown states.

## Library boundaries
- Flowbite/Tailwind: CSS/component semantics, avoid copying all Flowbite templates into custom static inventory.
- Alpine.js: tab selection, panels, selection, search inputs, progressive behavior; no policy or permissions.
- Apache ECharts: accurate metrics and axes with units and provenance; lazy only on chart screens; dispose listeners/instances.
- Cytoscape.js: actual canonical node+edge IDs, typed edge labels and selection deep-links; lazy and no-js text/list fallback; bounded layout.
- p5.js: optional noninteractive background only, gracefully disabled under reduced motion/low memory. Must not obscure text or create data visualizations.

## Browser interaction matrix
Search → canonical search / scoped query. Row click → stable detail route. Graph click → entity detail. Chart click → bounded filter or details with grounding. Run → schema-backed validation, authorization, job tracking. Merge/release/deploy → permitted action + policy gate + evidence. Keyboard focus, Escape, ARIA, reduced motion and URL state/refresh preservation are required.
