---
id: github-workgraph-shared-contract
kind: prompt-contract
scope: repository-wide
target: claude-code-fable-1m
repository: ~/dev/prismatic-majordomus
---

# Shared Contract — Majordomus GitHub / Work Graph Campaign

This file is normative for every prompt in this pack. Read it before each phase.

## Mission

Turn GitHub integration from a collection of endpoints/commands/helpers into a **canonical, typed, auditable Work Graph control plane** where Majordomus can derive and reconcile the full path:

`milestone/outcome → issue/execution contract → dependency → branch → canonical worktree → sessions/handover → commits → pull request → checks/evidence → accepted completion → release`

and traverse it in reverse:

`commit/PR/check/release → issue → milestone/outcome`.

GitHub is an external projection and collaboration surface, **not an independent second source of truth**. Git is authoritative for Git facts. Majordomus canonical repository metadata is authoritative for Majordomus governance/work semantics. GitHub is authoritative only for facts that originate there, such as GitHub object IDs, remote issue/PR state, review/check-run observations, URLs, timestamps and other explicitly external facts. Every synchronized field MUST have an explicit authority/provenance policy.

## Non-negotiable Majordomus principles

Preserve and strengthen the repository's existing philosophy:

- single source of truth;
- infer, derive and discover before introducing manual registration;
- data-driven and schema-backed;
- typed core models;
- deterministic behavior;
- canonical IDs and stable references;
- explicit provenance and authority;
- idempotent generation and reconciliation;
- all relevant surfaces derived from canonical capabilities/models;
- no duplicated CLI/API/OpenAPI/MCP/Cockpit inventories;
- no handwritten OpenAPI registry when a capability can generate it;
- no frontend reconstruction of backend domain semantics;
- no manually synchronized documentation inventories;
- no hidden shell-script business logic if it belongs in the Rust application;
- tests and enforcement are part of the implementation, not cleanup work;
- docs and GitHub Pages are consumers of canonical information;
- legacy data in scope must be migrated, not merely tolerated forever;
- failures must be actionable and explain WHY, WHERE, EXPECTED, ACTUAL, FIX;
- no secrets in repository state, logs, snapshots, generated docs, fixtures or UI.

## Mandatory repository discovery

Before modifying anything:

1. locate the real repository root; do not assume the CWD;
2. read `AGENTS.md`;
3. read `.ai/README.md` and execute its discovery protocol;
4. resolve the effective context/rules for every path you will touch;
5. run the existing `majordomus context` / context-resolution workflow if available;
6. read `.ai/repo/workflows/task-lifecycle.md`;
7. inspect relevant rules, doctrines, policies, schemas, ADRs, workflows, skills and knowledge;
8. inspect `docs/CAPABILITIES.md`, `docs/MCP.md`, worktree docs and GitHub-related docs if present;
9. inspect canonical capability definitions under the actual repository path, currently expected near `apps/majordomus-cli/src/capability/builtin/`, but DISCOVER rather than assume;
10. inspect existing GitHub provider/adapter code, Git abstraction, worktree subsystem, issue/milestone model, generated OpenAPI/Swagger, MCP, Cockpit, docs/site, CI and hooks;
11. inspect existing use cases and coverage tooling;
12. inspect current test/golden/fixture conventions.

The public repository currently states that `AGENTS.md` is generated from `.ai/repo/policy.yaml`, that `.ai/` is normative, that capabilities generate transport projections, that use-case coverage is enforced, and that canonical worktree topology is already governed. Treat those as architectural constraints, then verify the local checkout because it may be newer.

## Worktree discipline

Do not implement on trunk/master if repository policy forbids it.

Use the repository's existing worktree tooling and canonical topology. Expected shape is:

`<repository>-wt/<branch>`

with the hierarchy of the branch preserved.

Create one campaign branch/worktree according to current rules, for example a semantically suitable branch such as:

`feature/github-work-graph`

Do NOT invent this literal name if existing issue/task naming policy dictates another form.

At the beginning of each phase:

- inspect current branch/worktree;
- verify it is canonical;
- inspect uncommitted changes;
- do not overwrite another agent's work;
- use the Majordomus announce/session mechanism if present;
- read handover/session context before continuing.

## Capability architecture

If an operation belongs to the Majordomus executable:

- add/change the canonical capability declaration;
- derive CLI/API/OpenAPI/MCP/docs/etc. via existing generators;
- do NOT manually edit a transport registry to make a missing projection appear;
- run generation and drift checks;
- update use cases and close coverage gaps.

If current architecture contradicts this, inspect ADRs and determine whether the architecture legitimately changed. Do not casually bypass the canonical capability mechanism.

## Canonical domain boundaries

The target should clearly separate:

### Canonical work semantics
Majordomus-owned concepts such as:
- work item / execution contract;
- outcome / milestone specification;
- dependency edges;
- acceptance criteria;
- evidence requirements;
- work state machine;
- derived readiness/blocking;
- completion semantics;
- policy/enforcement status.

### Git facts
Derived from Git:
- repository identity;
- branches;
- refs;
- commit graph;
- changed paths;
- worktrees;
- ancestry;
- merge base;
- commit associations where inferable.

### GitHub facts
Observed from GitHub:
- repository node/id;
- issue/milestone/PR external IDs and numbers;
- issue/PR state as observed remote state;
- labels/assignees/reviews/comments when in scope;
- check runs/workflow results;
- merge status;
- GitHub URLs/timestamps;
- remote mutations performed by Majordomus.

### Projection mapping
Stable mapping between canonical IDs and external GitHub identities.

Never conflate a GitHub issue number with a global/canonical identity.

## Required field-level authority model

Every synchronized field must have one of the following or an equivalent typed policy:

- `Canonical`: local Majordomus semantic source wins;
- `GitDerived`: computed from Git and not pushed as semantic truth;
- `GitHubObserved`: remote is authoritative observation;
- `Projected`: derived from canonical state and pushed to GitHub;
- `Mergeable`: both sides may edit; requires deterministic merge semantics;
- `ExternalOnly`: remote data is recorded but never projected back;
- `Computed`: derived from multiple facts and never directly edited.

Do not implement this as undocumented conditionals sprinkled through adapters.

## Reconciliation state model

Use or adapt a typed model with semantics equivalent to:

- `InSync`
- `LocalAhead`
- `RemoteAhead`
- `Drift`
- `Conflict`
- `MissingLocal`
- `MissingRemote`
- `Unmanaged`
- `Invalid`
- `Blocked`
- `NeedsVerification`

Names must match project style, but the distinctions must exist where meaningful.

Reconciliation MUST be:

- idempotent;
- deterministic;
- dry-runnable;
- explainable;
- safe under retries;
- resilient to pagination;
- resilient to partial failures;
- explicit about rate limiting;
- explicit about permission errors;
- able to avoid network mutation in validation/check mode.

## Completion semantics

A GitHub issue being closed or a PR being merged MUST NOT, by itself, imply canonical completion.

Canonical completion should be derived from the execution contract and required evidence, for example:

- dependency constraints satisfied;
- required implementation merged/reachable;
- tests/gates passed;
- required docs/schema/generated artifacts synchronized;
- acceptance criteria backed by evidence;
- no unresolved blocking drift/conflict.

A remote close can be an observed signal and may cause a reconciliation discrepancy.

## Zero-registration acceptance criterion

Adding a representative new canonical entity or capability must not require editing equivalent inventories in:

- CLI;
- REST;
- OpenAPI/Swagger;
- MCP;
- Cockpit;
- docs/GitHub Pages;
- completion;
- generated indexes.

If it does, fix the architecture instead of documenting the duplication.

## GitHub implementation quality

Prefer the repository's existing GitHub client/provider abstraction. If none exists or it is inadequate, build a provider boundary that is generic enough for future hosting/providers without turning this task into a speculative framework.

Requirements where applicable:

- authenticated REST/GraphQL through a typed client boundary;
- pagination;
- conditional requests / ETag if useful;
- rate-limit awareness;
- retry only for safe/transient cases;
- timeout;
- structured errors;
- permission diagnostics;
- deterministic fixtures;
- no token leakage;
- network calls absent from pure domain tests;
- mutation APIs idempotent or guarded;
- dry-run plans before mutation;
- external request IDs / relevant audit metadata when available.

Do not shell out to `gh` as the canonical business implementation if the Rust architecture already owns providers. A `gh` compatibility adapter may be acceptable only if existing doctrine prefers it and the domain stays independent.

## Tests required throughout

Do not postpone tests to the final phase.

Use the repository's conventions, but aim for:

- unit tests for domain/state transitions;
- property tests for graph/reconciliation invariants if appropriate;
- deterministic fixtures for Git/GitHub;
- contract tests around provider serialization;
- integration tests across canonical capability projections;
- end-to-end scenario tests;
- generated artifact drift tests;
- negative tests for malformed/ambiguous mappings;
- idempotence tests;
- offline tests by default;
- opt-in real GitHub tests only when credentials/environment safely permit.

## Documentation required throughout

Update canonical docs as implementation evolves.

Do not generate prose that claims behavior unsupported by tests.

Document:
- architecture;
- data model;
- authority/provenance;
- lifecycle/state machine;
- reconciliation;
- CLI/API/MCP/Cockpit usage;
- extension points;
- GitHub permissions/config;
- troubleshooting;
- migration;
- security;
- audit trail.

## Security and privacy

Never expose:
- GitHub tokens;
- Authorization headers;
- secrets from `.envrc.local`;
- private repository content in snapshots/logs beyond what the user explicitly expects;
- full remote payloads containing unnecessary personal data.

Redact and test redaction.

Define least-required GitHub permissions.

## Implementation loop for every phase

For every phase, execute:

`inspect → map existing architecture → design → implement → migrate in-scope legacy → generate → test → document → enforce → run use-case impact/coverage → run focused checks → inspect diff → commit according to repository lifecycle`

Never stop after producing an audit if the phase asks for implementation and implementation is possible.

Do not ask the user to choose obvious filenames/module names. Infer from repository conventions.

## Phase handover

At the end of every phase create/update the repository's normal session/handover artifact if the lifecycle uses one.

Include:
- objective;
- architectural decisions;
- files/components changed;
- tests run;
- generated artifacts;
- unresolved findings;
- next phase;
- exact invariants now guaranteed.

Do not invent a parallel handover format if one exists.

## Final quality bar

The final implementation must make the following queries mechanically answerable:

- Which canonical outcome/milestone does this issue serve?
- Which issues realize this outcome?
- Which issues are ready, blocked, executing, verifying or complete, and WHY?
- Which branch/worktree owns this issue?
- Which issue does this branch/worktree belong to?
- Which commits belong to the work item?
- Which PR realizes those commits?
- What checks/evidence satisfy each acceptance criterion?
- Why is an issue considered complete or incomplete?
- What GitHub data is drifted from canonical state?
- What would reconciliation change before it changes anything?
- Can we trace a merged PR/release backwards to outcome and acceptance evidence?
- Can an agent determine the next ready work item without reading manually curated lists?

If these answers require grep archaeology across six unrelated stores, the campaign is not complete.
