# Phase 1 — Repository Audit, Fit Analysis, and RKS Architecture

Use `00_MASTER_CONTRACT.md` as binding context.

Your task is to produce the implementation architecture for RKS by deeply inspecting the actual Majordomus repository, then implement only the minimal foundational scaffolding that is unambiguously required and low-risk.

## Objectives

1. Map the current repository architecture relevant to RKS.
2. Identify existing primitives that RKS should extend rather than duplicate.
3. Locate existing knowledge, ADR, session-context, handover, schema/frontmatter, provider, CLI, API, OpenAPI, MCP, Cockpit, docs, rule/doctrine and capability-discovery mechanisms.
4. Identify how worktree state, branch identity, git diff, repository metadata and generated docs currently work.
5. Produce a concrete architecture decision for RKS that fits those systems.
6. Implement the smallest foundational module/package structure and typed capability hook necessary for subsequent phases, only if justified by the audit.
7. Capture the decision in the repo’s existing ADR/knowledge system, following its own standards.

## Required repository inspection

Read and analyze at minimum, if present:

- root `AGENTS.md`, `CLAUDE.md`, `README.md`
- nested `AGENTS.md` / README files governing touched directories
- `.ai/**`
- `.majordomus/**`
- Rust workspace manifests and `apps/majordomus-cli/**`
- current CLI command registry/macros
- server/router/OpenAPI generation
- MCP implementation and registry
- Cockpit structure and frontend stack
- docs/Zola/GitHub Pages generation
- schemas/frontmatter validation
- rules and doctrines
- ADR generation/storage
- session context / handover / prompt history
- just integration/completion machinery
- existing provider abstractions
- git/worktree tooling and enforcement
- test harnesses and fixture patterns
- recent git history for related subsystems

Search broadly. Do not assume file paths from earlier conversation descriptions still match reality.

## Deliverable: architecture map

Produce a concise but technically precise architecture map in repo docs/ADR form, including:

### Existing reusable primitives

For each relevant existing primitive, state:

- canonical implementation location
- its source of truth
- how it is discovered/generated
- where it is projected
- whether RKS should extend, compose, or avoid it

### Proposed RKS module boundaries

Identify boundaries such as:

```text
inventory/discovery
knowledge domain model
schema/versioning
extractors
evidence registry
claim/relationship derivation
freshness/impact
conflict detection
baseline/policy
reconciliation
query/search/context assembly
interface adapters
```

Map them to the repository’s actual crate/module architecture.

### Data flow

Document the authoritative flow from repository evidence to projections. Explicitly show where generated Markdown, API payloads and UI views sit relative to the canonical model.

### Storage strategy

Decide and justify:

- what is recomputed vs persisted
- what is tracked in git vs cached/ignored
- where branch/worktree-local state lives
- where brownfield baseline lives
- how schema versions are encoded
- how deterministic fingerprints are stored

Prefer minimal durable state and deterministic reconstruction when practical.

### Extension strategy

Identify the best existing Majordomus mechanism for generic extractor/capability registration. The architecture must not require editing a central list every time a new language or evidence extractor is added, unless a deliberate compile-time registry is clearly superior and mechanically generated.

### Migration strategy

Document compatibility expectations and how future schema migrations will work.

## Foundational implementation

Implement only infrastructure that is clearly needed regardless of later details. Examples may include:

- module skeletons
- shared IDs/newtypes
- a capability enum/type if the repo already has such a pattern
- schema version constants
- test fixture root

Do **not** prematurely implement extractors, UI, or dozens of types before the repo fit is understood.

## Tests

Add tests for any new foundational code. Ensure no existing tests regress.

## Acceptance criteria

- Architecture is grounded in real repository structures.
- No parallel/manual registry is introduced.
- Existing abstractions are reused where possible.
- Storage/worktree/versioning decisions are explicit.
- ADR/knowledge documentation is added according to repo doctrine.
- Any foundational code is minimal, tested, and ready for Phase 2.
- Relevant repo checks pass.

## Final response

Report:

1. architecture decision,
2. existing mechanisms reused,
3. files changed,
4. tests/checks run,
5. unresolved risks or contradictions discovered in the repo.
