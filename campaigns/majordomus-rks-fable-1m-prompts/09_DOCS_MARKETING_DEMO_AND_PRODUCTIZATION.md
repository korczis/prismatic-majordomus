# Phase 9 — Documentation, Product Positioning, Demo, and Commercial Readiness

Use the master contract and the working RKS implementation.

Turn RKS into a coherent Majordomus product capability without creating marketing/docs drift from implementation.

## Documentation architecture

Inspect the existing Zola/GitHub Pages/docs generation pipeline. Integrate RKS using current metadata/discovery conventions.

Do not hand-maintain another navigation tree if existing route/docs metadata can generate it.

Create/extend RKS documentation covering:

### Overview

Explain RKS as a continuously reconciled repository knowledge plane, not merely documentation generation.

### Brownfield adoption

Show how an existing repository adopts Majordomus later:

```text
majordomus init / knowledge bootstrap
```

Explain discovery, ownership, baseline, and non-destructive behavior.

### How Majordomus knows what it knows

This should be a flagship page.

Explain:

```text
Evidence → Claim → Provenance → Confidence → Freshness → Relations
```

Show examples of observed vs declared vs derived vs curated knowledge.

### Drift / reconciliation

Explain how repository changes invalidate knowledge, impact is computed, and reconciliation works.

### Existing docs

Explain external/hybrid/Majordomus ownership and why RKS does not create a parallel docs universe.

### CLI

Generate or link canonical CLI docs from Rust metadata. Include useful end-to-end examples.

### API / OpenAPI / MCP

Document the interfaces from generated schemas/metadata.

### Cockpit

Explain Overview, Graph, Impact, Gaps, Conflicts and evidence drilldown.

### Extending RKS

Document extractor/plugin/kind extension with a complete tested example.

### Security

Explain local vs remote processing, secret handling, provider policy and visibility.

### Troubleshooting

Cover stale cache, unknown IDs, baseline mismatches, schema migration and conflict interpretation.

## Marketing positioning

Preserve these core themes, adapting language to existing Majordomus brand tone:

### Primary value proposition

> Your repository should know what it knows.

### Strong secondary line

> Documentation that knows when it may be wrong.

### Agent pain

> Stop re-explaining your codebase to every new AI session.

Avoid commodity positioning such as “AI-powered docs generator.”

## Product story

Explain three converging use cases:

1. developer onboarding
2. agent context / AI development
3. governance / auditability

Demonstrate that the same provenance-backed system supports all three.

## Landing page / why pages

Integrate RKS into current Majordomus landing/why structure using generated/discovered page conventions.

Add concrete pain-driven examples such as:

- architecture doc says one thing, code says another
- new agent session lacks repository context
- feature branch changes an API but no one knows which docs/decisions are affected
- legacy repo has useful docs scattered across formats

Each example should link to an actionable feature/demo route if current site architecture supports it.

## Demo scenario

Create a deterministic demo fixture or scripted example that demonstrates:

1. brownfield repo with existing docs
2. bootstrap registers rather than duplicates them
3. a repository change affects known knowledge
4. `knowledge impact` explains what is affected
5. a conflict is visible
6. reconciliation restores health
7. Cockpit graph/evidence view reflects the state
8. MCP/agent retrieves relevant context

The demo must not require proprietary credentials for basic operation.

## Commercial packaging metadata

If the repository already has feature/capability tier metadata, register RKS appropriately. Otherwise document productization boundaries without hardcoding pricing into implementation.

Conceptual packaging:

### OSS/local

- local extraction
- CLI
- Markdown projection
- basic graph
- drift detection
- MCP

### team/paid future

- shared state/history
- PR knowledge gates
- centralized collaboration
- advanced analytics/integrations

### enterprise future

- RBAC/SSO
- governance
- evidence retention
- cross-repository knowledge
- private deployment

Do not implement fake enterprise stubs solely for marketing.

## Public-safe publishing

Ensure public docs/site generation cannot accidentally publish restricted knowledge. Respect visibility metadata and add tests around public projections if RKS data is exposed in GitHub Pages.

## Developer onboarding

Add a short quick-start that gets a user from clone/install to first useful RKS result with minimal configuration.

Avoid wizard sprawl. Infer first, ask only when ambiguity cannot be resolved safely.

## Acceptance criteria

- RKS docs are comprehensive and generated/discovered consistently.
- Product messaging emphasizes reconciliation/provenance, not generic AI docs.
- A deterministic end-to-end demo exists.
- Existing docs/landing architecture is reused.
- Public publishing respects visibility.
- Extension and security docs are production-worthy.
