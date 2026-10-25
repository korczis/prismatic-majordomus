# Canonical state machine (design contract; adapt to existing types)

READY → AUTHORIZING → QUEUED → CLAIMED → EXECUTING → VERIFYING → VERIFIED → DELIVERY_QUEUED → PR_WAITING → CI_WAITING → MERGED → RELEASED → DEPLOYING → SERVED_VERIFIED

Side transitions: `REFUSED(reason,policy,receipt)`, `BLOCKED(reason,dependency)`, `FAILED(reason,receipt)`, `CANCELLING`, `CANCELLED`, `UNKNOWN_OUTCOME`, `RETRYABLE(wait_until)`. No spontaneous UNKNOWN_OUTCOME→EXECUTING retry for non-idempotent operations; must reconcile remote evidence first.

`task.completed`, `issue.done`, `intent.satisfied`, `released` and `served_verified` are distinct and derive from existing sources. No special case allowing UI to bypass gates. A transport ACK is never a worker completion. Fenced leases must ensure outdated holders cannot commit or mark done. AuthZ occurs at command time, and when worker consumes a queued privileged ticket.

Events must contain: event_id, stream_id, sequence/cursor, timestamp, actor/node, target, execution_id, causation_id, correlation_id, schema_version, provenance, payload. Dedupe keyed by server-issued execution id and idempotency key for action; signed remote requests bind auth context and input digest. Store private logs securely, project only redacted summaries to Pages.
