# Canonical contracts: prompt, context, decision, execution, outcome

These are conceptual field sets for discovery, not instructions to create parallel structs. Reuse authoritative existing types and version schemas when possible.

## Immutable ingress receipt

`IngressReceipt { schema_version, event_id, provider, transport, session_id, provider_prompt_id?, actor_kind, original_prompt_ref?, payload_hash, repo_identity, observed_git_head, observed_at, causation_id?, correlation_id, source_classification, capture_status }`.

Separate raw bytes of a human prompt (sensitive, access-controlled, never general KB) from routing metadata that can be safely indexed. Missing provider fields are `unknown` with reason. Deduplication key should include provider/session/prompt identity where stable; do not merge legitimately repeated human prompts solely by text.

## Resolution and decisions

`IntentResolution { receipt_id, route, candidate_intents, action, selected_intent?, evidence_refs, confidence?, conflicts, governance_policy_hash, rationale, materiality, human_approval_required }`.

Actions: `create`, `attach`, `refine`, `split`, `merge`, `supersede`, `control`, `answer`. Some actions require permission/confirmation or record migrations. Read-only responses may have no intent allocation.

`ContextProjection { selected_refs, excluded_refs_with_reasons, precedence_chain, source_revisions, trust_level, token_budget, expiry/fingerprint, prompt_instruction_refs }`.

## Durable execution state

Model durable workflow stages according to current owners. An example high-level view is `received → resolved → grounded → authorized → planned → running → verifying → delivered`. The implementation needs explicit terminal/exception states: `completed`, `partial`, `blocked`, `failed`, `cancelled`, `superseded`. The existing Rust capability execution state machine (if still present) may be a different level and should **not** be conflated with overall intent delivery status. Design composition, not an ad hoc enum replacement.

Append-only transitions carry sequence, timestamp, causation, expected prior version and actor. Side effects are wrapped in idempotent operations plus reconciliation. Crash after external effect but before local ACK must not create a second PR/redeploy. No fabricated exactly-once messaging guarantee.

## Outcome and closure

`OutcomeEvidence { intent_id?, execution_ids, completed_scope, test_evidence, git_shas, PRs, CI_run_and_sha, deployment_probe?, generated_drift, governance_reconciliation, unresolved_blockers, actual_status }`.

All public surfaces read projections of the same canonical persisted state. No two independent status flags.

## Contracts to test

- duplicate and reordered hook events; delayed events, clock skew, missing optional data
- prompt capture + routing idempotency, never silent text loss
- Git HEAD changes between preflight and mutation
- compaction and restart with session reconnection
- corrupt journal/cache, downgrade/upgrade, version migration
- stale peer claim, concurrent worktree updates
- policy denied vs infrastructure unavailable
- no private raw prompt leakage to docs, UI, logs or other providers
- CLI/API/MCP/Cockpit projection version parity
