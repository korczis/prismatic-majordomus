# Phase 10: Expose, Document, and Cross-Link Worktree State Without Duplication

## Goal

Make canonical worktree lifecycle observable through existing Majordomus surfaces where operationally useful.

This phase is not permission to build a vanity dashboard.

## 1. CLI

Ensure canonical worktree state and diagnostics are accessible through existing CLI architecture.

Human output should be concise and deterministic.

Structured output should come from the same typed model.

## 2. API / OpenAPI

If the repository already exposes equivalent operational state through REST/API:

- expose worktree inventory/status through the canonical backend model,
- derive OpenAPI schemas/descriptions from canonical types/routes,
- do not hand-maintain duplicate JSON schemas,
- keep destructive operations properly explicit and safe.

Do not create public destructive HTTP endpoints if that conflicts with repository security model.

## 3. MCP

If MCP is a first-class control surface, expose useful read/plan operations from the same backend.

Potentially useful operations:

- inspect topology,
- get diagnostics,
- create canonical worktree,
- generate cleanup plan.

Destructive execution should follow existing authorization/safety patterns and should not become easier to trigger accidentally just because an LLM can call it.

## 4. Cockpit

If Cockpit already serves repository operational state, add a worktree view using canonical API data.

Useful UX:

- branch,
- path,
- clean/dirty status,
- ahead/behind,
- upstream,
- canonical-path diagnostic,
- merged/stale candidates,
- dependencies if already modeled,
- dry-run cleanup plan,
- links to relevant session/handover/issues if canonical relationships exist.

Do not reimplement Git parsing in JavaScript.

## 5. Documentation

Update canonical docs and GitHub Pages/site material to explain:

- worktree topology convention,
- why sibling `-wt` exists,
- branch/path mapping,
- how to create a worktree,
- how to inspect status,
- how to safely merge/rebase,
- how cleanup planning works,
- how preservation works,
- how to recover from interrupted operations,
- how validation/gates enforce policy.

Generate inventories/examples where possible instead of maintaining stale command lists manually.

## 6. AGENTS / agent bootstrap

Where repository policy allows, ensure agents are pointed to canonical worktree tooling and lifecycle rules automatically.

Do not paste a giant duplicate manual into `AGENTS.md`.

Link/derive from canonical documentation or generated guidance.

## 7. Completion / discoverability

If CLI/Just completion is generated at runtime, ensure worktree commands participate automatically from canonical command metadata.

No hand-maintained completion list.

## Acceptance criteria

- operational worktree state is discoverable where it belongs,
- every surface consumes the same canonical model,
- docs reflect actual commands and policy,
- no frontend/API/docs duplicate registry exists,
- unsafe destructive operations are not accidentally broadened.
