# 07 — Enforcement, Rules/Doctrines, Docs and Generated Surfaces

Turn the architecture into a repository law that future agents cannot casually bypass.

## Rules / doctrines

Find the canonical definitions of rule vs doctrine and use them correctly.

Encode enforceable invariants for:

1. automatic control-plane convergence,
2. single canonical domain implementation,
3. derived multi-surface exposure,
4. thin hooks/provider adapters,
5. typed/versioned schemas,
6. no manual duplicate inventories,
7. E2E cold-start requirement,
8. deterministic state,
9. security/redaction,
10. capability matrix completeness.

Do not create decorative prose only. Wire them into executable validators/gates.

## Capability matrix

Generate/derive a machine-readable capability matrix from canonical metadata if that fits existing architecture.

It should detect missing exposure where a capability declares it should exist.

Do not force every capability into every surface blindly.

## Documentation

Update the canonical docs hierarchy and GitHub Pages generation as appropriate:

- architecture overview,
- lifecycle diagram,
- operator quickstart,
- troubleshooting/doctor,
- provider adapter extension guide,
- MCP/API/Cockpit discovery,
- session/peer/claim semantics,
- security model,
- recovery from stale/crashed runtime,
- testing/validation.

Generated inventories must stay generated/derived.

## ADR

Follow repository ADR doctrine. If this change qualifies, create/update the ADR covering desired/observed state, reconciliation, identity scope, protocol, provider adapters and derived surfaces.

## Gates

Integrate with existing local/CI quality commands, git hooks and generated-drift checks without inventing redundant pipelines.
