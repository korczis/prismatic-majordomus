# Observable State & Visual Truth Contract

## Invariant
`canonical facts -> typed bounded observation -> freshness+provenance+authorization -> scene projection -> artistic mapping -> pixels`. Never infer facts from pixels. Ambient motion conveys no operational event semantics.

## Suggested Rust-shape conceptual contract (adapt to repo)
```rust
struct ObservatorySnapshot {
  schema_version: String,
  generated_at: Timestamp,
  completeness: Completeness,   // Complete | Partial | Unavailable
  sources: Vec<SourceEvidence>,
  nodes: Vec<ObservableNode>,
  edges: Vec<ObservableEdge>,
  metrics: Vec<ObservableMetric>,
  last_cursor: Option<Cursor>,
}
struct ObservableEvent {
  id: StableEventId,
  cursor: Cursor,
  event_time: Timestamp,
  observed_at: Timestamp,
  source: SourceRef,
  kind: EventKind,
  severity: Severity,
  entity_ids: Vec<CanonicalEntityId>,
  relation_ids: Vec<CanonicalRelationId>,
  redaction: RedactionMetadata,
  payload: TypedEventPayload,
}
struct ObservableMetric {
  key: MetricKey,
  unit: Unit,
  value: Option<f64>,       // None is unavailable, not zero
  observed_at: Timestamp,
  provenance: SourceRef,
  quality: Quality,         // Observed | Derived | Estimated | Unknown
}
```
Shape is a PROPOSAL; use established project Rust types and existing schema conventions. Avoid `unknown` catch-all JSON blobs unless needed for versioned external protocol boundaries.

## Semantics
- `Observed` from authenticated/authorized source, with timestamps and source identities.
- `Derived` deterministic transformation of observed data; attach input refs and rule/version.
- `Inferred` hypothesis based on evidence with explicit confidence/reason, never auto promoted to fact.
- `Estimated` quantitative projection with method and tolerance; never equal billed cost without reconciliation.
- `Simulated` deterministic demo/replay fixture, visually labeled.
- `Unknown/Unavailable/Partial/Stale` distinct from healthy and zero.
- Edges require real relationship evidence; grouping by geometry must not imply a real network link.
- Event time vs time observed vs current freshness must be separate.
- Original event IDs and source identifiers preserved through aggregation.

## Snapshot / delta / resume
Initial bounded snapshot, cursor-marked event stream, expiry/gap signaling, bounded queue, dedup IDs, re-sync on cursor invalidity and rate-limited reconnection. Define semantics for out-of-order arrivals and retention. Prefer existing monitor interface; add thin SSE adapter only where justified, WebSocket only for real bidirectional interactions; cursor polling fallback. Do not rebuild full graph per frame. Coalesce high-rate bursts with exposed counts, not invisible loss. No backend request per animation frame.

## Privacy & public projection
Server classifies and redacts snapshot AND deltas BEFORE transfer. Apply same rules to scene inspect endpoint, error logging, caches, replay and downloadable artifacts. Reject public mode if public-safe projection unavailable; query parameter is not authorization. Never serialize raw prompts, secrets, user identity, private issue text, IP/hostname/topology or sensitive cost/account data into unauthenticated kiosk page. Provide anonymized aggregates only if explicitly permitted. Authorize drilldown URL access separately.

## Scene data API
SourceRegistry selects existing capabilities; ObservatoryProjection creates stable identities, bounded graph and time window aggregates. SceneRegistry uses capability requirement metadata, decides availability with explicit reasons. Scene renderer only consumes prepared scene model; never executes shell/agents/MCP from animation callbacks.

## Explainability inspector
For chosen visual element show `entity id, kind, source, last observed, validity, derived rule, evidence ref, related entities, source freshness, operational state, canonical URL` where authorized. Explain color, pulse, intensity, geometry and grouping. UI displays absent source prominently and does not render zeros.

## Anti-invariants (validation failures)
Rendering unknown data as normal, creating fake event pulses in live mode, duplicating frontend domain state, running privileged operations on click without explicit authorized action, independent hand-maintained scene inventories, leaking private data in public stream, lost cursor silent reset, pretending recovered data is real-time when stale.


## FINAL OVERRIDING REQUIREMENT — REAL DATA ONLY
All production/runtime exhibition modes (including marketing and demo) MUST use real observed live records or clearly-labeled REPLAY of real captured, authorized, redacted records. Synthetic events/metrics/topology MUST NEVER flow to a running exhibition. Synthetic data is permitted ONLY in isolated automated tests and explicitly non-deployable conceptual sketches. Decorative motion does not represent operational activity. When data does not exist, display unknown/unavailable/idle and reason. This overrides any earlier wording suggesting synthetic demonstration.
