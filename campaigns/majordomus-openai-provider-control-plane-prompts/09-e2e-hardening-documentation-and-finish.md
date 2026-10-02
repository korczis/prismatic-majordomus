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


# Mission 09 — E2E hardening, docs, use cases, migration, performance and finish

Treat this prompt as an adversarial release review.

Assume previous implementation is incomplete until evidence proves otherwise.

## Architecture audit

Search the entire repository for:
- OpenAI-specific types outside adapter boundary;
- duplicate model/provider lists;
- manual CLI/API/MCP/Cockpit registrations;
- hard-coded route/tool/page inventories;
- duplicate schema definitions;
- duplicate docs tables;
- secrets or fake-secret snapshots;
- raw transcripts;
- unregistered `.ai/` content;
- new directories missing required README/context contract;
- generated files edited as sources;
- network calls in repository-open/hot paths.

Fix every structural violation.

## Use cases

Create executable use cases for at least:

1. configure/detect OpenAI without leaking credential;
2. discover models;
3. select a model by required capabilities;
4. execute a basic OpenAI response through canonical run;
5. execute a tool call round trip;
6. stream a run and get same final result as non-stream;
7. inspect provider/model via CLI;
8. inspect same via HTTP/OpenAPI;
9. inspect same via MCP;
10. inspect same in Cockpit;
11. Codex sees Majordomus MCP/skills;
12. capture an idea through MCP if implemented;
13. dry-run historical ChatGPT import;
14. promote an idea to canonical project object;
15. degraded/offline/no-key provider state.

Use the repository's executable scenario system, not prose-only examples.

## Integration tests

Require:
- fake provider server;
- CLI black-box;
- HTTP black-box;
- MCP black-box;
- cross-protocol parity;
- Cockpit probe;
- generator check;
- schema validation;
- mutation security tests;
- secret redaction property/regression tests;
- import idempotency;
- provider event state machine;
- cache invalidation/staleness semantics.

## Performance

Benchmark only meaningful hot paths:
- startup without network;
- provider listing from config/cache;
- model listing from fresh cache;
- capability projections;
- Cockpit provider/model pages.

Live provider network latency is not a deterministic performance baseline.

## Documentation

Update canonical docs and let generated docs derive:
- provider architecture;
- OpenAI setup;
- credentials;
- model discovery;
- capability selection;
- run/event model;
- host integrations;
- skills projection;
- ChatGPT import;
- ideas/intake;
- CLI;
- API/OpenAPI;
- MCP;
- Cockpit;
- security/trust model;
- troubleshooting;
- offline/degraded behavior;
- extension guide for a second provider.

The extension guide must demonstrate that adding a second provider requires implementing an adapter + descriptor/config, NOT editing every projection.

Update landing/site navigation only through existing data-driven mechanisms.

## Migration/backward compatibility

Ensure existing:
- MCP clients;
- CLI commands;
- docs routes;
- OpenAPI consumers;
- Cockpit;
- existing `.ai/` layers
continue working unless an explicit migration is justified.

Provide deterministic migration tooling if schema/config layout changed.

## Final gates

Run all live repository gates, including at minimum the current equivalents of:
- format;
- lint/clippy with warnings denied;
- unit + integration + doctest;
- generate --check;
- capabilities validate;
- usecase impact/coverage;
- Cockpit checks;
- docs/site build;
- schema/front-matter validation;
- benchmark coverage;
- task `check`;
- task `finish`.

Do not mark the issue complete until the repository's own evidence system agrees.

## Final report

Leave a concise implementation report in the canonical issue/decision evidence:
- what changed;
- invariants preserved;
- security decisions;
- exact commands/tests executed;
- known intentionally deferred capabilities;
- how to add provider #2;
- how OpenAI, ChatGPT and Codex differ in the resulting architecture.
