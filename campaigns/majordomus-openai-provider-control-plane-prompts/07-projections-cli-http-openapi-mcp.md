# Global execution contract

Before changing code, perform the repository's own bootstrap exactly as the live `AGENTS.md` and `.ai/README.md` require.

Non-negotiable constraints:

1. Read and obey live rules and their dependencies. Do not trust filenames alone.
2. Resolve path-specific context before changing files.
3. Use the canonical branch/worktree topology. Do not work on a feature branch in the trunk checkout.
4. Every substantial change must be attached to the repository's issue/milestone lifecycle if that lifecycle is enabled by the live policy.
5. Existing architecture outranks this prompt. Adapt this requested intent to the repository's current canonical mechanisms rather than creating parallel machinery.
6. No manual duplicate registration of a provider, model, capability, route, MCP tool, OpenAPI operation, Cockpit page, doc entry, benchmark target or skill projection where it can be derived.
7. Do not silently weaken a rule or bypass a gate to make the change pass.
8. Do not expose secrets or raw credentials in any output, diagnostics, snapshots, fixtures, generated docs or browser state.
9. New user-visible claims require executable evidence through the project's use-case/test system.
10. Update docs/ADRs/rules in the same change when semantics change.
11. Prefer strongly typed Rust models plus `schemars`/existing schema machinery over untyped JSON maps.
12. Preserve deterministic generation.
13. No network activity on repository-open or other hot paths unless an explicit operation requests it.
14. Provider discovery failure must degrade/refuse according to explicit semantics. Never fabricate model capabilities.
15. Treat external provider data as untrusted input.
16. Do not persist raw provider responses by default merely because they are available.
17. Keep provider-specific protocol vocabulary below the adapter boundary unless a provider-specific diagnostic explicitly needs to surface it.
18. Finish with focused tests, cross-projection parity tests, generation checks, use-case coverage, docs checks, lints and the repository's normal `finish` evidence.


# Mission 07 — Project provider/run/idea functionality through CLI, HTTP/OpenAPI, Swagger and MCP

At this point the canonical services should exist. Now wire them through the existing capability registry only.

## Critical invariant

Do not create separate semantics for any transport.

A provider operation must have ONE handler/application path. CLI, HTTP and MCP dispatch through it.

## CLI

Create ergonomic native command paths derived from canonical exposure declarations.

Expected families, adapted to existing grammar:
- providers list/describe/health;
- models list/describe/select;
- run execute/show/cancel if cancellation exists;
- hosts list/describe/health;
- ideas list/show/capture/promote/import if Mission 06 introduced ideas.

Human output:
- concise;
- stable;
- no secrets.

Machine output:
- existing JSON mode/conventions;
- versioned typed shape if repo requires it.

## HTTP / OpenAPI / Swagger

Use existing route projection.

Requirements:
- canonical schemas only;
- operationId = canonical id per current architecture;
- no hand-authored OpenAPI path;
- mutation commands use correct POST semantics;
- status/error semantics consistent across transports;
- documented security implications of binding beyond loopback if mutating capabilities now exist;
- if repository mutation is exposed over HTTP, revisit the old read-only/no-auth assumptions in ADR 0002. This is not optional.

This is a major security boundary:
the moment remote mutation exists, "loopback and read-only" is no longer an accurate premise.

Make an explicit decision:
- keep mutation MCP/CLI-only initially; or
- add proper HTTP authorization/CSRF/origin protections appropriate to actual exposure.

Do not casually expose write APIs because Swagger can render a button.

## MCP

Project canonical tools using the existing MCP surface generator.

Add accurate MCP annotations:
- read-only for queries;
- not read-only for mutations;
- destructive/idempotent semantics where supported.

Tool naming must derive from capability metadata.

## Cross-protocol parity tests

For representative operations prove:
- CLI JSON == canonical output;
- HTTP == canonical output;
- MCP == canonical output;
- error/refusal semantics equivalent;
- generated OpenAPI matches route schemas;
- generated docs are current.

## Generate/check

Run the repository generator and ensure no manually maintained inventory was added.
