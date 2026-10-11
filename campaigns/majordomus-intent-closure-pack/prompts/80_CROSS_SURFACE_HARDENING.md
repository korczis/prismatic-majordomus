# Milestone 80: Cross-Surface Consistency, Enforcement and Regression Hardening

## Mission

After the vertical loop works, eliminate bypasses and make intent-driven semantics consistent across CLI, API/OpenAPI, MCP, Cockpit, docs, validation, generated artifacts and CI.

This milestone is deliberately about hardening, not adding another feature layer.

## 1. Inventory every intent consumer

Search repository-wide for:

- intent parsing/loading;
- criterion status calculation;
- issue→intent mapping;
- completion flags;
- critique checks;
- GitHub mapping;
- frontend status derivation;
- docs/generated registries;
- API/MCP models.

Find duplicated or stale logic.

## 2. Single-source semantics

Refactor so these are owned canonically once:

- intent resolution;
- criterion satisfaction;
- intent stage/completion;
- opposition disposition;
- completion blockers;
- GitHub mapping/reconciliation state.

Consumers may format/project, not independently decide.

## 3. CLI

Ensure relevant commands expose stable structured output and actionable human output for:

- intent validate;
- preflight;
- coverage;
- satisfaction/status;
- explain/blockers;
- opposition;
- plan derivation if introduced;
- reconciliation.

Do not proliferate commands if existing subcommands can be extended coherently.

## 4. API/OpenAPI

Expose canonical intent lifecycle data/operations through the existing API model. Generate/update OpenAPI from canonical schemas/routes; do not manually maintain Swagger-only schema copies.

Contract-test the important lifecycle transitions.

## 5. MCP

Expose intent state and allowed operations through existing MCP resource/tool architecture, preserving the same validation and completion semantics.

MCP must not be able to bypass CLI/server gates.

## 6. Cockpit

Verify Cockpit consumes canonical status rather than recomputing satisfaction. Add contract tests between backend payload and UI assumptions.

## 7. Docs and generated projections

Update architecture docs, user guides, reference docs, generated indexes and diagrams so "intent-driven" claims match implemented behavior.

Remove/rewrite conditional/future-tense claims that are now implemented. Do not claim features that remain absent.

## 8. Governance

Encode the final invariants in the repository's actual executable governance mechanism. Candidate invariants:

- intent work requires resolved intent or typed exemption;
- criteria map to work and evidence;
- opposition must precede execution;
- satisfaction derives from evidence;
- completion cannot bypass satisfaction;
- surfaces cannot implement independent intent state.

Use rules/doctrines/validators consistent with repository taxonomy.

## 9. Migration and dead-path removal

Find and remove:

- manual completion flags that conflict with derived state;
- legacy issue-centric completion shortcuts;
- duplicate frontend/API status logic;
- stale generated intent documentation;
- obsolete critique-presence-only checks superseded by opposition disposition;
- dead adapters created during transitional development.

## 10. Adversarial tests

Add explicit regression tests attempting to bypass intent semantics via:

- alternate CLI entry point;
- API mutation;
- MCP tool;
- direct session resume;
- GitHub closed issue;
- stale evidence;
- stale critique;
- manually edited state;
- missing criterion coverage.

Expected result: rejection or explicit policy-approved exemption, never silent success.

## 11. Performance

Ensure intent loading/evaluation does not turn ordinary session start/status into a pathological full-repository scan if current registries/indexes can answer cheaply. Benchmark or use existing performance gates where relevant.

## Acceptance criteria

- no supported surface bypasses canonical intent lifecycle rules;
- CLI/API/MCP/Cockpit agree on status and blockers;
- OpenAPI/docs accurately describe current behavior;
- governance/CI enforce the invariants;
- legacy duplicate semantics are removed;
- adversarial bypass tests fail correctly.
