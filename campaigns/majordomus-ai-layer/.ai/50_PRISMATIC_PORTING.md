# Prismatic Porting Policy

## Non-negotiable boundary

Prismatic is prior art, not infrastructure.

Majordomus MUST NOT directly depend on Prismatic at compile time, runtime, deployment time, or operationally.

Forbidden forms include:

- imports from Prismatic modules/apps,
- package or repository dependencies,
- Git submodules,
- shared mutable runtime storage,
- internal RPC/API coupling,
- undocumented API usage,
- requirement for Prismatic services to run,
- deployment topology that assumes Prismatic exists.

## Allowed use

Prismatic may be consulted for:

- architecture patterns,
- lessons learned,
- algorithms,
- state models,
- UX ideas,
- storage techniques,
- session/context concepts,
- knowledge-base concepts,
- diagnostics,
- agent coordination ideas,
- provider abstractions,
- observability patterns.

## Required adaptation sequence

When functionality in Prismatic is relevant:

1. identify the Majordomus requirement,
2. identify the underlying invariant,
3. separate essential behavior from Prismatic-specific assumptions,
4. decide whether the concept actually belongs in Majordomus,
5. design a Majordomus-native interface,
6. port or independently reimplement the minimum needed functionality,
7. add Majordomus-native tests,
8. add Majordomus-native documentation,
9. verify that no direct dependency remains,
10. treat the result as Majordomus-owned code.

Session-context and knowledge-base tooling may be adapted under exactly this rule.

## Exit test

A Prismatic-inspired capability is acceptable only if a developer without access to Prismatic can build, test, understand, operate, and replace the Majordomus implementation.
