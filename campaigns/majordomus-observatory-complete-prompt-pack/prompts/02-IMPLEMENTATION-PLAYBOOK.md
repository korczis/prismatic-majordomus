# IMPLEMENTATION PLAYBOOK — thin, complete vertical slices

## Principle: run-time truth, presentation artistry
Existing canonical capabilities and monitor -> typed projection -> transport -> Director -> scene renderer. Do not implement a parallel mesh monitor, independent database, or second JavaScript business model. Discover actual repository paths before editing.

### Phase A: audit and establish baseline
1. Inspect HEAD, git status, upstream issues/PRs as available; do not overwrite work-in-progress.
2. Record current routes, capability registry, web surfaces, event semantics, transport, API/MCP derivation, web asset bundling/CSP, docs/CI/release hooks.
3. Produce audit table: source, contract, freshness, authorization, event identity, available scene, known gaps, evidence path.
4. Choose smallest available real vertical: preferably mesh peers and monitor events, but only if code proves they exist. Declare source limitations.
5. Run baseline focused tests and save commands/results.

### Phase B: canonical projection
Rust structs (conceptual only; adapt to actual type system): `ObservatorySnapshot { schema_version, observed_at, sources, entities, relationships, metrics, cursor, completeness }`; `ObservableEntity { canonical_id, kind, label_redacted, status, last_observed_at, provenance }`; `ObservedRelationship { id, from, to, kind, evidence_ids, confidence_class }`; `ObservedMetric { source, unit, value: Option<Number>, window, quality }`; `ObservatoryEvent { canonical_event_id, stream_cursor, event_time, ingest_time, source, event_kind, affected_entities, payload_projection, visibility }`.
- Opaque stable canonical IDs. Distinguish event-time from observation-time and ingestion-time.
- `Unknown`, `Stale`, `Unavailable`, `Partial`, `Fresh` distinct.
- No invented edges, peers, sessions, activity, prices, model usage, costs, cache statistics or topology.
- All cross-links from existing canonical resolver; invalid links should not be presented as functional.
- Projection respects actor authorization and server-side allowlist BEFORE caching/streaming.
- Deterministic sorting, versioned compatibility and bounded serialized payloads.

### Phase C: minimal screensaver route and genuine two-scene implementation
- Route `/ui/screensaver`; canonical route metadata; mode `kiosk`, scene IDs; query parameters never grant privileges.
- `Mesh Cosmos`: real node identities/relationships; if relations unknown render disconnected points rather than fabricated edges. Stable seeded layout from IDs, anti-jitter. Node loss is only inferred from evidence-backed freshness rules; no false 'offline' from silence unless protocol defines it.
- `Event River`: real monitor events with idempotent event->pulse mapping. A quiet system uses ambient nonsemantic animation and an accurate visible 'quiet/no events' status. Event bursts aggregate with count displayed.
- Implement small top-level status strip with LIVE/REPLAY/STALE/UNAVAILABLE; LIVE only when source meets heartbeat/freshness threshold. Avoid exposing secrets in DOM or canvas labels.
- First gate: trace one real event from producer -> projection -> stream -> renderer -> inspector; prove it.

### Phase D: Director and remaining conditional scenes
Build discoverable typed scene descriptors, unique stable IDs, canonical order, source requirements, disclosure policy, performance tier and initialization/disposal. `Director` uses deterministic policy: eligibility filter -> event significance -> priority / fairness -> min dwell and max dwell -> transition cooldown -> scene.
- Intent Nebula: only actual intent/milestone/issue/execution/evidence nodes and source-backed relationships.
- Token Spectrum: only actual usage telemetry, explicit units, estimated vs reconciled; missing provider -> truthful unavailable explanation.
- System Pulse: time-windowed genuine health, event rates and latencies.
- Delivery Tree: verified code/test/build/release evidence, avoid treating inferred completion as proven release.
- Cooperation Choreography: source-backed agent-peer interaction only; no arbitrary animation implying messaging.
- Ambient Intelligence: artistically dynamic even without traffic, but ONLY ambient decorative motion; data labels truthful and stable.
Allow unsupported scenes to be disabled with reasons rather than showing fake charts. Eight scene descriptors required, fully live scenes conditional on real sources.

### Phase E: art direction and controls
Instance-mode p5.js preferred, locally bundled pinned version; 2D or WEBGL selected after profiling. One render scheduling owner. Camera/orbit/particle trails/controlled glow/transitions; avoid endless allocations, context loss leaks and giant label count. Use canonical design tokens plus derived art semantics, not disconnected palette. Inspector overlays explain visual mapping/provenance, links.
Controls: scene previous/next, auto/manual, pause, quality, motion reduction, fullscreen (user gesture), overlay and pointer inspection. Esc preserves browser expectations. 1080p, 4K, safe zones, anti burn-in slow drift, no forced brightness manipulation unsupported by browser.

### Phase F: reliable streaming
Prefer existing subscription/monitor architecture. Support initial authorized snapshot + cursored events; use existing event transport if available; SSE or WebSocket only if justified; fallback bounded cursor polling. Gaps -> snapshot reconcile; duplicates -> idempotent; out-of-order -> policy; queue overflow -> count/coalescing indicator. No network requests in animation frame. Bounded queues and backoff with jitter. Clear event handlers, timers, subscriptions, buffers, textures on scene dispose.

### Phase G: governance and ship
Document new contracts and extension mechanism, update .ai only where existing governance requires, add executable checks, update derived docs and web surfaces, run smoke/E2E/property/browser/security/performance tests. Ship staged commits and releases only with verified repository permissions; preserve branches and safety gates. Capture genuine screenshots from running implementation. Never claim deployed if not deployed.

## Scene lifecycle example
`init(context)` -> `ingestSnapshot(data)` -> `applyEvent(event)` -> `update(dt)` -> `render(frame)` -> `suspend/resume` -> `dispose()`. Scene must not own an independent authoritative copy of repository state. Internal animated particles and coordinates are ephemeral presentation state only.

## Directed scheduler example
Every eligible scene gets a bounded opportunity in a fair rotating window; high-priority event may interrupt after a minimum dwell when severity warrants; incident conditions may pin a diagnostic scene but allow momentary recap; manual lock trumps ordinary events, not crucial health signaling; return to auto on kiosk reset. A scene that throws is quarantined while others continue. Record selection reasons in an inspectable, bounded decision trace.

## Stages of completion
P0 reproducible real source; P1 route plus two genuinely-live scenes plus rotation; P2 additional conditional scenes; P3 art and kiosk reliability; P4 docs/evidence/releases. Do not hide unfinished stages under screenshots or synthetic demos.
