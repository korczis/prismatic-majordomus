---
id: github-workgraph-phase-02
phase: 2
depends_on: [github-workgraph-phase-01]
goal: canonical typed work graph
---

# Phase 02 — Canonical Typed Work Graph

Read the shared contract and Phase 01 handover.

## Objective

Implement or consolidate one typed domain model that can represent Majordomus work as a graph rather than disconnected milestone/issue/status files.

Do not begin with GitHub API calls. First make the internal semantics correct and testable offline.

## Required concepts

Adapt names to repository style, but the domain must be capable of representing:

### Work identity
- stable canonical work item ID;
- stable canonical outcome/milestone ID;
- repository identity;
- optional external references;
- human display title separate from identity.

### Outcome / milestone
- purpose/outcome;
- acceptance/exit criteria where the project models them;
- contained/related work items;
- dependency/precondition relationships where appropriate;
- derived progress from child/evidence state rather than manually maintained percentages.

### Execution contract / issue
- objective;
- scope;
- dependencies;
- acceptance criteria;
- verification/evidence requirements;
- ownership/claim semantics if supported;
- expected paths/components when existing task lifecycle uses scope;
- external projections;
- derived lifecycle state.

### Edges
At minimum support typed relationships equivalent to:
- contains;
- depends_on;
- blocks;
- supersedes/replaces if existing domain needs it;
- realized_by;
- proposed_by;
- verified_by;
- belongs_to/serves outcome.

Avoid a generic `String -> String -> String` graph that destroys type guarantees.

## Status must be derived

Define a state machine based on facts, not hand-maintained booleans. Exact labels should fit the repo, but distinguish meanings such as:

- planned;
- blocked;
- ready;
- claimed/executing;
- changes-present;
- review/proposed;
- verifying;
- accepted/completed;
- conflicted/drifted;
- invalid.

Specify transition predicates. Example:

`ready = all required dependencies accepted AND contract valid AND not already claimed/completed`

`completed = acceptance contract satisfied by evidence AND required integration state reachable/merged according to policy`

Do not let GitHub `closed` short-circuit this logic.

## Acceptance criteria and evidence

Make criteria machine-addressable. Avoid only free-form Markdown if the project already has typed/schema infrastructure.

A criterion should be able to link to evidence such as:
- test result;
- capability/use-case execution;
- CI check;
- commit/tree state;
- generated artifact drift check;
- docs check;
- manual approval if policy explicitly allows it.

Separate "criterion text" from "evidence that satisfies it".

## Graph invariants

Enforce and test:
- canonical IDs unique;
- referenced nodes exist or have a typed unresolved/external representation;
- no invalid self-dependency;
- dependency cycles are detected with actionable cycle path;
- deterministic traversal;
- deterministic serialization;
- stable topological/readiness derivation;
- adding an unrelated item does not change existing IDs;
- progress derives from actual graph/evidence;
- invalid data cannot silently become `ready`.

## Provenance hooks

Prepare domain structures for field/entity provenance without baking GitHub transport details into the core.

Provenance should be able to answer:
- source kind;
- source identity;
- observed/derived time if relevant;
- authority class;
- derivation explanation.

## Schema

Integrate with existing schema generation. No parallel manually edited JSON Schema if typed generation exists.

If repository metadata uses YAML/Markdown front matter, define the canonical schema and migrate representative fixtures.

## Queries

Expose internal domain/query functions sufficient to answer:
- get work item;
- list by milestone/outcome;
- dependencies/dependents;
- readiness explanation;
- blockers;
- evidence summary;
- completion explanation;
- graph traversal from canonical ID.

Do not prematurely duplicate transport-specific DTOs unless boundary semantics require them.

## Tests

Include:
- DAG;
- diamond dependency graph;
- cycle;
- missing dependency;
- multiple milestones/outcomes if allowed;
- satisfied/unsatisfied criteria;
- invalid evidence;
- deterministic order;
- progress derivation;
- completion does not follow remote close;
- no network dependency.

Property-test DAG/readiness invariants if existing toolchain makes this reasonable.

## Migration discipline

If existing issue/milestone representations exist:
- write adapters/migrations into the canonical model;
- delete redundant domain representations when safely replaced;
- preserve compatibility only at deliberate boundaries.

## Gate

Phase passes only when a purely local fixture can answer, with explanations:

`milestone → issues → dependencies → readiness → acceptance criteria → evidence → completion`

and all of those answers are produced from one canonical graph.

Update use cases, docs, handover and checks.
