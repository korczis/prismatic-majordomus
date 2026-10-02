# MASTER ORCHESTRATOR — ChatGPT Project machine access for Majordomus

You are Claude Code with a large context window working inside `prismatic-majordomus`.

The user wants machine-readable access and subscription-like synchronization for his own ChatGPT Projects, including the Majordomus project, even when official public API coverage is incomplete. Build this as a production-quality Majordomus provider using official mechanisms where available and an authenticated browser/CDP + observed private protocol compatibility layer where necessary.

Do not ask the user to choose filenames, module names, endpoint paths, storage formats, schemas, route names, crates, JS components or obvious implementation details. Discover the repository and make the correct choices.

## Majordomus invariants

Treat these as architectural constraints. First discover the repository's current rules/doctrines and use their canonical terminology. If an equivalent invariant already exists, extend it rather than duplicating it.

- One source of truth; projections are derived/generated.
- Infer/discover before introducing registration.
- Typed Rust/domain model before renderer-, transport- or UI-specific structures.
- Schema-backed/versioned external boundaries.
- Deterministic ordering and stable identifiers.
- Zero manually synchronized inventories across CLI/API/OpenAPI/MCP/Cockpit/docs/completion.
- New providers/capabilities/entities become visible through existing registry/discovery mechanisms automatically.
- Reuse existing diagnostics, validation, event, storage, routing, docs and generation infrastructure.
- Fix in-scope legacy duplication instead of wrapping it.
- Public implementation follows repository documentation/test/doctest rules.
- No secrets in repository, generated docs, traces, fixtures, logs, snapshots or caches.
- Raw observation != normalized entity != derived knowledge. Preserve provenance.
- Capability negotiation is explicit. Unsupported operations never masquerade as empty success.
- Private/undocumented protocol behavior is a replaceable transport detail, never the canonical domain model.
- Browser/CDP automation may operate only inside the user's already-authenticated context.
- Never bypass MFA, CAPTCHA, access controls, account boundaries or anti-abuse controls.
- Never collect unrelated browser traffic.
- Do not hardcode undocumented ChatGPT endpoints from a prompt or random gist. Observe actual authorized traffic, sanitize it, validate it, then implement against evidence.
- Prefer read-only support first. Any future write action needs explicit capability, policy and user intent.

Before modifying code, read all relevant `AGENTS.md`, `.ai/**`, `.majordomus/**`, rules, doctrines, schemas, README hierarchy, ADR policy, provider abstractions, CLI architecture, REST/OpenAPI/MCP/Cockpit code, site/GitHub Pages generators, tests, worktree conventions and canonical validation commands. Discover paths; do not assume them.

## Mission

Implement an extensible **External Conversation Workspace / Project Provider** subsystem and a ChatGPT implementation that can:

- discover capabilities;
- enumerate/read authorized projects/workspaces;
- enumerate/read conversations and messages;
- enumerate/read attachment/file metadata and download only when explicitly supported;
- read project instructions/context metadata if observable and authorized;
- perform incremental sync from durable checkpoints;
- provide subscription-like/watch semantics using observed stream/network events when possible and deterministic reconciliation when not;
- attach to an explicitly configured authenticated Chromium-family browser via CDP;
- capture only relevant ChatGPT network/activity observations;
- sanitize before persistence;
- infer/version protocol observations and schema fingerprints;
- optionally use validated private HTTP operations as a fast path;
- fall back to browser/CDP when private contracts break;
- retain complete provenance;
- feed existing session-context/handover/knowledge pipelines through explicit imports/derivations;
- expose the same canonical data through CLI, JSON, REST, generated OpenAPI/Swagger, MCP, Cockpit/WebSocket/event stream, docs and GitHub Pages;
- update rules, doctrines, schemas, skills, ADRs, examples, tests, gates and generated artifacts;
- be reusable for future Claude/Gemini/Codex/browser-backed providers.

## Architectural boundaries

Exact names must follow the repository, but dependency direction must be equivalent to:

```text
Transport
  official | browser/CDP | private HTTP
        |
        v
Sanitized provider observation
        |
        v
Provider-specific DTO/mapper
        |
        v
Canonical ExternalWorkspace / Conversation / Message / File
        |
        v
Sync + checkpoint + event + provenance
        |
        +--> normalized store
        +--> derived knowledge/session imports
        +--> application service
               +--> CLI
               +--> REST/OpenAPI
               +--> MCP
               +--> Cockpit
               +--> docs/generated metadata
```

Raw ChatGPT DTOs must never become public Majordomus contracts.

## Capabilities

Use a typed capability model, conceptually including:

```text
projects.read
projects.watch
conversations.read
conversations.watch
messages.read
messages.watch
files.read
files.download
files.watch
instructions.read
transport.official
transport.browser
transport.private_http
```

Future write actions such as message creation or file upload are separate explicit capabilities. Do not smuggle them into read support.

## Transport policy

Centralize transport selection. Conceptually:

```text
official supported + healthy
  -> validated private HTTP fast path if policy allows
  -> authenticated browser/CDP
  -> optional DOM fallback only if there is no better source
  -> typed unavailable diagnostic
```

Do not copy cookies into config to make direct HTTP convenient. If browser-context execution is safer, use it.

## Subscription/watch truthfulness

Expose a canonical `watch/changes` abstraction, but include acquisition mode:

```text
push
SSE/stream
WebSocket
browser_network_observed
poll/reconcile
manual
```

Do not call polling a native subscription.

## Protocol observation and bounded self-healing

When a private operation breaks:

1. mark the contract degraded/incompatible;
2. fail closed rather than parsing partial junk;
3. fall back to browser transport if available;
4. capture sanitized new observations;
5. generate a deterministic schema/compatibility diff;
6. require validation/tests before promoting a new direct contract.

Never let one observed request rewrite production code automatically.

## Worktree discipline

Discover and obey the repository's worktree doctrine. Use canonical Majordomus tooling if present. Do not invent parallel branch/worktree semantics.

## Execute these phases to completion

1. repository audit and architecture;
2. canonical provider core;
3. browser/CDP transport;
4. protocol observations and private HTTP fast path;
5. sync/event/checkpoint/provenance;
6. ChatGPT semantic mapping;
7. CLI/API/OpenAPI/MCP/Cockpit;
8. docs/rules/doctrines/skills/GitHub Pages;
9. security/resilience/property/integration tests;
10. re-audit, migration, generated artifacts and closure.

Do not stop after design if code can be implemented without live credentials. CI must work from mocks/sanitized fixtures. Live ChatGPT access is an optional smoke validation.

## Hard acceptance criteria

- one canonical provider registry/discovery path;
- zero per-surface provider inventories;
- typed capabilities/support level/transport metadata;
- stable provider/upstream/local IDs;
- deterministic ordering;
- browser target/origin scoping;
- sanitization before persistence;
- no cookies/auth headers in logs/fixtures/snapshots;
- versioned protocol observations;
- direct private transport contract tests;
- safe browser fallback;
- incremental checkpointed sync;
- idempotent replay/dedupe;
- raw/normalized/derived separation;
- provenance end to end;
- CLI/API/OpenAPI/MCP/Cockpit/docs derived from canonical services/registries;
- rules/doctrines/gates enforce the architecture;
- new provider can be added without editing every consumer;
- canonical repository checks pass;
- generated docs/schemas/artifacts are in sync;
- no in-scope legacy duplicate implementation survives.

## Final report

Provide:
1. root gap;
2. final architecture;
3. capabilities;
4. transport policy;
5. sync/watch semantics;
6. security/privacy;
7. storage/provenance;
8. surfaces updated;
9. governance/docs;
10. exact validation commands/results;
11. files changed grouped by purpose;
12. live runtime setup needed;
13. upstream limitations.

Do not claim live access unless it was actually validated.
