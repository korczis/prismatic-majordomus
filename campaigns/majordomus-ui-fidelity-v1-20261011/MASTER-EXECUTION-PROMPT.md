# MASTER EXECUTION PROMPT — Majordomus UI, high-fidelity derived Cockpit

You are an expert Claude Opus/Fable repository engineer operating in `korczis/prismatic-majordomus` with authorized tools and existing Majordomus workflow. Your goal is to make the REAL `majordomus ui` experience visually close to `reference/dashboard-concept.png` while honoring the canonical repository architecture.

## Outcome
A user invokes the existing or appropriately extended `majordomus ui` command, sees a truthful, polished, keyboard-usable, responsive high-level interface, and can trace intent → milestone → issue → PR → release → deploy, inspect real metrics, mesh peers and evidence, and trigger authorized actions through the canonical executor. The public GitHub Pages site explains and links this product consistently, without impersonating live runtime data.

## Non-negotiable
- First inspect current repo and relevant PRs; use `docs/COCKPIT.md`, real Rust Cockpit, web topology, generated design tokens, governance and existing CI as constraints, not hypothetical architecture.
- Do not produce a new SPA, app server, business database, custom event ledger or alternative command runner.
- Prefer working server-rendered HTML plus small Alpine enhancements, Flowbite/Tailwind-based existing styling, ECharts for charts, Cytoscape for relations and optional p5 for decorative animation. Reuse installed libraries and current asset strategy wherever present. Avoid shipping a heavy library unless a real visual need and bundle/performance evidence justify it.
- No mocks, placeholders, invented statuses, trends, counts, release identifiers, avatars, peers, dates, CI percentages or timelines in production. The SCREENSHOT IS VISUAL ONLY. Never render illustration data as real.
- All state transitions are authorized and executed server-side. Defer feature UI entirely if its backend contract cannot truthfully support it; show an honest unsupported/unknown state in the interim.
- Preserve compatibility, use same schema/registry source in CLI/API/OpenAPI/MCP/Cockpit/Docs, derive artifacts, enforce via real CI gates, and release progressively with evidence.
- No repeated conversational `proceed` prompts: use existing authorized mesh/supervisor if available, checkpoint and continue within permissions, record blockers without falsely claiming autonomous completion.

## The reference image
`reference/dashboard-concept.png`: wide, polished dark dashboard. LEFT 210–230px navigation rail. TOP 56–64px toolbar. MAIN hero band then six KPI cards. MID central left ~2/3 Cytoscape flow, right mini network panel. LOWER three charts; BOTTOM PR drain, mesh peers, roadmap cards. RIGHT activity rail ~285px. Thin luminous borders, midnight/blue-black background, understated cyan/indigo/violet/green highlights, dense information but clear spacing. Preserve proportions and information hierarchy across 1440–1680px desktop; on mobile replace rather than squeeze columns. Pixel similarity is important; truthful data and usability outrank literal screenshot text.

## Execution protocol
1. Read README, VISUAL-SPEC, DATA-CONTRACTS, manifest and all prompts 00–09. Reconcile work with current canonical intents/milestones/issues/PRs. No duplicate issues.
2. Perform prompt 00 audit and record source-of-truth map and baseline screenshots before changing architecture.
3. Derive canonical high-level projection with authoritative provenance and states (01). Build shell/card UX (02), then graph/chart/operational components (03–07), then enforcement/docs/pages (08), continuous verification/release/rollout (09).
4. Split implementation into 50 atomic issue slices from manifest; prioritize vertical slices: data contract + renderer + actions + tests + docs + enforcement for each domain. Dependents cannot run before prerequisites. Parallelize nonconflicting slices via existing mesh only.
5. At every green integration slice: format, run targeted gates, generate canonical projections, stage all relevant inputs, check drift, inspect diff and PR collision scope, merge using existing policy, analyze version bump, trigger authorized release/deploy when appropriate, verify UI health and Pages freshness, publish execution evidence and continue.
6. Maintain `TRACKING.md` in the pack as a human planning aid; actual progress must be in canonical repository issues/evidence. `TRACKING.md` is not a runtime source of truth.
7. Stop only when verifiably complete, genuinely externally blocked, or permission/budget exhausted. Never weaken required security/integrity checks to claim success.

## Final evidence
Produce links to relevant commits/PRs, exact changed files, verified commands and durations, screenshots at 1440/1024/768/375, visual diff summary, a11y findings, p95 metrics where actually measured, exposed capability data provenance, mesh execution evidence, version/tag/release URL if published, live Pages URL and verified deploy SHA, real remaining blockers. No success claim without corresponding evidence.

## Binding architectural rules (apply to EVERY issue)
1. Inspect current `AGENTS.md`, `.ai/**`, ADRs, schemas, tests, `docs/COCKPIT.md`, `apps/majordomus-cli/src/cockpit/**`, `apps/majordomus-cli/src/web/**`, `share/design/tokens.yaml`, `site/**`, and current GitHub issues/PRs before editing. Names are hints, discover actual paths and runtime behavior.
2. Cockpit remains a server-rendered enhancement of canonical Rust `capability!` + typed registries, index, graph and executor. No parallel SPA, no parallel command registry, no hand-maintained navigation/source-of-truth, no JS business decisions, no extra server or independently tracked status.
3. Reuse `share/design/tokens.yaml` and generated theme/CSS/status vocabulary. Respect existing Flowbite/Tailwind semantics and actual component contracts. Never modify generated `apps/majordomus-cli/src/design/tokens.yaml` directly. Prefer reusable view primitives and clean, minimal front-end enhancement.
4. Use existing API/OpenAPI/MCP execution and event channels. The same action must have the same server-side authorization, validation, execution ID, and evidence regardless of the UI entry point. A browser is not an authority.
5. Every displayed count, percentage, trend, version, branch, user, peer, CI latency, PR ID, roadmap date, activity event and health indicator comes from a canonical typed projection and identifies its provenance/freshness. NO mock values, fake avatars, placeholder chart series, invented release names, illustrative activity events or synthetic network topology in production.
6. Loading, missing, unsupported, permission-denied, offline, error, and empty states are separate, legible, accessible states. If a metric is not measured, show `Not measured`, not zero or a fabricated trend. Never display `Autonomy enabled` unless the run is actually authorized and active.
7. Keep HTML useful and navigable with JS disabled; graph/chart enhancement progressive. Keyboard, 320/375/768/1024/1440 widths, dark/light themes, contrast, reduced motion and focus handling must work. Avoid indiscriminate chart library loading.
8. Tests and enforcement MUST be executable in canonical CI paths; changes to `.ai/` rules and docs in the same increment. No decorative Markdown rules; migration/removal of superseded rules required. Inspect and avoid widening expensive unconditional gates.
9. Work in incremental vertical slices, reconcile canonical intent/milestones/issues/PRs, run targeted validation, derive artifacts in repository order, commit, merge if authorized and green, analyze version bump, release/deploy if eligible and authorized, then probe served Cockpit and public Pages. Never claim completion with missing evidence.
10. Adopt the provided screenshot for visual *composition*, not data or product claims. All mock screenshot labels, names, versions and metrics are non-authoritative fiction.
