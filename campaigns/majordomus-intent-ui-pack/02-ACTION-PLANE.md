# 02 — TYPED, POLICY-ENFORCED ACTION PLANE

Implement the **smallest complete action substrate** that allows the UI to execute all authorized existing capabilities without duplicating domain logic. Work from the actual registry and execution plane. Migrate or wrap existing shell operations only as justified by ownership and compatibility. Never create a generic unauthenticated shell runner or endpoints that accept arbitrary filesystem paths, shell fragments or unrestricted URLs.

Extend existing canonical capability metadata as necessary with typed: stable identity; schema version; input/output JSON Schema; operation/effect class (`query`, local mutation, remote mutation, destructive, long-running); scope/resources; preflight; authn/authz; idempotency; timeout/deadline; cancellation semantics; concurrency/locking; event and evidence output; docs; UI suitability. Reuse present enums/types rather than cloning. Make it possible to explain *why* an action is disabled, and to derive a safe form from input schema and contextual defaults. Fail closed on unknown effects and malformed inputs.

Canonical invocation envelope, **conceptual only** (adapt to existing types):

```
capability_id, capability_version, validated_input,
actor, repo_identity, context_revision, intent_id?,
correlation_id, idempotency_key, expected_state_revision?,
authorization, requested_autonomy
```

Return a typed result: validation/preflight refusal with actionable diagnostics, immediate query result, or accepted execution ID with linkable status. Add compare-and-set or equivalent optimistic concurrency where source records can be edited concurrently. Ensure atomic local file writes, lock discipline, bounded retries, exactly-once *effect intention* where practical and explicit at-least-once delivery semantics for events. Idempotency must prevent duplicate external operations on repeated clicks/network retries. Persist state before announcing success.

For all adapters (UI/HTTP, CLI, MCP): the same handler, validation, effect policy and audit event. Tests must prove cross-surface parity. UI must never directly call `git`, `gh`, external CI, arbitrary shell or a proprietary workflow behind the action layer. Existing adapters may legitimately use process runners behind the server boundary with allowlists and bounded working directory.

Security matrix: browser origin + CSRF (for cookie sessions), local bind defaults, authentication on network exposure, role/capability authz, no secrets in HTML or logs, sanitized streamed output, path traversal prevention, SSRF hardening for remote links, upload limits, rate limiting, timeout, process-group termination, no automatic dangerous commands based solely on LLM text. Commands may need explicit review/approval based on existing policy. Enforce via negative tests, not only docs.

Acceptance proof: invoke same safe example via available CLI/API/MCP, validate identical schema and authoritative outcome; retry with same idempotency key; submit malformed/unauthorized/stale revision; cancel long-running work; restart server mid-execution and reconcile actual durable state. Every path produces auditable evidence. Preserve backward-compatible read-only behavior while adding write support.
