# 01 — Typed action contract and security boundary

**Read MASTER_PROMPT.md first; its invariants are binding.**

## Implementation directive
Specify ActionIntent { target(kind,id,canonical_revision), action(implement|solve|verify|deliver), requested_by, actor, auth_context, idempotency_key, constraints, policy_decision, correlation_id }; ExecutionTicket {stable_id, lease_fence, scope, worker_requirements, state, timings, recorded_events, evidence_refs, delivery_refs}. Adapt current canonical types rather than proliferating. Define GET/read-only status versus POST mutation, auth/session model, CSRF, origin, signed remote identities, permission checks by scope and repo. Make HTTP/MCP/CLI adapters call the same typed capability. Prove replay-safe exact-once effect under request duplication, including uncertain transport outcome. Define typed event ordering, monotonic cursor, journal provenance and schema/version migration. Fail closed for unsafe mutations, clear reasons for offline and policy deny. Tests proving missing/invalid auth, cross-repo access, stale revision and adversarial retry.

## Mandatory handoff
Write: canonical files changed, existing capabilities reused, tests run/results, CI SHA, decisions, blockers, safe rollback, risk of change and next step. Keep incomplete as incomplete.
