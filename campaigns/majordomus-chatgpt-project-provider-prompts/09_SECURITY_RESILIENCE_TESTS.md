# Phase 9 — Security, resilience, hostile tests and observability

Assume upstream changes and messy browser state. Prove failure is safe.

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

## Threat model

Cover:
- session/cookie/token leakage;
- malicious/oversized upstream payload;
- script/HTML content rendered in UI/docs;
- cross-account/workspace confusion;
- stale/locked browser profile;
- compromised fixture;
- path traversal in filenames;
- SSRF if remote file URLs are fetched;
- unbounded file/message size;
- replay/duplicates/out-of-order events;
- browser/SSE/WebSocket disconnect;
- partial pagination/cursor loops;
- unrelated tab observation;
- diagnostics leaking content;
- locally connected private data accidentally exposed on publicly bound API/Cockpit.

## Privacy defaults

Status/list commands should expose metadata by default. Full message content requires explicit show/export operations according to existing UX.

Audit server bind/auth behavior. A local ChatGPT connection must not silently become remotely readable.

## Browser security tests

Prove:
- origin/target scoping;
- cookies/auth/CSRF never persist;
- unrelated traffic is excluded;
- disable/kill-switch works.

Seed fake secrets and assert they never appear in fixtures/logs/snapshots/API diagnostics.

## File safety

If attachments are downloaded:
- safe path handling;
- no path traversal;
- content/size limits;
- hashes;
- atomic writes;
- explicit overwrite;
- no execution;
- no trust in upstream filename path components.

## Parser resilience

Unknown optional fields tolerated; incompatible required shapes fail loudly. No production `unwrap()` on private upstream data.

## Failure matrix

Test:
- 429/5xx;
- auth expiry;
- closed browser;
- stream disconnect;
- schema mismatch;
- corrupt checkpoint;
- workspace/account switch;
- inaccessible/deleted project;
- deleted conversation;
- repeated cursor/cursor cycle;
- pagination truncation;
- timestamp anomalies;
- concurrent sync+watch.

## Property/fuzz-style tests

Where supported:
- sanitization never emits seeded secrets;
- stable IDs/dedupe;
- canonical ordering;
- checkpoint round-trip;
- malformed JSON/unknown content;
- route/schema fingerprinting;
- replay idempotency.

## Observability

Use existing tracing/metrics for:
- sync duration;
- processed/deduped count;
- retry count;
- transport chosen;
- fallback count;
- contract failure;
- checkpoint age;
- watch connection state;
- redaction count without values.

Do not emit conversation bodies into telemetry by default.

## Security gate

Add/extend a repo gate scanning fixtures/generated artifacts for known and seeded secret patterns. Prove with a test canary that the gate catches leakage.

Acceptance: protocol break, expired auth or browser crash causes no canonical corruption, no secret leak, bounded retries, actionable diagnostics and resumable sync.
