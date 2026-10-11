# Milestone 6: Canonical Graph Projections + Cytoscape

Introduce first-class graph projections and render them with Cytoscape.js where graph structure materially improves comprehension.

## Candidate graphs

Only implement graphs backed by actual Majordomus relationships, such as:

- intent -> milestones -> issues -> criteria -> evidence
- session -> handover -> branch/commit/work item
- mesh peers -> capabilities -> assignments
- rules -> doctrines -> validators/gates
- artifact provenance
- dependency/impact graph

Do not fabricate graph edges in frontend heuristics.

## Architecture

```text
canonical relationships
  -> GraphProjection
      nodes
      edges
      groups
      semantic kinds
      actions
      provenance
  -> Cytoscape renderer
```

## Graph contract

Each node/edge should have stable IDs and typed semantics.

Do not encode Cytoscape CSS selectors in domain models.

## Interaction

Support useful interactions:

- zoom/pan
- select node
- inspect details
- follow canonical links
- filter by semantic kind
- reset layout
- focus current intent/context

Keep local viewport/selection state in the client.

## Layout

Choose layouts based on graph semantics, not random aesthetics.

Persist layout only if the repository has a genuine user-state model for that purpose; otherwise treat layout as ephemeral presentation state.

## Cross-surface reuse

Where useful, expose the same graph projection through:

- API structured output
- MCP
- CLI JSON
- docs generation

Do not create a web-only relationship model.

## Tests

- deterministic graph node/edge IDs
- no duplicate edges
- no dangling references
- canonical source relations preserved
- graph JSON stable for stable input
- frontend renders representative small/large graphs

## Acceptance criteria

- at least one high-value graph is implemented end-to-end
- frontend performs no domain relationship reconstruction
- graph projection is typed and reusable
- graph rendering is accessible enough to have a non-graph fallback/list/detail path
