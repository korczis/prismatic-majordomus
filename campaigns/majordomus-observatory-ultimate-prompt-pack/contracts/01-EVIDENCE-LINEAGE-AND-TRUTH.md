# Evidence lineage / no-fake-pixels contract (V2 normative)

## Proof obligation
For every operational visual signal, generate a traceable chain:
`canonical capability → observed record → typed projection → transformation definition/version → scene signal → rendered mark → inspector`. Every edge must be navigable where authorization allows. A scene must not claim LIVE merely because a stream socket is connected.

## Suggested typed contract, adapt to verified Rust types
```rust
struct SignalEvidence {
  signal_id: StableId,
  source_capability: CapabilityId,
  source_record_ids: Vec<RecordId>,
  source_schema_version: SchemaVersion,
  observed_at: Timestamp,
  event_time: Option<Timestamp>,
  transform_id: StableId,
  transform_version: Version,
  input_digest: ContentDigest,
  output_digest: ContentDigest,
  classification: DataClass, // Observed | Derived | Inferred | Estimated
  quality: DataQuality, // Fresh | Stale | Partial | Gap | Unavailable
  authorized_view: VisibilityClass,
}
```
Digest is for stable correlation/determinism, NOT proof of truth or permission to expose sensitive inputs. Avoid hashes of low-entropy secrets. Use existing canonical IDs and trust model. Never infer association from time adjacency alone.

## Mandatory invariants
- Each visual semantic mapping declared in a **visual semantics registry**, with input type, output geometry, threshold/unit, transform version and interpretation; no duplicated mapping descriptions in frontend/docs.
- `Observed` ≠ `Inferred` ≠ `Derived` ≠ `Estimated`. Inference supplies hypothesis and calibrated confidence or no number; no silent conversion into fact.
- Every visible metric displays meaningful unit and interval, freshness and source in inspection. Missing => unavailable, never zero.
- For live edges demand source-backed relationship record. Spatial proximity may visually group but must not draw link-like semantics without explicitly labeled grouping.
- Aggregates preserve exact count, window and omitted records; no unlabeled event downsampling.
- Per-source watermarks distinguish collected-at, occurred-at, projection-generated-at. Track clock skew and monotonic local elapsed for timeouts.
- Replay uses previously observed capture with capture manifest and original event IDs, authorization/redaction applied before exhibition.

## End-to-end proof harness
Implement a repository-compatible test or diagnostic command that selects a real or approved recorded event ID and prints: source capability, source record identity, transform chain, scene binding, visual effect category, frontend receive timestamp, pixel/display confirmation instrumentation, drilldown URL, and redaction status. Screenshot cannot alone prove source-to-pixel lineage.

## Fail-closed behavior
Unknown provenance prevents operational animation. Source unavailable = diagnostic/ambient, not simulated payload. Missing capability disables semantic scene portions. Unsupported scene metadata is an actionable validation failure. Error logs must not echo private raw payloads.
