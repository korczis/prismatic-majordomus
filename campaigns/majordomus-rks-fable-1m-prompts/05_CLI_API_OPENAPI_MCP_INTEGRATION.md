# Phase 5 — CLI, API, OpenAPI/Swagger, MCP, Completion, and Just Integration

Use the master contract and completed RKS backend.

Expose RKS through Majordomus interfaces using the repository’s canonical metadata/discovery systems. Do not manually duplicate command/route/tool descriptions across layers.

## CLI

Add polished RKS commands aligned with current CLI conventions.

Target capability set, adapted to actual existing names:

```text
majordomus knowledge
majordomus knowledge bootstrap
majordomus knowledge scan
majordomus knowledge status
majordomus knowledge list
majordomus knowledge show <id>
majordomus knowledge search <query>
majordomus knowledge explain <id>
majordomus knowledge graph
majordomus knowledge impact [revision/range]
majordomus knowledge gaps
majordomus knowledge coverage
majordomus knowledge stale
majordomus knowledge conflicts
majordomus knowledge reconcile
majordomus knowledge validate
majordomus knowledge baseline
majordomus knowledge check
```

### Default `knowledge` UX

Render a compact knowledge health dashboard showing useful counts and next actions. Do not fabricate percentage coverage unless backed by deterministic denominators.

### Explain UX

`knowledge explain` should expose:

- provenance
- confidence and basis
- evidence
- freshness
- dependents/relations
- last relevant revision
- why Majordomus believes the claim/node

### Impact UX

Clearly separate:

- definitely affected
- possibly affected
- unaffected summary

Include reasons.

### Machine output

All relevant commands should support the repository’s standard JSON/machine output mode derived from the same typed response objects.

## Dynamic completion

Integrate knowledge IDs/kinds and subcommands into the current shell completion infrastructure.

Examples:

```text
majordomus knowledge explain <TAB>
majordomus knowledge show <TAB>
```

must derive candidates from current registry/state without a handwritten list.

## Just bridge

If Majordomus currently bridges or generates `just` tasks, expose high-value RKS actions without duplicating implementation.

Examples:

```text
just knowledge
just knowledge-check
```

should delegate to `majordomus` or be derived from command metadata. Do not create a second execution path.

## HTTP API

Expose RKS through the existing server/router architecture.

Representative endpoints, adapted to repo style/versioning:

```text
GET /knowledge
GET /knowledge/{id}
GET /knowledge/search
GET /knowledge/graph
GET /knowledge/coverage
GET /knowledge/gaps
GET /knowledge/stale
GET /knowledge/conflicts
GET /knowledge/impact
POST /knowledge/reconcile
POST /knowledge/validate
```

Use typed request/response models from the canonical RKS domain layer.

## OpenAPI / Swagger

Ensure API schemas/routes appear automatically through existing OpenAPI generation. Do not hand-maintain a parallel OpenAPI file.

Descriptions/examples should derive from code metadata or existing documentation conventions.

Verify rendered Swagger/OpenAPI is correct and useful.

## MCP

Expose RKS via existing MCP registry/protocol abstractions.

Target tools/resources where meaningful:

```text
knowledge_search
knowledge_get
knowledge_explain
knowledge_related
knowledge_impact
knowledge_conflicts
knowledge_gaps
```

MCP outputs should use the same canonical model and respect visibility/security policy.

Provide agent-friendly concise forms when huge graph payloads would waste context.

## Cross-interface consistency

Add tests asserting that representative RKS data is semantically consistent across:

```text
CLI JSON
HTTP API
MCP
OpenAPI schema
```

Do not test string formatting only; test the same underlying domain fields.

## Completion/docs generation

If Majordomus already generates CLI docs from Rust metadata, ensure every new command appears automatically in generated docs. Extend doctest/example machinery rather than adding static docs lists.

## Error semantics

Provide stable structured errors for:

- unknown knowledge ID
- unavailable baseline
- invalid revision range
- reconciliation blocked by ownership/policy
- stale cache/state requiring refresh

Map them consistently across CLI/API/MCP.

## Tests

Required:

- CLI E2E for key commands
- completion smoke tests if harness exists
- API E2E
- OpenAPI schema assertions
- MCP E2E
- cross-interface consistency test
- just bridge tests where applicable

## Acceptance criteria

- RKS is first-class in CLI/API/OpenAPI/MCP.
- Dynamic completion works from real registry/state.
- No duplicated manual command/route/tool registry is introduced.
- Machine output is typed and consistent.
- Generated CLI/API docs update automatically.
- Tests verify end-to-end behavior.
