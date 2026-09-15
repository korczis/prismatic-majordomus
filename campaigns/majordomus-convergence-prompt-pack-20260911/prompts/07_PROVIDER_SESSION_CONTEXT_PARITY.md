# Stage 07 — Provider Session/Capture/Context Parity

Generalize lifecycle automation from a Claude-specific hook implementation into a canonical provider adapter architecture.

## Starting point

The audited snapshot had provider metadata for multiple providers but automatic capture/lifecycle adapter tables only for `claude-code`. Re-verify current state.

## Target architecture

```text
provider registry
    ↓
provider capabilities
(prompt capture, session start/end, compact/checkpoint, identity, etc.)
    ↓
adapter implementation
    ↓
canonical lifecycle event
    ↓
canonical session/context/handover model
    ↓
CLI / doctor / MCP / Cockpit / docs
```

## Tasks

1. Enumerate configured providers and the events/integration mechanisms they actually expose.
2. Define typed provider lifecycle capability metadata. Do not claim an unavailable host event exists.
3. Replace static shell-only adapter knowledge with canonical adapter registration/discovery owned by the proper layer.
4. Implement adapters for Claude Code, Codex and Gemini to the maximum reliable semantics supported by each environment in the repo.
5. For providers without a native event, implement an explicit degraded/fallback strategy if safe, and expose that status.
6. Normalize all provider events into one typed internal lifecycle protocol.
7. Ensure session identity/provider identity cannot collide.
8. Make session context creation, loading, checkpointing, handover and close idempotent/crash-safe.
9. Ensure provider adapters cannot bypass governance preflight/context discovery.
10. Add provider-specific fixtures and cross-provider contract tests.
11. Surface provider capability/status in `doctor`, structured CLI, API/MCP and Cockpit.
12. Generate docs/provider matrix from canonical metadata.

## Context quality

The lifecycle pipeline should automatically connect to the existing context compiler/preflight model: relevant session contexts, knowledge, ADRs, git/source/tests, issues/milestones, rules/doctrines. Do not dump the entire repository into prompts. Preserve relevance ranking/token budgeting/provenance architecture if present.

## Acceptance

- There is one canonical provider lifecycle model.
- Claude/Codex/Gemini status is explicit and tested.
- Unsupported semantics are visible, not silent.
- Session contexts are automatically created/refreshed/closed at available lifecycle boundaries.
- Adding a provider adapter does not require editing every consumer.
- Docs/Cockpit/provider diagnostics derive from canonical adapter metadata.
