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


# Mission 05 — Host adapters: Codex, ChatGPT/Workspace Agents and skill projection

Implement host integration separately from provider execution.

## First: verify current official support

While executing, inspect current official OpenAI/Codex/ChatGPT documentation and explicitly record what is officially supported.

Never build Majordomus architecture on undocumented ChatGPT web endpoints or DOM scraping.

## Codex host

The repository already has `.codex/config.toml`, MCP bootstrap and AGENTS-based context.

Make Codex a first-class host descriptor that can report:
- detected configuration;
- Majordomus MCP connection configuration;
- repository instruction/bootstrap state;
- skill projection state;
- diagnostics.

Do not duplicate the client config generator if one already exists.

## ChatGPT / Workspace host

Treat supported ChatGPT workspace/agent APIs as a host adapter, not a model provider.

If the official API supports triggering/running published workspace agents:
- create a host execution capability with its own typed run reference;
- map workspace conversation/run identifiers without conflating them with Majordomus sessions;
- keep credentials independent from OpenAI Platform credentials if the API semantics require that;
- expose unsupported capabilities explicitly.

The user-provided ChatGPT Project URL must not become a magic scraper integration.

## Skills

The repository already treats skills as data.

Implement or extend a projection layer from canonical Majordomus skills to provider/host-specific layouts only where necessary.

Rules:
- one canonical skill body;
- generated/symlinked/adapter projection is derived;
- no manual duplicate skill registration;
- deterministic validation;
- host-specific compatibility diagnostics;
- skills exposed in CLI/API/MCP/Cockpit through existing object/capability mechanisms.

If Codex expects `.agents/skills`, produce that as a projection from the canonical skill source rather than making it a second source of truth.

## Host capability examples

Potential canonical capabilities:
- `hosts.list`
- `hosts.describe`
- `hosts.health`
- `hosts.skills`
- `hosts.project`
- `workspace_agents.list` only if official API permits it
- `workspace_agents.run`
- `workspace_agents.run_status`

Use live repository naming/module conventions.

## E2E

Prove that opening/using supported hosts yields the same Majordomus context/MCP capabilities without per-host manual registration.
