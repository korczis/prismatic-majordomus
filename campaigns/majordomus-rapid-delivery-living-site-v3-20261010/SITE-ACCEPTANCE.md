# Living landing page: user-facing acceptance specification

## First 10 seconds
Visitor understands what problem Majordomus solves and what it does not do. Hero explains outcome in plain language. CTAs: install and inspect a real proof/demo. Avoid unverifiable "fully autonomous" copy until product evidence supports it.

## Content hierarchy
1. Promise and limitations in one screen.
2. Three-step visual flow: intent/context → worker/mesh → repo-verified result.
3. Clear pain points (lost context, agent collision, false done, stale PRs, slow release) and corresponding actual capability links.
4. 60-second example with real commands and outcomes drawn from executable use case/evidence.
5. Features indexed/grouped from canonical feature registry, no hardcoded cards.
6. Guarantees with tests and provenance, plus a visible limitations/roadmap section.
7. Changelog "since this site's previous deployed revision" derived from Git/release and capability diff, labeled delivered vs upcoming.
8. Install, platform support, privacy/security model, link to API/MCP/Cockpit, references.

## Presentation requirements
Use existing tokens/Tailwind/Flowbite if present, responsive at 320px through desktop, keyboard friendly navigation and dialogs, contrast and reduced motion, optimized images and fonts, progressive enhancement. Graphs/charts only where they explain real flows, not decoration. All interactions real, accessible and mobile-tested.

## Truth and synchronization contract
Site release/version, homepage badges and install links refer to actual currently deployed artifact (or explicitly label release channel). Never derive "latest" solely from git HEAD. Capability shipping state must be supported by build/test/release evidence. If multiple environments differ, show each with explicit label.

## Minimum automated checks
Version identity consistency, baseline delta correctness, no draft feature presented as shipped, canonical inventory propagation, link validation, keyboard navigation, 320/375/768/1280 widths, basic accessibility, SEO/canonical links, build size budgets, deployed asset SHA and API health, rollback replay and stale-cache behavior.
