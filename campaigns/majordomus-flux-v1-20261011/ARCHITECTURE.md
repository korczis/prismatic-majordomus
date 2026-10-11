# Flux Architecture — implementation constraints

## Canonical flow
```
canonical inputs & versioned dependencies
  -> existing discovery and capability executor
  -> typed Flux projection envelope {value, provenance, fingerprint, freshness, permission scope}
  -> deterministic fragment/component renderer (extend `cockpit/html.rs`)
  -> cached full page or fragments / conditional HTTP response
  -> optional live subscription / paginated query
  -> real Cockpit, /ui/*, docs, Pages
```

Never let a template query the repository directly. Query through existing canonical capabilities, ideally request-scoped batch/snapshot composition with trace and cache.

## Typed contracts to reuse or extend
- `ProjectionDescriptor`: id, source dependencies, semantic version, data/schema reference, rendering targets, cache invalidation policy, freshness contract. Derived from existing declaration where possible.
- `ProjectionSnapshot<T>`: value, source revision/digest, generation fingerprint, obtained_at where meaningful, permission scope, explicit availability, evidence link.
- `Fragment`: stable ID, representation, dependencies, ETag/fingerprint, safe render entrypoint, keyboard-compatible URL and no-JS fallback.
- `PageQuery`: filter, canonical sort, stable cursor/limit, snapshot revision. No random offset ordering, no arbitrary infinite datasets.
- `LiveEvent`: source identity, stream id, sequence/cursor, resource identity, event kind, timestamp, payload version; resync contract.
- `CachePolicy`: allowed scope, inputs, TTL only if appropriate, invalidation triggers, size/retention and privacy constraints.
These are *conceptual* names; discover actual equivalent types and extend them. Avoid introducing pointless wrappers.

## Presentation
Reuse Rust typed HTML builder and Flowbite/Tailwind tokens. Alpine.js for progressive interactions; Cytoscape.js only for graphs, ECharts only for time series. Consider Askama/Tera/minijinja only if measured rendering/maintainability improvement outweighs dual-renderer complexity; document ADR if selection changes architecture. Prefer no new runtime service.

## Render vs live
Static projection: generation-time data + provenance; canonical freshness at publication. Dynamic projection: request snapshot then WebSocket events relevant to the entity, with bounded replay, reconnect and monotonic sequence. Disconnection => visible stale indicator, not an eternally green badge. Page load must not await an event stream.

## Invalidation
Dependency graph keyed by canonical identity, source digest, projection/generator version, configuration affecting bytes and authority boundary. Single-flight per identical key. Atomic staging and publish. Cache misses and failures must not be disguised as hits. Permissions checked at request time as needed, even if public data has cached bytes.

## HTTP
Use strong ETag/If-None-Match, conditional GET and hashed immutable asset caching where correct; private/sensitive responses no-store or scope-safe; cache variation includes representation/locale/scope if relevant. Never cache streaming handshake as a page. Avoid custom HTTP machinery when current router or existing tested dependencies suffice.
