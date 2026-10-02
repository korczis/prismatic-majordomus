# Prompt 08: Final Convergence, Cleanup and Verification

Use MAXIMUM AVAILABLE CONTEXT one final time. Do not trust earlier prompts' assumptions. Reconstruct current master/worktree/session/issue/deployment reality.

Perform a repository-wide convergence audit and finish anything that prevents the architecture from being truly canonical and regression-proof.

Verify and fix:

## Architecture
- one provider registry
- one model registry/projection
- one canonical selection/router
- one session resolver/lifecycle
- one handover model/lifecycle
- one context discovery/compiler path
- one execution/invocation pipeline
- one safe diagnostics/provenance model

## No hidden duplication
Search for duplicate provider arrays, model arrays, routing tables, session loaders, handover parsers, frontend/backend ordering/config copies, provider-specific generic-domain conditionals and direct adapter invocations. Remove unjustified duplicates.

## Continuity proof
Run deterministic E2E proof for:

```
Codex -> Claude -> Gemini -> Ollama
```

using fake/test adapters where paid/live services are inappropriate, and prove canonical session/task/handover/context continuity through the invocation envelope and persisted state.

Run peer transfer proof and crash/restart proof.

## Cross-surface proof
Verify the same canonical provider/session state through CLI, API, OpenAPI, MCP, Cockpit backend payload and RepositoryEnvironment.

## Enforcement proof
Intentionally exercise negative paths and confirm gates catch:

- direct provider bypass
- missing session
- stale context
- broken handover chain
- duplicate provider ID
- secret exposure fixture
- invalid fallback cycle
- policy conflict

## Coverage/gates
Run the full canonical repository validation suite, coverage, docs drift, generated artifacts, CI-equivalent gates and E2E.

## Delivery
Inspect Git state, branches/worktrees/PRs/issues/milestones. Ensure work is landed on canonical branch and no relevant changes are stranded.

Verify GH Pages/public docs/runtime deployment as required.

## Final report
Produce evidence-backed final report containing:

- root causes fixed
- final canonical architecture
- migrations/deletions
- session/handover continuity behavior
- provider/model routing behavior
- enforcement rules and tests
- cross-surface integration
- exact commands + results
- coverage result
- issue/milestone completion state
- commit(s)/merge state
- deployed URLs/surfaces verified through project tooling
- only genuine remaining debt

Do not claim completion if any acceptance criterion is merely documented but not implemented/tested/verified.

Final invariant to prove:

> Majordomus owns continuity. Every stateful AI task automatically resolves its session, handover, governance and relevant context before execution; provider/model/peer transitions preserve that canonical state; all surfaces derive from the same backend truth; and regression gates prevent the system from returning to manual, duplicated or provider-specific memory.
