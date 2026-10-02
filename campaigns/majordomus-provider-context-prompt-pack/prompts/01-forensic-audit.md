# Prompt 01: Forensic Audit and Current-State Reconstruction

You are Codex working inside `prismatic-majordomus`.

Use MAXIMUM AVAILABLE CONTEXT before acting. Follow the pack `INSTRUCTIONS.md` preflight completely.

Your task is to reconstruct the real current state of provider/model switching, routing, sessions, handovers and context continuity across the repository. Do not implement architecture until you have mapped existing reality.

Audit all relevant sources:

- provider/model abstractions and adapters
- Claude / Anthropic
- Codex / OpenAI
- Gemini / Google
- Ollama/local models
- provider CLI/subprocess adapters
- HTTP/SDK/MCP transports
- model discovery
- configuration and credential handling
- provider/model selection and defaults
- fallback/retry logic
- profiles/workflows/skills
- session contexts
- handovers
- context discovery/compiler
- peer/mesh/runtime discovery
- CLI
- API/OpenAPI/Swagger
- MCP
- Cockpit
- RepositoryEnvironment/banner
- rules/doctrines/policies
- schemas/ADRs
- tests/coverage/gates
- docs/GH Pages/landing claims
- issues/milestones/session histories
- relevant Git history

Produce an internal evidence-backed map:

```
concept
canonical/current source
existing type/schema
consumers
legacy duplication
current tests
drift/defect
migration action
```

Explicitly identify:

- duplicate provider/model inventories
- provider-specific state used as canonical memory
- manual session loading
- manual handover loading
- direct provider invocation bypassing session/context orchestration
- stale/incomplete session schemas
- broken session/handover lifecycle
- provider switches that lose context
- global mutable provider state
- surface-specific routing logic
- hardcoded model catalogs
- secrets exposure risk
- docs claims not backed by implementation/tests
- deployed GH Pages drift

Update/create appropriate issue/milestone tracking according to repository conventions for discovered work.

Do not merely report. Fix immediately any small, unambiguous defects that are prerequisite to continue safely, but keep broad architectural changes for subsequent prompts.

At the end provide:

1. root-cause map,
2. canonical components that should be reused,
3. legacy components to migrate/delete,
4. precise implementation sequence,
5. exact validation commands run,
6. updated issue/milestone/session state.
