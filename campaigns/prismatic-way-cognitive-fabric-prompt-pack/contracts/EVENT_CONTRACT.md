# Logical event and reconciliation contract

**Design aid, not a mandate to introduce new repository types.** First map each field onto existing Rust types and journal schemas; only add missing fields when justified by a consumer/test.

```rust
// Conceptual example only. Adopt actual naming and stable IDs from HEAD.
struct CognitiveEventRef {
  source: EventSource,
  source_event_id: String,
  repository_id: RepositoryId,
  worktree_id: Option<WorktreeId>,
  session_id: Option<SessionId>,
  causal_parent: Option<EventId>,
  observed_at: Timestamp,
  source_revision: Option<Revision>,
  content_digest: Digest,
}

struct CandidateChange {
  canonical_entity: EntityId,
  operation: ProposedOperation,
  provenance: Vec<EvidenceRef>,
  epistemic_status: EpistemicStatus,
  expected_revision: Revision,
  origin_event: EventId,
}
```

Pseudocode:

```text
receive(event)
  authenticate/admit source using existing boundary
  normalize(event)
  reject malformed, wrong-repository, unauthorized, excessive-size event
  key = stable_source_identity(event)
  if committed(key): return previously committed result
  affected = reverse_dependency_closure(changed_source_refs(event), bounded)
  facts = deterministic_extract(event, affected)
  proposed = reconcile_existing_entities(facts)
  if material_ambiguity(proposed): schedule existing reasoning plan within budget
  validate schema, scopes, provenance, evidence, expected revision
  atomic_apply_through_canonical_owner(proposed, idempotency=key)
  publish bounded typed delta via existing monitor/mesh/UI facilities
  record timing, outcome, reuse and budget counters
```

Idempotency is outcome-level, not only request-level. Concurrent 'same key' writers must converge to one result or explicit conflict. Generated events should reference causal parents so recursive effects can be stopped. Queue and retry policy must distinguish transient vs permanent failures. No event with untrusted text may directly become a command to run or a rule to obey.

Verification examples: shuffle the same event stream repeatedly; the result set of canonical entities and links must match. Replay an identical hook 100 times; exactly one logical change. Two contradictory claims remain a visible conflict with provenance instead of picking newest wall clock time.
