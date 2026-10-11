# Prompt 05 — Cockpit, CLI, API, OpenAPI and MCP Integration


You are working inside the `prismatic-majordomus` repository.

Non-negotiable execution discipline:
- Inspect the real repository before deciding architecture.
- Reuse existing Majordomus abstractions, registries, rules, doctrines, schemas, adapters, diagnostics, tests, and deployment paths.
- Do not create parallel systems if an existing canonical mechanism can be extended.
- Everything must be typed, deterministic, data-driven, discoverable, testable, documented, and enforced where appropriate.
- No hardcoded duplicate inventories across CLI/API/MCP/Cockpit/docs.
- No claims without repository evidence.
- No "implemented" status unless tests pass and the relevant runtime/deployment surface is verified.
- Preserve existing public interfaces unless a migration is justified and completed.
- Prefer deleting obsolete duplicated machinery over adding another adapter layer.
- Use the repository's real canonical commands for format/build/test/check/deploy. Discover them instead of inventing them.
- At the end, report root cause, architecture, files changed by purpose, tests executed, validation evidence, and remaining debt.


Mission:
Expose browser peers and browser tasks through all relevant Majordomus surfaces from one canonical model.

CLI:
- list browser-capable peers
- inspect a peer
- inspect capabilities
- run a supported browser task
- inspect task/result/provenance
- diagnose browser integration

Do not invent command names if the CLI already has generic `peer`, `mesh`, `run`, `doctor`, or similar namespaces. Extend the canonical command tree.

API/OpenAPI:
- reuse generic peer/task endpoints where possible
- expose browser-specific fields through typed schemas
- ensure OpenAPI is generated/updated automatically
- no manual Swagger-only definitions

MCP:
- browser peers/resources/tools should derive from canonical registries
- avoid duplicating capability definitions
- task invocation must obey same policy checks as CLI/API

Cockpit:
- peer state
- capabilities
- active/recent browser tasks
- errors/diagnostics
- provenance/evidence links
- connection/pairing visibility where appropriate

Do not implement frontend-only truth.
Cockpit must consume canonical backend data.

Tests:
- CLI structured output
- API contract
- OpenAPI drift
- MCP listing/invocation
- Cockpit data contract
- cross-surface peer/task identity
- no manual duplicate inventories

Acceptance criteria:
- same browser peer/task can be inspected consistently across surfaces
- adding a browser peer does not require editing every consumer
- OpenAPI/MCP/Cockpit reflect the canonical schema
