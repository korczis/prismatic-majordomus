# Majordomus Intent-Driven UI — Implementation Prompt Pack

**Audience:** Claude Code Opus/Fable or another repo-aware coding agent.  
**Mission:** make `majordomus ui` the canonical, derived, interactive, real-time development control plane.  
**Repository:** `https://github.com/korczis/prismatic-majordomus`  
**Scope:** deeply integrate existing `.ai/`, CLI, Rust capability registry, API, MCP, Cockpit, execution plane, GitHub workflows, mesh and docs. No parallel universe.

## How to execute

Start with [`00-ORCHESTRATOR.md`](00-ORCHESTRATOR.md). It is the only initial prompt required. It delegates the numbered phases in order, with verification and a checkpoint after each phase. The phases are independent, copy-paste-ready instructions, but must run against the actual checkout and its *current* `.ai/` contracts. The orchestrator's gates always outrank optimistic completion claims.

| Phase | Prompt | Deliverable |
|---|---|---|
| 00 | [Orchestrator](00-ORCHESTRATOR.md) | Repo-grounded plan, controlled implementation loop |
| 01 | [Audit & contract](01-AUDIT-CONTRACT.md) | Capability/ownership map, gap assessment, design decisions |
| 02 | [Typed action plane](02-ACTION-PLANE.md) | Secure, auditable, schema-driven command execution |
| 03 | [Intent vertical](03-INTENT-VERTICAL.md) | Intent → gap → critique → plan → work → evidence |
| 04 | [UI projections](04-UI-PROJECTIONS.md) | Mobile-first interlinked `majordomus ui` |
| 05 | [Live & mesh](05-LIVE-MESH.md) | Resume-safe events, peer/task interaction |
| 06 | [GitHub & delivery](06-GITHUB-DELIVERY.md) | Issues, milestones, PRs, CI, version, release, deploy |
| 07 | [Enforcement & tests](07-ENFORCEMENT-TESTS.md) | `.ai/` doctrines, executable gates, dogfooding |
| 08 | [Rollout & closure](08-ROLLOUT-CLOSURE.md) | Verified staged release and evidence report |

## Fundamental constraints

- **Repo reality beats this prompt.** Inspect before editing. Referenced symbols, flags, endpoints and layouts are *discoverable candidates*, never assumed truths.
- **Canonical ownership.** `.ai/repo/` is canonical for tracked AI governance where the current repo contract says so; `.ai/local/` is local. Generated views never become an alternative writable source of truth.
- **One semantic mutation path.** CLI, MCP, API and UI invoke the same authorized typed operation and record the same outcome. Adapters may differ; policy must not.
- **Reuse before introduce.** Extend existing capability, execution, policy, projection, route, asset and event machinery. No independent CRUD app or JS-backed state model.
- **Honest autonomy.** Authorization, scope, effect classification, allowlisted tools, human approvals when mandated, idempotency, audit, remote-mutation policy and rollback must actually be enforced. Never bypass branch protections or CI.
- **Evidence first.** No claims of finished, released or deployed work without independently checked artifacts and post-deploy smoke evidence.
- **Incremental integration.** Produce a working vertical before expanding views. All phases may be split into small, reviewable commits/PRs when the existing workflow dictates.

## Research anchors (not substitutes for checkout inspection)

- [GitHub repository](https://github.com/korczis/prismatic-majordomus)
- [Documentation index](https://majordomus.dev/docs/)
- [Intent Cockpit guarantee](https://majordomus.dev/guarantees/intent-cockpit-pages/)
- Review current `docs/CAPABILITIES.md`, `docs/EXECUTIONS.md`, `docs/COCKPIT.md`, `docs/WEB.md`, `docs/ENVIRONMENT.md` and the applicable `.ai/` tree *from the checkout*.

## Expected final user experience

```
majordomus ui
  → select project
  → create/inspect intent and acceptance criteria
  → infer gap, critique and plan; review/edit canonical records
  → derive milestones, issues and dependencies
  → authorize execution of ready work via existing capabilities
  → watch workers, nodes, logs, events, evidence live
  → inspect changes, CI, PR readiness, merge policy
  → version, release, deploy and verify when policy allows
  → intent satisfaction calculated from trustworthy evidence
```

`majordomus ui` must not need manually synchronized frontend inventories. New canonical capabilities, object types and links should appear via generic projections, schema-driven actions and typed relationship metadata, subject to the current registry's actual extension mechanism.
