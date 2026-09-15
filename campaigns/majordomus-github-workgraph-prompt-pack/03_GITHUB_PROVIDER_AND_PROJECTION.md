---
id: github-workgraph-phase-03
phase: 3
depends_on: [github-workgraph-phase-02]
goal: GitHub provider and canonical external projection
---

# Phase 03 — GitHub Provider, Identity Mapping, and Projection Model

Read the shared contract and prior handovers.

## Objective

Create or repair a clean GitHub integration boundary that maps GitHub objects to/from the canonical Work Graph without infecting the domain model with transport details.

## First: inspect existing provider abstractions

If Majordomus already has provider/adaptor traits, HTTP client conventions, credential resolution, caching, diagnostics or schema types, use them.

Do not create:
- a second HTTP abstraction;
- a second credential registry;
- a GitHub-specific logging system;
- a parallel retry framework.

## External identity

Define a stable external reference type capable of distinguishing:
- provider (`github`);
- host if GitHub Enterprise is potentially relevant and existing architecture supports hosts;
- repository identity;
- object kind;
- external immutable/node ID where available;
- human number (`#123`) as display/reference, not sole identity;
- canonical URL.

Mapping must survive title changes and normal renames as far as GitHub identifiers permit.

## GitHub snapshot

Implement typed observation models for in-scope data:
- repository metadata needed for reconciliation;
- milestones;
- issues;
- PRs;
- labels/assignees only if current canonical semantics use them;
- reviews if completion policy needs them;
- check runs/workflow/check-suite results as evidence;
- merge state;
- relevant comments/body markers only if you deliberately need them.

Keep raw provider payloads at the adapter boundary. Convert to normalized typed observations.

## API mechanics

Implement robustly:
- pagination;
- timeout;
- typed status errors;
- auth/permission diagnostics;
- rate limit information;
- retry policy for safe transient failures only;
- no retries that duplicate mutations;
- user-agent/version according to repo conventions;
- redaction;
- deterministic JSON fixtures.

Choose REST/GraphQL based on actual needs and existing dependencies, not fashion. Mixed use is acceptable if cleanly encapsulated.

## Authentication/configuration

Use existing provider credential/config layer. Prefer environment/local secret sources that repository policy already supports.

Document exact least-privilege token/App permissions for:
- read-only status/diff;
- mutation/reconcile;
- checks/reviews if consumed.

Never store tokens in `.ai/repo`.

## Projection specification

Define typed projection rules from canonical work entities to GitHub.

For each projected field document:
- canonical source;
- external destination;
- serialization;
- authority;
- whether user editing on GitHub is accepted, ignored, imported, or conflicts;
- normalization rules.

Examples to decide explicitly:
- milestone title/description;
- issue title/body;
- labels;
- issue milestone assignment;
- dependency representation;
- acceptance criteria representation;
- canonical ID marker;
- state open/closed.

Avoid brittle hidden HTML comments unless needed. If markers are used, schema/version them, parse them defensively and keep human-facing content readable.

## Import/adoption model

A GitHub object may be:
- managed and mapped;
- discoverable candidate for adoption;
- unmanaged/external;
- ambiguous;
- invalid.

Do NOT auto-adopt arbitrary issues based only on similar titles.

## Read-only first

Before remote mutation, implement a read-only observation/diff path so the reconciler can later operate on typed facts.

## Tests

Use provider fixtures to test:
- pagination;
- missing permissions;
- rate-limit response;
- deleted/missing object;
- renamed titles;
- stale mapping;
- duplicate/ambiguous marker;
- redaction;
- serialization;
- deterministic normalization.

No real network in default test suite.

## Gate

A local canonical Work Graph and a GitHub fixture snapshot must be comparable through stable typed external mappings without any reconciliation code containing raw HTTP/JSON logic.

Update docs, use cases and handover.
