# Milestone 12: Final Integration, Deployment, and Verification

Perform the final repository-wide integration pass for the new Majordomus UI/UX/DX architecture.

Do not declare success because pages render locally.

## Integration audit

Verify:

- canonical projections are actually used
- old duplicate data paths are deleted
- no stale frontend inventories remain
- CLI/API/MCP/Cockpit semantics agree
- navigation/action derivation is live
- task-oriented overview is using real data
- graph views use canonical graph projections
- explainability uses canonical logic
- persona/progressive disclosure works
- responsive/accessibility expectations hold

## Required validation

Use the repository's actual canonical commands for:

- formatting
- build/compile
- unit tests
- integration tests
- UI/component tests
- E2E tests
- accessibility checks
- schema validation
- API/OpenAPI checks
- MCP tests
- generated artifact drift
- docs/site build
- repository quality gates

Do not invent substitute commands if the project already has canonical ones.

## Deployment

If the repository has deployed Cockpit/docs/Pages/runtime surfaces:

- deploy using existing supported process
- verify deployed version
- validate key routes
- verify assets and CSP/build integration
- verify no mock/hardcoded data sneaked into production
- verify canonical API data powers the UI

## Performance

Check that browser-first improvements did not make:

- page boot excessive
- frontend bundle unreasonable
- server projections expensive
- direnv/environment entry slower due to UI concerns

No network dependency should be added to repository-entry/banner paths merely for UI metadata.

## Security/privacy

Verify:

- no secrets rendered
- provider credentials never exposed
- debug payloads do not leak environment-sensitive data
- graph/provenance views respect existing authorization boundaries

## Final report

Provide:

### Before
- architectural/UI problems discovered

### After
- canonical projection architecture
- design system
- navigation/actions
- overview/workspace
- graphs
- explainability
- personas

### Duplicate state removed
List concrete legacy inventories/logic deleted.

### Enforcement
List tests, validators, doctrines/rules/gates.

### Validation evidence
Exact commands and outcomes.

### Deployment evidence
Exact deployment/verification evidence if applicable.

### Remaining debt
Only genuine unresolved items, each with reason and priority.

### Zero-registration proof
Demonstrate one representative new canonical entity/feature propagating to its intended UI consumers without manual per-consumer registration.

## Final invariant

The work is complete only when this is true:

> Majordomus models reality once, derives presentation semantics once, and renders them consistently through CLI, API, MCP, Cockpit, docs and visualizations. The frontend owns interaction state, not project truth.
