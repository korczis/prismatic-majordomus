# Milestone 10: Enforcement, Contract Tests, and Zero-Duplication Gates

Turn the UI architecture principles into executable repository guarantees.

## Enforce these invariants

1. UI is a projection, never a source of domain truth.
2. Durable domain state is not owned by Alpine/frontend local state.
3. Equivalent navigation/action/status inventories are not manually duplicated across consumers.
4. New canonical entities/features automatically propagate to intended presentation surfaces.
5. Graph relationships come from canonical graph projections.
6. Renderer-specific classes do not leak into domain models.
7. Generated UI/docs artifacts cannot silently drift.

## Validation targets

Add or extend canonical validators to detect where practical:

- duplicate route/nav arrays
- duplicate action lists
- duplicate status/tone mappings
- duplicated domain enums in JS
- hardcoded domain catalogs in templates
- stale generated artifacts
- projection schema violations
- graph dangling references
- frontend assumptions about capabilities that backend marks unavailable

Avoid brittle grep-only rules if typed/build-time validation can do better.

## Cross-surface tests

For representative canonical fixtures, assert semantic agreement across:

- domain/projection
- CLI JSON
- API
- MCP
- Cockpit payload
- docs projection if applicable

## Zero-registration test

Add a fixture or controlled test entity and prove it appears in all relevant derived consumers without manually editing each consumer.

This test is central.

## Property/determinism testing

Randomize insertion/discovery order where relevant and prove stable projections/output.

## CI integration

Integrate into existing canonical quality gates, not a redundant one-off workflow.

## Acceptance criteria

- architecture violations fail locally and in CI
- zero-registration propagation is tested
- duplicated frontend truth is materially reduced
- failures include actionable diagnostics
