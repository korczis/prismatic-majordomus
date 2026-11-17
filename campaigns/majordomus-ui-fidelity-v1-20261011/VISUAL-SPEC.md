# Visual specification — screenshot fidelity without fabricated data

## Canvas & proportions
Reference 1672×941 desktop concept. Overall application uses an adaptive 3-zone grid: left side navigation ~212px; content fills remaining width, right live activity rail ~285px. Toolbar 56px. Outer gaps 12–16px, internal cards 12–20px, radii 9–13px. At 1440 desktop, layout can reduce sidebar/right rail; do not introduce horizontal scroll for dashboard itself. Background charcoal-navy; cards one elevation brighter, borders subtle cool blue-gray; typography system font per canonical tokens. Main headline bold ~28–32px, KPI values ~22px, labels ~12–14px, monospace only machine identities.

## Region-by-region reference
1. **Brand/sidebar:** glowing geometric mark sourced from canonical brand asset, `majordomus ui`, small subheading; vertical icons + labels for Overview, Intents, Milestones, Issues, PR Drain, Mesh, CI/CD, Releases, Docs, Roadmap, Settings IF these destinations exist. Real counts only and relevant active state. Bottom small autonomy status panel can say `No autonomous run` when none active. Do not claim anything is 'enabled' based on config alone.
2. **Toolbar:** command/search field with keyboard hint; canonical repository/branch switchers and verified release status; profile only if identity is actually known and authorized. No fictional Alex Chen, no invented GitHub repo.
3. **Hero:** dark mountain-like tonal depth is a stylistic reference, not an instruction to load a copyrighted photograph or claim capabilities. State truthful summary: e.g. `Development overview` or `Delivery in progress` only if active. Prefer a quiet gradient / CSS pattern. Real active intent, PR, health counts.
4. **KPI strip:** 6 cards in reference: PR Queue, CI Latency, Active Intents, Deployed Version, Mesh Peers, Blockers. Label units, trend windows, missing data, sample sizes and sources in accessible details. No derived numbers from arbitrary frontend heuristics. Avoid filling missing cards with zeros.
5. **Development Flow:** colored connected graph `Intent → Milestone → Issues / PRs → Release → Deploy`. Render only canonical confirmed edges. Distinguish incomplete/unlinked paths. Legend actual domain state vocabulary. Action `View full graph` leads to derived graph view.
6. **System Activity motif:** compact gently animated point/edge network. If p5 employed, PURE AMBIENT illustration with accessibility `aria-hidden=true`. It must not imply 8 peers or 42 tasks unless sourced by adjacent real text. Never silently animate against a reduced-motion preference.
7. **Metrics:** 3 evidence-backed cards: CI Time (line/area), Delivery Throughput (bars), Release Cadence (timeline). The screenshot is layout/appearance reference only. Empty datasets show useful explanation and path to create evidence.
8. **Lower tables:** PR Drain rows (ID/title/phase/age/owner/overflow), Mesh Peers (identity/capabilities/last-seen/health), Roadmap (milestone title/dependency/state/evidence). Click through, authentic URLs, responsive table alternative.
9. **Right activity:** vertical timestamped event feed. Stream only actual events with event identity, type, timestamp, source and relation to execution/PR/release. Event order deterministic. Never copy image's made-up event examples.
10. **Footer:** docs, CLI, API/Swagger links derived from web topology; no dead links and no redundant hand-maintained nav.

## Responsive matrix
- ≥1440: sidebar + content + right rail, six KPIs; diagrams/charts panel density similar to concept.
- 1024–1439: sidebar compresses, activity rail becomes collapsible, KPIs 3 columns.
- 768–1023: nav drawer, topbar compact, two-column cards, graph full width, activity beneath.
- 320–767: sticky mobile toolbar, accessible drawer, single-column cards; timeline, charts and lists never clip; interactive legend and chart controls accessible; user has a full fallback table instead of miniature unreadable graph.

## Interaction/visual acceptance
Visual snapshot comparisons at 1672x941, 1440x900, 1024x768, 768x1024 and 375x812; compare spacing, panel sequence, hierarchy and color roles, not literal screenshot numbers. State badges must use canonical status tokens. Check keyboard-only journeys, reduced motion, text zoom 200%, focus return, touch targets, light theme where current design contract requires it, and browser errors. Avoid illegible decorative microtext.
