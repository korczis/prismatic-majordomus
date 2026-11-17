# MASTER EXECUTION PROMPT — Majordomus UI V3 (Opus / Fable)

You are executing a **repository-changing mission**, not writing a proposal. User will supply this whole conversation and its UI references to Claude Code. Treat images as visual design references, **never as facts about what Majordomus currently has**. Use the actual HEAD, current PRs, repository docs, `AGENTS.md`, `.ai/**` and real executable to determine behavior.

## Mission
Rebuild and converge **existing** `majordomus ui` / `/cockpit` into one highly polished, cohesive, interactive, mobile-first, accessible, server-rendered experience spanning Overview, Command, Roadmap & Intents, Issues & PRs, PR Drain, Mesh, CI/CD, Deployments, Capabilities, Evidence, Documentation and Governance. Aim for strong visual fidelity to ALL supplied screenshots, not literal re-creation of fictitious values. The screenshots overlap: synthesize their best aspects into a single consistent design system. Build for arbitrary adopter repositories rather than this project's hardcoded names.

## Current architectural constraints to verify on live code
- Existing `apps/majordomus-cli/src/cockpit/mod.rs`: Cockpit is a server-rendered **projection**, not a separate application; nav and runner come from registry/capability executor. Existing server uses `/cockpit` and routes registered through current mechanisms. Preserve compatibility and make `majordomus ui` reach that same server/projection; locate command implementation instead of inventing a competing launcher.
- `apps/majordomus-cli/src/web/mod.rs`: web topology is discovered; no manually duplicated route inventories.
- `share/design/tokens.yaml` owns design values; `apps/majordomus-cli/src/design/tokens.yaml` is generated. Extend tokens in authority, regenerate projections, never edit generated copies.
- Respect actual `.ai/` rules, schemas, gate model, ADRs, CI, UI tests, generated assets and docs. Amend superseded invariants with migration and executable proof; do not add empty ceremonial rules.

## Non-negotiable truths
1. ZERO mocked/stubbed/placeholder operational data in production, including names, avatars, PR numbers, release versions, statuses, graphs, target percentages and charts from screenshots. Synthetic fixtures may exist in TESTS only, isolated from production and clearly labeled.
2. Every rendered datum is tied to a canonical typed producer, entity ID, observation time, freshness semantics and evidence/provenance where applicable. Unknown is `unknown`, not zero, healthy or green. Cached is not `live`; disconnected is not `online`.
3. Every view has usable SSR content and links without JS. Flowbite + Tailwind tokens for components, Alpine.js for progressive controls, Cytoscape.js for canonical graphs, Apache ECharts for measured series, p5.js ONLY decorative/lazy and optional. Pin and serve dependencies from supported local pipeline, respect CSP.
4. All write actions go through already authorized canonical capability/executor with typed input schemas, validation, CSRF/security where applicable, audit/execution IDs and eventual outcome. Do NOT create arbitrary shell routes or unreviewed GitHub mutations.
5. Canonical registries own capability/route/navigation metadata. UI adds presentation components, never alternative domain orchestration or a second status/permission engine.
6. Detect and reconcile all overlapping active PRs and existing milestones/issues. Create no duplicate work. Use Majordomus mesh where actually available and authorized; don't claim durable autonomy if no working supervisor exists.
7. Each vertical slice is complete: model → SSR → progressive behavior → negative tests → doc → `.ai/` gates → merge → version analysis → authorized release/deploy → live verification, with evidence captured in the canonical project model.

## Immediate process
- Execute prompts `M00` through `M13`, respecting dependencies. Within each milestone complete the five atomic tasks in its file, refining work IDs into EXISTING canonical intent/milestone/issue records rather than duplicating them.
- Enumerate all screenshots from `references/` with hashes; create a visual taxonomy of components / pages / interaction patterns / display states. NEVER copy strings/numbers from screenshot into runtime source.
- Baseline real pages at 320, 375, 768, 1024, 1440 and 1680 px; compare screenshots after each slice (mask only nondeterministic timestamps and truly live values, not broken layout).
- Measure LCP/layout shift JS bundle, route render time, graph redraw performance, endpoint/event errors and relevant data freshness. Profile before optimizing.
- Parallelize only conflict-free implementation tasks, maintain canonical checkpoints, integrate frequently. If blocked, mark exactly which external capability, permission or record is missing and continue independent useful work.
- No generic textual promises of 'real time': define source transport, delivery lag, reconnect/replay, staleness windows and prove them in tests.

## Required delivery evidence
For each route: screenshot reference IDs; actual URL; capability and producer; tested states (real, empty, denied, stale, offline); action execution ID where relevant; UI and API parity; no-JS SSR proof; mobile/a11y results; bundle/perf; commit/PR/CI status; version/release/deploy SHA and verified URL. Public GitHub Pages is static and MUST display snapshot source/time, not claim live state. Actual user-facing Cockpit must use real runtime server data.

Begin NOW with M00 audit and real branch/PR reconnaissance. Do not wait for repeated 'proceed' prompts within authorized operation. Never claim deploy or completion without live proof.
