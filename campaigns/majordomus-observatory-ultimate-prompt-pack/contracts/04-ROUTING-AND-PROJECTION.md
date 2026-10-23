# Normative Route, API and Capability Integration Contract

## Routing decision
- Canonical operational route: `/ui/observatory`. Route `/ui/obsrvatory` is a typo alias ONLY if cheap, preserving correct canonical URLs, and should 308 redirect to canonical unless existing router prevents it.
- `/ui/kiosk` is a **preset** of the same Observatory renderer and state, not a second app, router or store. Query parameters may request `scene`, `auto`, `quality`, `motion`, `density`, but cannot grant authorization.
- `/ui/screensaver` legacy route: redirect to `/ui/kiosk` while preserving safe allowlisted presentation parameters. Do not create parallel renderer.
- Use canonical web UI registry, route discovery, menu/navigation, docs and tests. No duplicated route definitions.
- Provide safe navigation out of kiosk by keyboard/pointer (kiosk not security confinement).

## Projection boundary
`Canonical capabilities/monitor → authoritative runtime adapter → typed authorized Observatory projection → shared snapshot+delta transport → director → scene renderer`.
Perform authorization and redaction before projection serialization for both snapshot and streaming paths. Never rely on client masking.
Do not add an independent domain database, event log or duplicated state authority. Discovery of actual available data sources must precede design. Capability names are hypotheses until verified in checkout.

## Suggested contracts (adapt to repository)
```rust
struct ObservatoryProjection {
    schema_version: String,
    projection_version: String,
    generated_at: DateTime,
    policy_id: String,
    sources: Vec<SourceStatus>,
    entities: Vec<ObservedEntity>,
    relationships: Vec<ObservedRelationship>,
    metrics: Vec<ObservedMetric>,
    watermark: Option<OpaqueCursor>,
}

struct SourceStatus {
    capability_id: String,
    coverage: Coverage, // complete | partial | unavailable
    freshness: Freshness, // fresh | stale | unknown
    last_observed_at: Option<DateTime>,
    reason: Option<String>,
}
```
Contractual meanings of EMPTY, IDLE, STALE, GAP, PARTIAL and UNAVAILABLE must differ and be tests. Unsupported capability disables ONLY dependent subscenes, preserving healthy portions.

## Single origin of derived values
- Rust typed definitions or existing canonical schema must own domain contracts.
- If front-end bindings need generated TypeScript/JSON Schema, generate and validate no drift.
- Derived OpenAPI, MCP or CLI projection only if repository architectural policy supports it; do not blindly multiply endpoints.
- Source-to-rendered mark inspector must be authorized and bounded in size.
- Deterministic stable ordering of scene and entity inventories; explicit user sorting may override presentation only.

## Auth and non-public data
- Private mode adheres to authenticated identity and scopes.
- Public mode is a separate allowlisted server projection, not a URL flag that weakens auth.
- Captured/replayed real events can contain sensitive information: authorize and redact at capture AND presentation as applicable.
- Policy changes require stream reauthorization and cache invalidation.
- Enforce same confidentiality on errors, logs, WebSocket/SSE payloads, URLs and screenshots.
- Provide safe local/offline mode when public authorization unavailable rather than leaking topology.
