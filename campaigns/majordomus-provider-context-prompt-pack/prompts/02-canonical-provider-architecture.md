# Prompt 02: Canonical Provider / Model Runtime Architecture

Use MAXIMUM AVAILABLE CONTEXT. Resolve current session + handover + issue/milestone state first. Re-read the forensic audit and verify it against the repository because concurrent work may have changed reality.

Implement the canonical provider/model runtime.

The architecture must distinguish, where repository semantics require it:

- provider kind
- provider instance
- transport
- model identity
- provider-native model ID
- runtime availability
- credentials state
- capabilities
- locality/runtime/peer location
- selection policy
- fallback policy
- invocation
- execution provenance

Do NOT solve this with one giant enum plus matches across consumers.

Create/reuse one canonical provider registry and model registry. Every consumer must derive state from these registries.

Establish/reuse a common provider adapter contract for:

- descriptor
- availability/probe
- model discovery
- capability reporting
- invocation
- streaming/cancellation where already supported
- safe error normalization

Migrate existing OpenAI/Codex, Anthropic/Claude, Gemini and Ollama implementations into the canonical architecture to the extent actually supported by the repo. Do not claim support that is not implemented and tested.

Explicit requirements:

- no duplicated provider/model lists in CLI/API/MCP/Cockpit/docs
- no hardcoded production model catalog unless genuinely canonical
- no secrets in safe state
- provider aliases resolve centrally
- provider/model IDs are typed/stable
- provider instance != provider kind
- model registry supports static + dynamic discovery
- expensive/network model discovery is cached and never required for repo-entry/banner path
- provider adapters do not own canonical session memory
- provider-native conversation/thread IDs are metadata only
- invocation is request-scoped, not a global mutable selected provider

Integrate provider state into canonical RepositoryEnvironment/runtime state without making repo entry expensive.

Add zero-registration extension tests using a fake provider: canonical registration should propagate automatically to all applicable backend projections without editing consumer-specific inventories.

Migrate/delete legacy duplicated implementation after consumers are moved.

Add/update schemas, migrations, ADR if project policy requires it, and architectural docs.

Run focused + repository validation and update session/handover/issue state before finishing.
