# Majordomus Cockpit Runtime Orchestration — Prompt Pack

## Goal

Implement the runtime orchestration system as a first-class, inspectable, controllable Cockpit capability, backed by the same canonical typed state exposed through CLI/API/OpenAPI/MCP.

This pack covers:

1. repository discovery and gap analysis,
2. canonical orchestration domain/state,
3. context compiler + provenance + token budgeting,
4. model routing + switching + escalation + cross-model handover,
5. Cockpit UX and live execution observability,
6. cross-surface CLI/API/OpenAPI/MCP integration,
7. governance, doctrines, validation and anti-drift enforcement,
8. legacy migration, tests, docs and deployment,
9. adversarial final review and proof of zero-registration extensibility.

## How to use it

Run the prompts sequentially in the same feature worktree/session where possible.

Recommended order:

```text
00 COMMON CONTRACT
02 DISCOVERY / GAP AUDIT
03 DOMAIN / SNAPSHOT
04 CONTEXT COMPILER
05 MODEL ROUTER / HANDOVER
06 COCKPIT UX
07 CROSS-SURFACE INTEGRATION
08 GOVERNANCE / ENFORCEMENT
09 MIGRATION / E2E / DOCS / DEPLOY
10 FINAL OPPOSITION / ACCEPTANCE
```

`11-MASTER-ONE-SHOT.md` is the combined "do the whole thing" prompt for a 1M-context agent. Prefer the staged prompts when you want reviewable commits and cleaner failure isolation.

## Session protocol

At the beginning of every phase, consume the previous phase's handover and verify the repository state. Do not blindly trust prose handovers if Git/tests disagree.

At the end of every phase, emit:

```text
PHASE HANDOVER
- objective completed
- architecture introduced/reused
- canonical types/registries changed
- files changed by purpose
- tests added
- commands run + outcomes
- generated artifacts updated
- governance/docs updated
- known pre-existing failures
- remaining risks
- exact next entry point
- commit(s), if repository workflow requires them
```

The handover itself should be stored using the repository's existing session/handover mechanism if one exists.

## Commit discipline

Follow the repository's branch/worktree/commit/issue/milestone doctrine. Do not invent a parallel workflow.

Prefer coherent commits per architectural phase. Never hide failing tests by squashing evidence away.

## Definition of done

The feature is not done merely when a page renders.

It is done only when:

- orchestration state is canonical, typed and serializable,
- Cockpit renders that state without owning duplicated semantics,
- context compilation is inspectable with provenance,
- routing decisions are explainable,
- model switching preserves execution state,
- escalation can be triggered by epistemic uncertainty, not only API errors,
- cost/token/latency/confidence data are observable where available,
- secrets remain redacted,
- CLI/API/OpenAPI/MCP expose the same model,
- stream/event behavior is tested,
- governance prevents reintroducing frontend-only or provider-specific drift,
- docs and generated pages are current,
- deployment/verification follows repository policy,
- a newly added provider/model/router criterion or orchestration phase propagates through relevant consumers without manual registration in each surface.
