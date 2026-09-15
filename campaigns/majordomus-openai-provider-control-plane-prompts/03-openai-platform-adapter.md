# Global execution contract

Before changing code, perform the repository's own bootstrap exactly as the live `AGENTS.md` and `.ai/README.md` require.

Non-negotiable constraints:

1. Read and obey live rules and their dependencies. Do not trust filenames alone.
2. Resolve path-specific context before changing files.
3. Use the canonical branch/worktree topology. Do not work on a feature branch in the trunk checkout.
4. Every substantial change must be attached to the repository's issue/milestone lifecycle if that lifecycle is enabled by the live policy.
5. Existing architecture outranks this prompt. Adapt this requested intent to the repository's current canonical mechanisms rather than creating parallel machinery.
6. No manual duplicate registration of a provider, model, capability, route, MCP tool, OpenAPI operation, Cockpit page, doc entry, benchmark target or skill projection where it can be derived.
7. Do not silently weaken a rule or bypass a gate to make the change pass.
8. Do not expose secrets or raw credentials in any output, diagnostics, snapshots, fixtures, generated docs or browser state.
9. New user-visible claims require executable evidence through the project's use-case/test system.
10. Update docs/ADRs/rules in the same change when semantics change.
11. Prefer strongly typed Rust models plus `schemars`/existing schema machinery over untyped JSON maps.
12. Preserve deterministic generation.
13. No network activity on repository-open or other hot paths unless an explicit operation requests it.
14. Provider discovery failure must degrade/refuse according to explicit semantics. Never fabricate model capabilities.
15. Treat external provider data as untrusted input.
16. Do not persist raw provider responses by default merely because they are available.
17. Keep provider-specific protocol vocabulary below the adapter boundary unless a provider-specific diagnostic explicitly needs to surface it.
18. Finish with focused tests, cross-projection parity tests, generation checks, use-case coverage, docs checks, lints and the repository's normal `finish` evidence.


# Mission 03 — Production OpenAI Platform adapter

Implement OpenAI as the first production ProviderAdapter.

First inspect current official OpenAI API documentation while executing this prompt. Do not rely on model-memory assumptions about endpoint shapes, model names or supported features.

## Boundary

The adapter owns:
- authentication header construction;
- base URL;
- request/response JSON;
- OpenAI error envelopes;
- model enumeration;
- Responses API translation;
- streaming transport/event decoding;
- optional OpenAI-specific feature metadata.

Core Majordomus does NOT expose raw OpenAI SDK/types.

Prefer the repository's dependency philosophy. Evaluate direct HTTP implementation versus an SDK based on:
- footprint;
- async/runtime implications;
- streaming;
- testability;
- version churn;
- current existing HTTP stack.

Do not casually add a second heavyweight runtime.

## Responses execution

Implement a minimal but complete first vertical slice:
- text input;
- system/developer/user semantic roles mapped correctly;
- text output;
- structured JSON output if core already models it;
- function/tool definitions;
- tool-call results;
- usage;
- provider request/run ids;
- errors;
- cancellation/timeout behavior supported by current runtime.

Then extend only capabilities needed by concrete use cases.

## OpenAI model discovery

Use the official model enumeration endpoint where applicable.

Do not assume enumeration alone provides every capability. Separate identity discovery from capability enrichment/probing.

## OpenAI-compatible endpoints

Do NOT pretend arbitrary OpenAI-compatible servers behave exactly like OpenAI.

If useful, implement a distinct adapter family/configuration with:
- custom base URL;
- explicit compatibility level;
- conservative capabilities;
- no automatic assumption that proprietary OpenAI tools exist.

## Security

Tests must prove:
- Authorization header never logs;
- API key never serializes;
- HTTP errors cannot echo secrets through diagnostics;
- Cockpit/OpenAPI/MCP cannot expose credential values;
- fixtures use fake credentials and local fake servers.

## Contract tests

Build a deterministic fake OpenAI server fixture and test:
- model discovery;
- basic Responses execution;
- tool round trip;
- structured response if supported;
- provider error mapping;
- malformed response;
- timeout;
- rate limit;
- empty/missing fields;
- secret redaction.

Real-network tests, if any, must be opt-in and never CI-required.
