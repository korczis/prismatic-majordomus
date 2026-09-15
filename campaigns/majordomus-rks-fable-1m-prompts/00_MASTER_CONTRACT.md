# MASTER CONTRACT — Majordomus Repository Knowledge System (RKS)

You are working inside the real `prismatic-majordomus` repository. Treat this prompt as a hard architectural contract for all RKS work. Do not implement from memory or assumption: first inspect the actual repository, its AGENTS/CLAUDE guidance, `.ai/**`, `.majordomus/**`, Rust workspace, current CLI/API/MCP/Cockpit machinery, rules, doctrines, generated docs, tests, existing knowledge/ADR/session-context systems, worktree rules, and recent git history.

## Mission

Design and implement a first-class **Repository Knowledge System (RKS)** for Majordomus.

RKS is not an AI documentation generator. It is a continuously reconciled repository-native knowledge plane that:

1. discovers what a repository contains,
2. registers evidence,
3. derives typed claims and relationships,
4. distinguishes observed facts from declared statements, derived inference, and curated knowledge,
5. tracks provenance and confidence,
6. detects conflicts and stale knowledge,
7. computes change impact incrementally,
8. supports brownfield adoption via baselines,
9. reuses existing docs rather than cloning them,
10. exposes the same canonical model to CLI/API/OpenAPI/MCP/Cockpit/docs/completion,
11. supports agents with relevance-ranked context slices,
12. remains secure, deterministic-first, extensible, versioned, and testable.

## Non-negotiable Majordomus invariants

### A. Single canonical definition

Every RKS capability MUST have one canonical machine-readable definition or typed source of truth. Applicable projections must derive from it or be mechanically discoverable from it.

Never manually maintain parallel lists for:

- CLI commands
- API routes
- OpenAPI schema
- MCP resources/tools
- Cockpit menu entries
- docs navigation
- completion candidates
- capability registry entries

If a current Majordomus abstraction already solves this, extend it. Do not create a competing registry.

### B. Deterministic first, LLM second

Prefer deterministic extraction whenever the repository itself provides structured evidence:

- git
- workspace/package metadata
- AST/symbol information if existing tooling makes this practical
- manifests
- OpenAPI
- schemas
- imports/dependency graphs
- routes
- migrations
- CI definitions
- deployment manifests
- documented frontmatter

LLMs may enrich semantics, summarize, classify, propose links, detect semantic contradictions, or create explanations. They must not replace reliable structural parsers for facts that can be extracted deterministically.

### C. Evidence before machine-derived claims

Every generated/derived claim MUST reference evidence. No evidence means unknown/unverified, not invented certainty.

Required provenance classes:

- `observed`
- `declared`
- `derived`
- `curated`

Support explicit conflict/unverified state separately.

### D. No silent drift

If evidence changes, affected knowledge must become `possibly_stale`, `stale`, or `conflicted` according to deterministic impact rules. A node may not remain silently `current` after material source changes.

### E. Brownfield safety

Majordomus must integrate into an existing repository without punishing historical debt.

Implement baseline semantics:

> existing debt may be tolerated; new debt must not increase unless explicitly accepted.

`majordomus init` or the equivalent adoption workflow must never turn a legacy repository into hundreds of immediate mandatory failures by default.

### F. Reuse existing knowledge

Discover and register existing materials such as:

- README files
- `docs/**`
- architecture docs
- ADRs/RFCs
- runbooks
- OpenAPI/schema files
- contributing guides
- relevant structured code comments
- CI/deployment definitions

RKS must not automatically duplicate these into a competing `.ai/knowledge` reality. Track canonical ownership:

- `external`
- `majordomus`
- `hybrid`

Respect ownership when proposing or applying updates.

### G. Machine-readable core, Markdown as projection or source

Do not model the system primarily as `KnowledgeDocument`. Model knowledge as typed nodes/claims/evidence/relations. Markdown may be:

- an existing registered source,
- a human-readable projection,
- a curated knowledge artifact.

### H. Worktree awareness

RKS must obey Majordomus worktree doctrine. Knowledge state in a feature branch/worktree must be a branch-local view derived from canonical baseline + local delta. A feature branch must not mutate a global shared knowledge snapshot in a way that leaks across worktrees.

### I. Versioned schemas and migrations

All externally visible RKS data structures must have explicit schema/versioning. Provide a migration path before format evolution becomes painful.

### J. Secure ingestion

Never send repository evidence to remote model providers before policy evaluation and secret-safe preprocessing. Respect per-source and per-provider remote-processing policy.

### K. Explainability

A user or agent must be able to ask:

- why does Majordomus believe this?
- what evidence supports it?
- what changed?
- what depends on it?
- is it current?
- what is conflicting?

The model must support those answers directly.

## Core conceptual model

Use the following as conceptual guidance, not a demand to overwrite superior existing repo abstractions:

```text
Repository
   ↓
RepositoryInventory
   ↓
EvidenceRegistry
   ↓
Claims + Relationships
   ↓
KnowledgeGraph / KnowledgeRegistry
   ↓
Freshness / Conflict / Coverage / Baseline / Impact
   ↓
Projections and interfaces
```

Expected domain concepts include equivalents of:

```text
RepositoryKnowledge
KnowledgeNode
KnowledgeKind
Claim
Evidence
EvidenceRef
Relation
RelationRef
Provenance
Confidence
Freshness
Ownership
KnowledgeConflict
KnowledgeImpact
KnowledgeCoverage
KnowledgeBaseline
KnowledgeGap
Extractor / Discoverer
Reconciler
```

Do not force these names if the repository has a stronger naming scheme.

## Suggested knowledge kinds

Provide a typed, extensible taxonomy. Core kinds may include:

```text
repository
architecture
component
service
library
executable
interface
api
schema
data_store
data_model
concept
workflow
operation
deployment
dependency
security_boundary
decision
constraint
convention
risk
integration
```

Plugin-defined kinds must be schema-governed and discoverable without editing a central switch statement whenever practical.

## Freshness states

Support a clear model such as:

```text
current
possibly_stale
stale
conflicted
unverified
```

Age is not freshness. An old ADR can remain current; yesterday’s README can already be contradicted.

## Brownfield maturity modes

Integrate with Majordomus policy/gating machinery using modes comparable to:

```text
observe
warn
protect
strict
```

Semantics:

- observe: collect and report, no gate
- warn: report drift in developer UX/CI without blocking
- protect: prevent new debt above baseline
- strict: require full consistency according to configured policy

Do not hardcode policy behavior in multiple places. Model it centrally.

## CLI target UX

Aim for discoverable commands equivalent to:

```text
majordomus knowledge
majordomus knowledge bootstrap
majordomus knowledge scan
majordomus knowledge status
majordomus knowledge list
majordomus knowledge show
majordomus knowledge search
majordomus knowledge explain
majordomus knowledge graph
majordomus knowledge impact
majordomus knowledge gaps
majordomus knowledge coverage
majordomus knowledge stale
majordomus knowledge conflicts
majordomus knowledge reconcile
majordomus knowledge validate
majordomus knowledge baseline
majordomus knowledge check
```

Map to existing CLI naming conventions and metadata/macros. Do not maintain separate command docs manually.

## API / MCP / Cockpit targets

Expose the same canonical model through existing Majordomus machinery.

Representative API capabilities:

```text
GET knowledge collection
GET knowledge node
GET graph
GET search
GET coverage
GET gaps
GET stale
GET conflicts
GET impact
POST reconcile
POST validate
```

Representative MCP capabilities:

```text
knowledge_search
knowledge_get
knowledge_explain
knowledge_related
knowledge_impact
knowledge_conflicts
knowledge_gaps
```

Cockpit should offer:

```text
Overview
Explore
Graph
Changes / Impact
Coverage
Gaps
Conflicts
Sources
Schemas
Diagnostics
```

Every derived/declared claim shown in UI should make provenance and evidence inspectable.

## Testing contract

RKS changes are not complete without coverage across the levels appropriate to the change:

- unit tests
- schema tests
- property/invariant tests
- golden/snapshot tests where useful
- realistic brownfield repository fixtures
- CLI E2E
- API E2E
- MCP E2E
- Cockpit E2E where UI changes exist
- migration tests
- worktree isolation tests
- security policy tests
- incremental invalidation tests
- performance/regression benchmarks for hot paths

Create or extend realistic fixtures such as:

```text
tiny-rust
documented-rust
legacy-rust
conflicting-docs
monorepo
```

Prefer representative fixtures over toy examples that cannot catch architectural regressions.

## Documentation contract

RKS must be documented as both a developer subsystem and product capability. Documentation must be integrated with existing Majordomus docs generation/navigation rather than creating a second manual hierarchy.

Must explain:

- conceptual model
- brownfield adoption
- how provenance works
- how drift is detected
- how baselines work
- how to inspect evidence
- how to extend extractors/kinds
- CLI/API/MCP/Cockpit usage
- security / remote provider policy
- troubleshooting
- limitations and uncertainty

A prominent page/section should answer:

> How does Majordomus know what it knows?

## Product positioning guardrail

Do not present RKS primarily as “AI documentation generation.” Preserve the differentiated value proposition:

- documentation/knowledge that knows when it may be wrong
- provenance-backed repository knowledge
- continuous reconciliation
- brownfield-safe adoption
- agent-ready context
- explicit uncertainty and conflicts

## Execution discipline

For every phase:

1. Inspect existing repo structures and conventions before changing code.
2. Read the relevant AGENTS/CLAUDE instructions and nested README/frontmatter rules.
3. Search for existing generic abstractions before adding new ones.
4. State the implementation plan briefly in your working notes.
5. Implement the narrowest coherent vertical slice.
6. Run formatting/lint/tests relevant to touched areas.
7. Run higher-level gates if feasible.
8. Update docs/tests/schemas/interfaces in the same change.
9. Do not leave placeholder TODO architecture unless unavoidable; if unavoidable, explain why and create a tracked issue if the repo’s process supports it.
10. Summarize exactly what changed, what derives from what, tests run, and any remaining risk.

## Definition of done

A capability is only done when the canonical implementation and all applicable projections are integrated, tested, documented and discoverable. Avoid backend-only or docs-only partial realities unless a staged migration explicitly requires it.
