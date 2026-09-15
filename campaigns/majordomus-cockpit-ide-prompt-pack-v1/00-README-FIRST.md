# Majordomus Cockpit IDE / Control Plane Prompt Pack

## Mission

Transform the existing Majordomus Cockpit from a registry/execution projection into a **real development IDE, collaboration surface, runtime control plane, and monitoring environment** without violating the repository's core architecture:

```text
canonical declarations + repository state
              ↓
      typed canonical model
              ↓
       one executor/runtime
              ↓
 CLI · HTTP API · OpenAPI/Swagger · MCP · Cockpit · docs/GH Pages
```

The work MUST extend the existing system. It MUST NOT create a second frontend-owned application model, a duplicate API registry, hand-maintained navigation, hand-maintained Swagger, or a separate workflow catalogue.

The repository already states that:
- `AGENTS.md` and `CLAUDE.md` are generated bootstraps from canonical `.ai/` policy.
- agents must use Majordomus context/governance before substantive work.
- `bin/majordomus-mcp` starts/reuses the shared server.
- capability declarations under `apps/majordomus-cli/src/capability/builtin/` are canonical for transport projections.
- Cockpit is already a projection of the capability registry/index.
- the same executor backs transports.
- docs/generated and website registry views are projections.
- new capabilities must be propagated by generation rather than manual registration.

Treat those as existing contracts to inspect and preserve, not assumptions to overwrite.

## Required execution order

Run the prompts in numerical order.

Do not skip the audit prompts because "the feature probably already exists". This repository has enough moving pieces that claims without source/test/runtime evidence are worthless.

Suggested phase grouping:

1. `01` baseline and architecture audit
2. `02` contract and information architecture
3. `03` IDE workspace and navigation
4. `04` terminal / REPL / execution UX
5. `05` editing and development workflows
6. `06` observability and monitoring
7. `07` peers / collaboration / sessions / handovers
8. `08` planning / issues / milestones / git / worktrees
9. `09` governance / rules / doctrines / policies / preflight
10. `10` API / OpenAPI / Swagger / MCP / CLI parity
11. `11` docs / GH Pages / generated knowledge surfaces
12. `12` startup / automagic attachment / DX
13. `13` quality / tests / security / performance
14. `14` integration / cleanup / release / deployment
15. `15` adversarial acceptance and final proof

## Universal non-negotiables

Every prompt inherits these requirements:

- Inspect before changing.
- Load and follow `AGENTS.md`, `CLAUDE.md`, `.ai/README.md`, effective rules, doctrines, ADRs, workflows, knowledge, session context and relevant README chain.
- Use `majordomus context`, context resolution, task lifecycle, peer board and collision checks according to current repository rules.
- Discover current names/paths. Do not invent parallel structures because a prompt used conceptual names.
- Single source of truth.
- Typed contracts.
- Derived/inferred/data-driven output where appropriate.
- Deterministic ordering and stable IDs.
- No independently maintained transport inventories.
- No independently maintained UI catalogues.
- No hardcoded generated docs indexes where the registry can derive them.
- No separate Cockpit-only business logic if a capability/runtime model owns the semantics.
- CLI/API/MCP/Cockpit must represent the same capability where transport exposure permits.
- OpenAPI/Swagger must be generated from canonical transport/schema metadata.
- GH Pages/docs must be generated/derived from the same authoritative model where content is machine-derivable.
- Human prose may explain; it must not become a second inventory.
- Every modification must update tests and documentation in the same change.
- Every bug found during the work must be root-caused and regression-tested.
- Generated artifacts must pass drift checks.
- No stale open work/PR/worktree created by this pack may be left accidentally unintegrated.
- No success claim without exact evidence.

## Definition of Done

A phase is NOT done because code compiles.

It is done only when the phase has evidence for all applicable rows:

| Obligation | Required evidence |
|---|---|
| canonical owner | source file/type/declaration |
| CLI | invocation + output/contract test |
| API | route + contract/integration test |
| OpenAPI | generated operation/schema + drift test |
| Swagger | route renders and calls real API |
| MCP | tool/resource/prompt mapping + test |
| Cockpit | page/component/action + browser/e2e test |
| docs | canonical explanatory docs |
| GH Pages | generated/rendered discoverability |
| tests | unit + integration + regression |
| security | threat-specific checks where relevant |
| performance | benchmark or budget evidence |
| governance | rule/doctrine/policy and executable enforcement |
| automation | startup/discovery/attachment verified |
| deployment | deployment job/result if repository supports it |
| verification | post-deploy/runtime smoke check |
| git | clean intended diff, committed/pushed/integrated according to workflow |

## Final invariant

> A developer can enter the repository, automatically attach to Majordomus, open the Cockpit, understand the current project/session/governance/runtime state, inspect and operate capabilities, execute workflows and commands, edit or initiate development work, collaborate with peers, observe progress and failures, navigate issues/milestones/worktrees/git state, and verify completion. All of those surfaces are projections of canonical typed state and behavior rather than separately maintained products.
