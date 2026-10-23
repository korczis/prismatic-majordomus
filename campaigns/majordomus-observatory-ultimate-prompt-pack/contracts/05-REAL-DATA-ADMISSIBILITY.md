# Real-data-only admission and scene eligibility

## Production rule
Every operational claim rendered in production must be backed by an observed canonical record or a reproducible, explicitly tagged computation over such records. **No synthetic nodes, event traffic, fake usage statistics, random agent activity, fabricated commits, random edges or dummy peers.** Inferred attributes are hypotheses with method, quality and confidence where meaningful, not facts. Estimated economics is clearly estimated, with units and pricing source.

## Decorative exceptions
Procedural background particles, camera movement, interpolation, shader noise and ambient geometry may use randomness, but must be visibly semantically separate from data-bound nodes, connections and events. When no source exists, show a truthful no-data composition, never a generic 'all systems green'. Art motion must not be mislabeled as realtime work.

## Capture/replay
Marketing replay is permitted only using genuinely recorded events with authenticated capture manifest: capture time/window, source capabilities, original IDs, transformations, redaction policy, source availability, provenance and replay clock. Prominently label `RECORDED REPLAY` and preserve original event time. Do not label replay LIVE. If no suitable real capture is available, show `NO PUBLIC REPLAY AVAILABLE`; synthetic fixtures remain test-only and visibly marked TEST FIXTURE.

## Machine-readable admissibility
For each signal require:
- source capability ID, concrete source record IDs or exact aggregate definition;
- observed_at and occurred_at when supplied, explicit time basis;
- transform ID and version, input/output units;
- classification OBSERVED | DERIVED | INFERRED | ESTIMATED;
- freshness and completeness status;
- visibility policy and source authorization;
- scene/mark mapping version and bounded inspector link;
- evidence URI or explicit reason why it cannot be disclosed.

If an admissibility requirement fails: do not emit semantic visual signal; emit structured diagnostic and mark scene degraded/unavailable. Never silently coerce missing metric to 0 or unknown identity into named entity.

## Bounded proof fixture vs production
Test fixtures may synthesize events only in a test harness, blocked from production injection by distinct test-only module, artifact path and feature gate. Browser E2E should additionally use captured real data fixtures where allowed. 'Demo mode' means real redacted replay, never artificial production activity.

## Acceptance drills
- Disconnect all sources: scenes show explicit status, no fake events.
- Inject event with missing lineage: no semantic mark and actionable diagnosis.
- Remove auth midstream: immediate cancellation and no protected cached bytes exposed.
- Real event affects correct mark, searchable by ID in inspector, with trace and screenshot evidence.
- Offline playback from actual capture displays RECORDED REPLAY timestamp + provenance.
