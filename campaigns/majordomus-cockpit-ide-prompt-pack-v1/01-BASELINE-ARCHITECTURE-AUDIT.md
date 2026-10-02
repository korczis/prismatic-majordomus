# Prompt 01 — Baseline, Repository Archaeology, and Gap Matrix

You are the lead architect implementing a repository-wide Cockpit IDE/control-plane evolution in `prismatic-majordomus`.

Do NOT begin feature coding.

## 1. Governance preflight

Before substantive work:

1. read root `README.md`;
2. read `AGENTS.md`;
3. read `CLAUDE.md`;
4. read `.ai/README.md`;
5. resolve effective rules and dependencies;
6. read task lifecycle and relevant workflows;
7. resolve context for every directory likely to change;
8. inspect current peer board;
9. announce intent and candidate paths;
10. run collision checking for new paths/identifiers;
11. inspect current worktree and branch topology;
12. inspect session context / handover state and use the repository's actual canonical mechanism.

If any of these mechanisms are broken, record the defect and root cause. Do not silently route around it.

## 2. Establish current canonical architecture

Prove from source, tests and runtime, not prose alone, how each of these is currently owned:

- capability declarations;
- module/root composition;
- capability registry;
- canonical executor;
- HTTP route projection;
- OpenAPI generation;
- Swagger UI;
- MCP projection;
- CLI projection/local exceptions;
- Cockpit route generation/navigation;
- execution model and live event channel;
- object/index discovery;
- graphs;
- health;
- performance counters;
- peer board;
- worktree topology;
- issue/milestone integration;
- sessions / contexts / handovers;
- rules/doctrines/policies;
- docs generation;
- GH Pages generation/deployment;
- `.envrc`/repo-entry bootstrap;
- `.mcp.json`;
- `.gemini/settings.json`;
- `.codex/config.toml`;
- `AGENTS.md` generation;
- `CLAUDE.md` generation.

For every item record:

```text
subject
canonical source
typed model
runtime owner
derived projections
generator
validator
tests
docs
known drift
missing surfaces
```

## 3. Run the system

Start it using the repository's supported path.

Verify at minimum:

- shared server startup/reuse;
- root redirect/content negotiation;
- `/cockpit`;
- `/openapi.json` or actual canonical path;
- `/swagger` or actual canonical path;
- `/api/v1/...`;
- MCP initialize/list/read/call;
- CLI capabilities/introspection;
- generated docs checks;
- health report;
- peer discovery;
- execution listing/live channel.

Record exact commands and observed outputs.

## 4. Audit Cockpit against "real IDE/control plane"

Build a gap matrix for these capabilities:

### Development
- capability explorer
- command palette
- generic execution runner
- streaming output
- terminal/REPL-like interaction
- source/file navigator
- source viewing
- safe editing workflow
- diff viewing
- test execution
- formatting/lint/check workflows
- build/release/deploy workflows
- task/workflow launch
- reusable workflow forms
- provider/model interaction where already architecturally supported

### Planning and SCM
- repo state
- branches
- worktrees
- commits
- diffs
- PRs
- issues
- milestones
- issue↔milestone↔code linkage
- release/changelog/version

### AI/runtime collaboration
- peers
- claims/scopes
- collisions
- current task/mandate
- session context
- handover
- workflow state
- model/provider surfaces
- executions
- cancellation
- retries
- live events/logs

### Governance
- effective rules
- doctrines
- policies
- ADRs
- context resolution
- provenance
- governance preflight
- violations and remediation commands
- quality gates
- "done" obligations

### Observability
- server health
- process/runtime state
- perf counters
- phase timings
- cache behavior
- request/execution history
- diagnostics
- CI status
- deploy state
- docs/site freshness
- generated artifact drift

### Docs/knowledge
- docs
- generated reference
- API reference
- Swagger
- knowledge
- ADR graph
- rules graph
- use-case graph
- searchable project knowledge
- links into GH Pages

For each mark one of:

- canonical + implemented + verified
- implemented but duplicated
- documented only
- planned
- partially implemented
- broken
- absent

## 5. Drift audit

Search for duplicated semantic definitions:

- route arrays;
- Swagger/OpenAPI handwritten paths;
- command catalogues;
- Cockpit menu arrays;
- provider arrays;
- workflow arrays;
- docs inventories;
- GH Pages navigation inventories;
- duplicated schema definitions;
- duplicated state/status enums;
- hardcoded counts;
- manual version strings;
- manual endpoint lists;
- frontend-only capability logic.

Find every place that violates the projection model.

## 6. Deliverable

Create/update a tracked implementation plan according to project conventions.

It must include:
- root causes;
- architecture diagram;
- gap matrix;
- dependency graph;
- phased migration;
- explicit non-goals;
- acceptance tests;
- risks;
- files/modules likely to change;
- generated files that must NOT be manually edited;
- current failures that must be fixed before the IDE expansion.

Do not implement broad UI changes yet.

End with exact evidence and the smallest safe next phase.
