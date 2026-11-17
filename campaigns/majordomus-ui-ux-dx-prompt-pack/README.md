# Majordomus UI/UX/DX Prompt Pack

Purpose: evolve `prismatic-majordomus` into a user-friendly, browser-first, derived and enforced UI/UX/DX system without creating a parallel frontend universe.

Primary consumer: Claude Code Opus 5.5 or equivalent high-capability coding agent.

## Core invariant

Majordomus discovers/models truth once, derives typed presentation projections once, and renders them through every surface. The UI is a projection, never a source of domain truth.

Canonical flow:

```text
repository/domain state
  -> typed canonical registries / snapshots
  -> canonical projection layer
  -> CLI / API / MCP / Cockpit / docs / visualizations
```

Client stack target:

- Tailwind CSS: layout, tokens, responsive utilities
- Flowbite: preferred component primitives where suitable
- Alpine.js: ephemeral local interaction state only
- Cytoscape.js: canonical graph projections
- p5.js: specialized exploratory/ambient visualizations only when justified

## Non-negotiable principles

- no parallel inventories in templates or JS
- no frontend-owned durable domain state
- no hardcoded nav catalogs duplicated from backend capabilities
- no client-side reconstruction of domain relationships that backend already knows
- no Tailwind classes in domain types
- semantic presentation tokens, not framework-specific domain metadata
- progressive disclosure for non-technical users
- task-oriented UX before entity-browser UX
- accessibility, mobile, keyboard and dark/light mode are first-class
- every displayed state must be explainable and traceable to canonical source data
- new canonical entities/features should propagate to relevant UI surfaces without manual registration in each consumer
- all architecture must be validated by tests/gates, not by documentation alone

## Suggested execution order

1. `00-master-orchestrator.md`
2. `01-current-ui-audit.md`
3. `02-canonical-presentation-projection-layer.md`
4. `03-design-system-tailwind-flowbite.md`
5. `04-derived-navigation-and-actions.md`
6. `05-task-oriented-overview-and-workspace.md`
7. `06-graph-projections-cytoscape.md`
8. `07-explainability-provenance-and-evidence-ui.md`
9. `08-responsive-accessibility-and-mobile.md`
10. `09-non-programmer-personas-and-progressive-disclosure.md`
11. `10-testing-enforcement-and-zero-duplication.md`
12. `11-docs-dx-onboarding-and-examples.md`
13. `12-final-integration-deploy-and-verification.md`

Each prompt is intentionally repository-aware but path-agnostic. The agent MUST inspect the current repository and reuse existing architecture, doctrines, rules, schemas, APIs, registries and test infrastructure instead of blindly creating new parallel abstractions.
