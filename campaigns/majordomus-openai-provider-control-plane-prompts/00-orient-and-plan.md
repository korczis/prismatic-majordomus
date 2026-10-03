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


# Mission 00 — Repository archaeology, gap analysis and executable implementation plan

Do not implement the provider subsystem yet.

Your task is to prove you understand the live repository and turn the requested OpenAI/provider integration into the repository's own project model.

## Inspect deeply

Inspect at minimum:

- root `AGENTS.md`, `README.md`, `CLAUDE.md`;
- `.ai/README.md`, `.ai/manifest.yaml`;
- effective project + vendored rules;
- `.ai/repo/policy.yaml`;
- task lifecycle and use-case workflow;
- ADRs 0002, 0012, 0020 and every newer ADR touching capabilities, projections, providers, sessions, prompts, runtime, web surfaces or worktrees;
- current project issues/milestones related to providers/models/OpenAI/agents;
- `apps/majordomus-cli/src/app.rs`;
- capability descriptor, registry, builtin composition and macro implementation;
- CLI dispatch architecture;
- HTTP router/OpenAPI/surface discovery;
- MCP surface/transport;
- Cockpit rendering/navigation/pages;
- graph composition;
- index/discovery/kinds/schema machinery;
- sessions/prompts/handover code;
- skill discovery + provider projections;
- tests for projection parity, HTTP/MCP, Cockpit, generation and use cases;
- generated docs and Zola/site integration;
- Cargo dependencies and runtime assumptions.

Use search aggressively, but do not infer semantics from names alone.

## Produce in-repo planning evidence

Create or update the appropriate canonical issue(s)/milestone(s) rather than writing a detached TODO document.

Build a dependency graph of work packages:
- governance/ADR changes;
- provider kernel;
- provider configuration + credential resolution;
- normalized model descriptors/discovery;
- OpenAI Platform adapter;
- run/event model;
- streaming;
- host abstraction;
- Codex/ChatGPT integration;
- skills projection;
- idea/intake model;
- import path;
- capability projections;
- Cockpit;
- tests/use cases;
- docs/site;
- migrations/backward compatibility.

Identify what already exists and MUST be reused.

## Required decision record

If the live architecture has no accepted decision that cleanly covers provider + host + run separation, prepare a new ADR proposal. The ADR must distinguish:

- provider: an execution backend/model platform;
- host/client: a product or agent runtime in which work happens;
- model: provider-discovered execution target;
- run: canonical Majordomus execution episode/result;
- tool: callable capability supplied to a model;
- skill: declarative reusable instruction asset;
- conversation/session: provider/host-local continuity, not necessarily equivalent to Majordomus session;
- idea: curated semantic intake object, not raw transcript storage.

Explicitly reject conflating OpenAI API, ChatGPT, Codex and workspace/agent APIs.

## Output of this prompt

End with:
- repo findings;
- exact existing mechanisms to extend;
- exact anti-patterns to avoid;
- proposed issue/milestone breakdown;
- migration risks;
- tests/use cases that will prove completion;
- list of any new rules/doctrines actually justified.

Do not start implementation unless doing so is necessary to create the canonical planning records and the repository lifecycle expects them.
