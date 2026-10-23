# Proposed typed contract (adapt to existing canonical entities)

**Do not implement as a second ledger.** Investigate actual Rust types before naming fields. Logical concepts:

```rust
struct UsageObservation {
  schema_version: SchemaVersion,
  event_id: EventId,  // stable dedup, source-key + emitter + kind
  source: TelemetrySource,
  observed_at: Timestamp,
  received_at: Timestamp,
  provider: ProviderId,
  model: Option<ModelId>,
  subject: Option<ExecutionRef>, // only with causal evidence
  request_id: Option<RequestId>,
  tokens: TokenBreakdown, // separate input, output, cache_read, cache_write
  quantity_semantics: Temporality, // delta/cumulative/request fact
  cost: Option<CostMeasurement>,
  provenance: EvidenceRef,
  quality: MeasurementStatus,
}

enum MeasurementStatus { Observed, Derived, Estimated, Reconciled, Unobserved { reason: String } }
struct CostMeasurement { amount_minor_units: i64, currency: Currency, basis: CostBasis, price_version: Option<String> }
struct AttributionEdge { observation: EventId, target: EntityRef, method: AttributionMethod, confidence: Confidence, evidence: EvidenceRef }
struct Coverage { source: TelemetrySource, window: TimeRange, expected: Option<u64>, observed: u64, gaps: Vec<Gap> }
```

**Caution:** an `event_id` alone may not distinguish metric aggregate from request event; dedup must use signal-specific native identifiers and interval semantics. Use decimal/fixed point for money, not floating point accounting. Original tokens/cost fields need separate null/unknown; when one model omits cache categories, do not silently assume zero. Schema migration backward/forward behavior must be tested.

## Source and authority hierarchy

Provider request event → reported usage fact; provider metric → aggregate/reconciliation corroboration; status line → ephemeral projection; hook → correlation semantics; provider billing → reconciled invoice/usage fact where authorized; git/CI/test evidence → outcome; pricing table → estimated conversion and counterfactual. Conflicting sources remain distinct and trigger explainable discrepancy diagnostics.

## Test cases

Replay, dedup, data gaps, duplicate exporters, cumulative resets, late arrival, causality across mesh, absent billing API, subscriptions, token categories, rate-card drift, observed zero vs no data, privacy, auth, Pages projection allowlist.
