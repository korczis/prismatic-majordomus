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


# Mission 04 — Canonical runs, events, streaming, provenance and audit

Normalize provider execution into a Majordomus run model.

## Canonical run

Implement a typed Run object containing only what Majordomus actually needs, such as:
- run id;
- provider;
- model;
- originating Majordomus session/task if available;
- normalized request summary;
- tool set identity;
- capability requirements;
- timing;
- status;
- usage;
- result summary;
- provider references;
- provenance.

Do not persist raw prompts/responses into tracked repository context by default.

Respect `never-store-transcripts`.

## RunEvent

Design a closed/forward-compatible event vocabulary for:
- run started;
- provider accepted;
- output delta;
- reasoning summary when provider explicitly supplies safe summary data;
- tool call requested;
- tool call started/completed/failed;
- usage updated;
- response completed;
- run failed/cancelled/completed.

Provider event names map to these exactly once.

Do not spread OpenAI event strings through CLI/MCP/Cockpit.

## Streaming

Create one internal stream/event interface consumed by:
- CLI streaming;
- HTTP streaming if existing server stack supports it appropriately;
- Cockpit live view;
- audit/event persistence if enabled.

If current HTTP stack cannot sanely support a desired streaming transport without architectural damage, document and phase it rather than introducing a second server stack casually.

## Audit semantics

Separate:
- ephemeral live events;
- persisted audit summary;
- raw provider payloads.

Default persisted data must be minimal, privacy-aware and consistent with existing local/session policy.

Any tracked durable record needs a strong justification and schema.

## Cost/usage

Normalize tokens/usage without pretending all providers use the same accounting fields. Preserve provider-native metadata behind a bounded extension field only if needed.

Never hard-code price tables as timeless truth. If cost estimation is introduced, source and timestamp it explicitly.

## Tests

Prove:
- event order/state-machine validity;
- terminal status exactly once;
- malformed provider sequences fail deterministically;
- streaming/non-streaming produce equivalent final result;
- secrets never enter events;
- raw transcript storage is not accidentally introduced.
