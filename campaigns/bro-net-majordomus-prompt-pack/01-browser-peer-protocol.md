# Prompt 01 — Browser Peer Protocol


You are working inside the `prismatic-majordomus` repository.

Non-negotiable execution discipline:
- Inspect the real repository before deciding architecture.
- Reuse existing Majordomus abstractions, registries, rules, doctrines, schemas, adapters, diagnostics, tests, and deployment paths.
- Do not create parallel systems if an existing canonical mechanism can be extended.
- Everything must be typed, deterministic, data-driven, discoverable, testable, documented, and enforced where appropriate.
- No hardcoded duplicate inventories across CLI/API/MCP/Cockpit/docs.
- No claims without repository evidence.
- No "implemented" status unless tests pass and the relevant runtime/deployment surface is verified.
- Preserve existing public interfaces unless a migration is justified and completed.
- Prefer deleting obsolete duplicated machinery over adding another adapter layer.
- Use the repository's real canonical commands for format/build/test/check/deploy. Discover them instead of inventing them.
- At the end, report root cause, architecture, files changed by purpose, tests executed, validation evidence, and remaining debt.


Mission:
Implement the canonical typed protocol/model that makes a browser a first-class Majordomus mesh peer.

Requirements:
- Reuse the existing peer/session/mesh model where possible.
- Introduce a browser-specific adapter/type only where browser runtime semantics differ.
- Define canonical peer identity, lifecycle, heartbeat/status, capabilities, task request/result, diagnostics, and provenance references.
- Keep transport independent from domain model.
- Define schema versioning using repository conventions.
- Serialize through existing schema/OpenAPI/MCP machinery where appropriate.
- No browser-extension-specific JSON structures leaking into core domain types unless justified.

Model state such as:
- discovered
- connecting
- online
- busy
- degraded
- offline

But use existing state models if present.

Every result must be attributable to:
- peer
- task/job
- capability invoked
- target origin/page
- timestamp
- relevant session/intent/context
- execution status
- evidence/provenance payload where appropriate

Tests:
- serialization roundtrip
- schema validation
- stable identifiers
- lifecycle transitions
- duplicate peer handling
- stale heartbeat behavior
- version mismatch behavior
- malformed capability declarations
- deterministic capability ordering
- backward compatibility if existing peer schemas are extended

Acceptance criteria:
- browser peer exists as typed canonical Majordomus entity
- no transport dependency in core model
- schemas generated/validated
- tests pass
- docs explain protocol ownership and extension path
