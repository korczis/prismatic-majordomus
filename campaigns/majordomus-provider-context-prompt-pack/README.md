# Majordomus Provider + Session Continuity Prompt Pack

This pack is intended for Codex operating inside `prismatic-majordomus`.

## Goal

Implement provider switching/routing as a first-class Majordomus subsystem that automatically and transparently works with session contexts and handovers out of the box, with strong regression prevention, full cross-surface integration, documentation, deployment and verification.

## Operating principle

Every prompt in this pack MUST use maximum available context before acting.

"Maximum context" means the agent must reconstruct the real current state from all applicable sources before making changes, including at minimum:

- repository source code and tests
- AGENTS.md / CLAUDE.md / Codex instructions
- `.ai/**` and `.majordomus/**`
- rules, doctrines, policies, schemas, ADRs, skills, profiles, workflows
- current session context and relevant prior sessions
- handovers and predecessor/successor lineage
- current Git/worktree/branch/HEAD/dirty state
- issues and milestones, including active development state
- current CLI/API/OpenAPI/MCP/Cockpit architecture
- provider/model/runtime implementation and configuration
- generated docs and GH Pages source
- CI/CD, gates, coverage and deployment tooling
- relevant Git history when it helps explain legacy implementation or regressions

The agent MUST prefer canonical machine-readable sources over prose when they conflict, and must report detected drift rather than silently guessing.

## Hard invariants

1. Majordomus owns continuity. Providers are disposable execution engines.
2. Stateful repository work must resolve/create a Majordomus session automatically.
3. Relevant handovers must be discovered and loaded automatically.
4. Governance + context must be hydrated before provider/model routing.
5. Provider switching/fallback/peer transfer must preserve canonical session continuity.
6. Provider-native threads/conversations are metadata, not canonical memory.
7. Provider/model registries are canonical and shared across CLI/API/OpenAPI/MCP/Cockpit/docs.
8. No consumer-specific provider/model inventories.
9. No secrets in safe state, logs, docs, sessions, API, MCP or Cockpit payloads.
10. New/modified behavior must be regression-tested, validated and enforced.
11. Documentation and GH Pages must reflect deployed reality.
12. Work is not complete until landed, deployed and verified according to repository conventions.

## Recommended execution order

Run prompts in order unless the repository's own issue/milestone/session orchestration already decomposes the work more precisely:

1. `01-forensic-audit.md`
2. `02-canonical-provider-architecture.md`
3. `03-session-handover-continuity.md`
4. `04-routing-fallback-governance.md`
5. `05-cross-surface-integration.md`
6. `06-testing-enforcement-regression.md`
7. `07-docs-gh-pages-deployment.md`
8. `08-final-convergence-verification.md`

Each prompt should be executed from a fresh/continued Codex session that first resolves current Majordomus session + handover state. Do not assume previous prompt output is current; rediscover and reconcile current repository state on every run.

## Completion standard

The pack is complete only when the implementation proves this flow end to end:

```
repository/task
  -> session resolution/creation
  -> handover resolution
  -> governance preflight
  -> context discovery/compiler
  -> task requirements
  -> provider/model router
  -> invocation
  -> provenance/events
  -> session/handover update
  -> validation
  -> docs/deploy verification
```

and switching among Codex / Claude / Gemini / Ollama, or transferring between peers, does not lose canonical project context.
