# Stage 11 — Docs, GitHub Pages and Zero Drift

Ensure every public statement/surface is generated or validated against canonical runtime truth.

## Tasks

1. Map all docs/GitHub Pages inventories to their canonical sources: capabilities, commands, rules, doctrines, providers, claims, milestones/issues, versions, routes.
2. Eliminate manually duplicated inventory tables/nav/status lists where derivation is appropriate.
3. Preserve hand-written explanatory prose, but validate factual claims embedded in it where the claim system supports this.
4. Generate/update architecture diagrams/indexes from typed metadata where the repo already has generators.
5. Ensure canonical ordering is used in generated indexes.
6. Ensure site claim state is derived from claim/evidence graph.
7. Ensure Cockpit and GitHub Pages share the design system or enforce the documented relationship without copy/paste drift.
8. Add generated-artifact drift gates.
9. Verify links/anchors/routes and version references.
10. Build the full site using canonical commands.
11. If deployment credentials/network exist, deploy through the official pipeline and verify the published revision/status.

## Acceptance

- No stale page claims an unimplemented capability as complete.
- Generated docs are reproducible and `git diff` is clean after regeneration.
- Docs link every core rule/capability to operational/evidence pages where designed.
- GH Pages/site build passes and, where possible, deployed content is verified against landed commit.
