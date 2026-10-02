# 02 — Canonical Control Plane Model

Read `SPEC.md` and the completed audit.

Establish or repair the typed canonical domain model before wiring more surfaces.

## Required outcome

There must be one coherent representation for repository/worktree identity, desired runtime services, observed service instances/endpoints, health, sessions, agents/providers, capabilities, peers, claims/ownership, handovers and diagnostics.

Reuse existing types wherever they are already canonical.

Do not build a monolithic god object if repository architecture favors smaller domain modules. The requirement is canonical ownership and composability, not one enormous struct.

## Tasks

1. Identify every duplicate model of runtime/session/service/capability state.
2. Select/establish canonical domain owners.
3. Define stable IDs and schema/version semantics.
4. Define `desired state` separately from `observed state`.
5. Define a canonical snapshot/projection consumed by surfaces.
6. Ensure deterministic ordering for collections.
7. Generate/derive JSON Schema/OpenAPI models from typed sources where existing tooling supports it.
8. Introduce migration/compatibility handling for legacy persisted state where required.
9. Add diagnostics model reuse rather than string errors scattered across consumers.

## Identity requirements

Prove behavior for:

- same repo, same worktree across processes,
- same project, different worktrees,
- two unrelated repos with same directory basename,
- branch changes,
- missing/changed origin where supported,
- cloned repository identity according to intended semantics.

Document exact identity derivation.

## Tests

Add focused tests for:

- serialization/versioning,
- deterministic snapshots,
- identity stability,
- duplicate ID rejection,
- schema generation/drift,
- legacy migration where applicable.

## Anti-goals

No parallel “Cockpit model”, “MCP model” and “CLI model” carrying the same facts manually.
No giant YAML registry introduced just to centralize duplication.
No hardcoded endpoint values in presentation layers.
