# Prismatic Porting Policy

## Non-negotiable boundary

Prismatic is prior art, not infrastructure.

Majordomus MUST NOT depend directly on Prismatic at compile time, runtime, deployment time, or operationally.

Forbidden dependency forms include:

- imports from Prismatic modules/apps,
- package dependencies,
- repository dependencies,
- Git submodules,
- shared mutable storage,
- internal RPC/API coupling,
- undocumented API usage,
- requirement for Prismatic services to be running,
- deployment topology that assumes Prismatic exists.

## Allowed use of Prismatic

Prismatic may be consulted for:

- architecture patterns,
- lessons learned,
- algorithms,
- state models,
- UX ideas,
- storage techniques,
- provider abstraction patterns,
- session/context concepts,
- knowledge-base concepts,
- diagnostics,
- agent coordination ideas,
- observability patterns.

## Mandatory adaptation sequence

When considering a Prismatic-derived feature:

1. Identify the Majordomus requirement.
2. Identify the invariant the Prismatic implementation was trying to preserve.
3. Separate essential behavior from Prismatic-specific assumptions.
4. Decide whether the idea is actually appropriate for Majordomus.
5. Design a Majordomus-native interface.
6. Port or independently reimplement the minimal functionality.
7. Add Majordomus-native tests.
8. Add Majordomus-native documentation.
9. Verify no direct dependency remains.
10. Treat the resulting code as owned by Majordomus.

Do not mechanically copy architecture just because code exists elsewhere.

## Session-context and knowledge-base tooling

These are explicitly permitted sources of inspiration from Prismatic, but they must be adapted rather than linked.

For session/context functionality, determine first what Majordomus actually needs, such as:

- durable session identity,
- bounded context,
- contextual metadata,
- provenance,
- retrieval,
- lifecycle,
- compaction,
- reproducibility.

For knowledge-base functionality, determine what is actually needed, such as:

- canonical records,
- indexing,
- retrieval,
- provenance,
- versioning,
- invalidation,
- projection/rebuild semantics.

Do not inherit Prismatic's full stack unless every component is independently justified.

## Provenance

Design documentation may state that a concept was inspired by or adapted from Prismatic.

Example:

> Inspired by the Prismatic session-context model; independently implemented for Majordomus.

This is architectural provenance, not a dependency.

## Exit test

A Prismatic-inspired feature is acceptable only if a developer with no access to Prismatic can:

- build Majordomus,
- run its tests,
- understand its public/internal contracts from Majordomus docs,
- operate the feature,
- replace or refactor it without consulting Prismatic internals.
