# 06 — Provider-Neutral Agent Auto-Attach

Make supported coding agents attach automatically through thin adapters.

## Inspect actual provider integrations

Find existing Claude/Codex/Gemini/OpenAI/provider support and current hooks/config files. Reuse established plugin/adapter architecture.

## Adapter contract

Each supported adapter should map provider lifecycle into canonical operations:

```text
resolve repository/worktree
ensure control-plane
attach session
publish provider + capabilities
load context/handover
start presence heartbeat
subscribe/resync collaboration state
checkpoint/handover on lifecycle events where possible
release/expire claims on exit
```

Provider-specific mechanics may differ. Domain semantics must not.

## Bootstrap

Normal agent startup in an enabled repository must not require the human to type a separate `majordomus join`.

Manual attach/join commands may remain for diagnostics or unsupported environments.

## AGENTS.md / generated config

Use canonical instructions/generated fragments where current architecture supports them.

Avoid copied blocks that will drift between providers.

## Failure behavior

If Majordomus cannot attach:

- expose actionable diagnostics,
- do not silently pretend collaboration is active,
- degrade safely according to repository policy,
- never leak credentials.

## Tests

Build adapter contract tests using fakes/test harnesses where real provider startup is unsuitable for CI.

Then add at least one true end-to-end path for the provider integration that repository CI/local tooling can reliably execute.

Prove two provider adapters can coexist in one project and see common peer state.
