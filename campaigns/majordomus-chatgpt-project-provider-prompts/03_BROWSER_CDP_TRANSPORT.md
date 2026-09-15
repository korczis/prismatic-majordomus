# Phase 3 — Authenticated browser/CDP transport

Implement the robust gray-zone transport: an explicitly configured, already-authenticated Chromium-family browser observed through CDP.

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

## Boundary

Browser code is transport infrastructure. It emits sanitized observations/results. It does not define project/conversation domain semantics.

## Connection modes

Integrate with repo conventions to support sensible modes such as:
- attach to explicit CDP endpoint;
- launch configured Chromium with explicit persistent profile reference;
- reconnect to Majordomus-managed browser session.

Do not:
- silently copy the user's default browser profile;
- export cookies into config;
- globally disable browser security;
- add stealth/evasion plugins;
- bypass MFA/CAPTCHA.

## Target/origin scoping

Observe only authorized ChatGPT targets/origins. If scope cannot be proven, fail closed.

Do not persist unrelated browsing activity.

## Network observer

Capture only metadata/data needed for provider operation:
- method;
- normalized route fingerprint;
- content type/status/timing;
- allowlisted response/request bodies with strict size limits;
- SSE/WebSocket frames if supported and relevant;
- navigation/lifecycle;
- request/response schema fingerprints.

Strip before persistence:
- Cookie;
- Set-Cookie;
- Authorization;
- CSRF/session tokens;
- secret-bearing query parameters;
- unrelated headers;
- configured sensitive payload fields.

Sanitization occurs before storage, logs, fixtures or API exposure.

## Browser lifecycle

Implement:
- connection status;
- bounded reconnect/backoff;
- tab/target rediscovery;
- browser crash/closed tab handling;
- profile lock diagnostics;
- account/workspace fingerprint change detection where possible;
- graceful shutdown.

Never continue writing to the previous provider namespace after identity/workspace changes without explicit reconciliation.

## Raw observation envelope

Produce typed observations with fields equivalent to:
- observation ID;
- provider;
- transport=browser;
- browser session;
- timestamp;
- target-safe fingerprint;
- event kind;
- route/stream fingerprint;
- sanitized payload;
- schema fingerprint.

## UX

Expose connect/status/inspect/disconnect through the existing generic provider/transport command structure. Ensure API/MCP/Cockpit status derives from the same registry.

## Tests

Use a local deterministic test server, not live ChatGPT in CI.

Test:
- attach/launch abstraction;
- request/response observation;
- SSE/WebSocket observation where implemented;
- strict origin filtering;
- secret redaction;
- oversized/malformed payloads;
- disconnect/reconnect;
- multiple tabs;
- identity change;
- no unrelated traffic persisted.

Provide an opt-in live smoke procedure for the user's own account, excluded from CI.
