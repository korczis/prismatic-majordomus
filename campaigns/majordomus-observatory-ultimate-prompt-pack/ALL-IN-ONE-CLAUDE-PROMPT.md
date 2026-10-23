# MAJORDOMUS OBSERVATORY — ULTIMATE SINGLE-PASTE EXECUTION PACK

**Canonical:** `/ui/observatory` · **Kiosk:** `/ui/kiosk` · **Legacy:** `/ui/screensaver` → kiosk.

**Priority:** newest ULTIMATE contract below > compatible V2 content. Every operational pixel is backed by real data. Do not implement a mockup.

---

# ULTIMATE MISSION OVERRIDE — enforce on top of V2

You are implementing **Majordomus Observatory**, a canonical, traceable, realtime generative art installation in the existing `korczis/prismatic-majordomus` repository. Do not merely design or emit another prompt. Deliver working software, approved integrations, tests and release evidence.

## Contract precedence
1. Actual repository governance/security, user approvals and truth of the running system.
2. This ULTIMATE override and the `04–06` normative contracts.
3. V2 proof, consistency, director and test contracts.
4. V2 remaining architectural/art/reference detail.
If conflict, choose truthful data, security, working small slices and current repository contracts over speculative scope.

## Canonical routes
`/ui/observatory` = canonical discoverable interactive route; `/ui/kiosk` = unattended preset **same app/data/scene renderer**; `/ui/screensaver` = backward-compatible redirect; `/ui/obsrvatory` = optional typo redirect. No duplicate UI apps. Share auth, registries, scene selection, generated schema, monitoring and design tokens. Routes must appear through current canonical derived UI discovery and docs.

## Primary features
1. Real-data-only visual grammar with durable source→record→typed transform→scene signal→mark proof and provenance inspector.
2. Eight conditional distinct scenes: Mesh Cosmos, Event River, Intent Nebula, Token Spectrum, System Pulse, Delivery Tree, Cooperation Choreography, Ambient Intelligence.
3. Reliable cursor-safe live stream or existing server equivalent, resnapshot recovery, bounded queues, honest stale/gap states.
4. Deterministic fair Director, activity-aware schedule, incidents, cooldowns, preloading, safe transitions, graceful missing capabilities.
5. Cinema-grade art direction and a 24/7-tolerant kiosk experience with 1080p/4K evidence, accessible reduced motion and adaptive rendering.
6. Server-authorized redacted public projection and genuinely recorded replay only; zero fake operational telemetry, even marketing/demo.
7. Canonical derived route/docs/schema/CLI/MCP exposures **only as supported by verified repo architecture**, enforcement, tests, incremental release.

## Order of execution
- Audit checkout, record revision. Read `.ai/`, AGENTS, ADRs, existing UI/routing, monitor/mesh, security, assets, CI and docs. Produce source/capability/trust map.
- Land `/ui/observatory` minimal route with a real authorized state projection, prove G1→G2 end-to-end source-to-rendered mark; commit after validation.
- Add second scene and Director; ship accessible usable `/ui/kiosk` using same renderer. Show honest offline/empty.
- Harden stream retry/replay/gap and interruption; test no leaks and no fabricated activity.
- Extend to remaining scenes **only if verified capabilities are present**; otherwise render explicit unavailable diagnostic and log missing source, no mocked semantics.
- Harden privacy, art composition, browser/perf, documentation, generated artifacts and test matrix.
- Run all repo gates; release/merge/deploy only using authorized repo workflow; verify actual deployed endpoints and provide measured evidence.

## Non-negotiable stop/budget rules
Never build a new general platform, custom event DB, speculative plugin marketplace, rewritten frontend, new provider integrations or new transport protocol to satisfy this feature. The moment scope explodes, return to two real scenes and end-to-end proof. Do not mutate repository governance simply to silence failures. Do not declare pass for unrun tests. Public unsafe = closed. Unknown capability = unavailable. Keep decisions documented in repo with actual evidence, not promises.

## No question loops
Make reasonable implementation decisions using existing conventions; ask only when blocked by credentials, safety-critical approvals or external permissions. Do not wait for aesthetic choices. Work in small independently verifiable increments and coordinate concurrent agents through existing approved mechanisms.

## Report format
For every slice report revision/commit, observed functionality, exact data source and trace, commands and test outcome, current blockers. At end include gate manifest G0–G7, screenshots, perf figures with environment, deployed URLs if verified, and remaining defects. **Never claim shipped if merely planned.**

---

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

---

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

---

# Unattended Hall LCD / Kiosk Operations

## Mode hierarchy
- `/ui/observatory`: full interactive Observatory with inspector and links.
- `/ui/kiosk`: same renderer, automatic scheduling, UI auto-hide, high-legibility composition, recoverable operator controls.
- `/ui/screensaver`: compatibility redirect only.
- `/ui/obsrvatory`: optional typo redirect; only if easy and tests pass.

## Installation constraints
Support 1920x1080 and 3840x2160 landscape, dynamic DPR caps, typography readable at 3–5 m, safe areas, overscan, large status glyphs, and low-noise branding. Avoid flashing and high brightness defaults. Add low-risk slow composition offsets to reduce persistent LCD image retention; do not promise burn-in prevention. Browser fullscreen requires gesture or genuine managed Chromium kiosk flags; do not spoof browser permissions.

## Session resiliency
- No busy loops or unbounded retry storms.
- Distinguish offline, disconnected, stale, no events and healthy idle.
- Implement streaming -> bounded cursor-poll fallback when canonical transport unavailable.
- Re-authenticate after token/policy changes; fail closed for unauthorized.
- Automatically recover from backend restart, network flap, browser sleep, WebGL context loss, scene exception, tab visibility changes and resizing.
- Display small, readable connection state, last verified observation and ongoing degraded status.
- Keep service worker/cache and browser asset updates coherent with repository conventions. No arbitrary self-reload that destroys capture state.

## Ops and DX
Document Linux systemd/Chromium kiosk and macOS launch options only as justified by existing packaging and supported environment. Prefer thin launch command in majordomus binary when supported; no new daemon solely for Observatory. Provide startup prerequisites, auth provisioning, privacy checklist, crash recovery, hardware sizing, measured resource budgets, and runbook with observable diagnostics.

## Art quality review
Capture real UI screenshots at 1080p and 4K and verify readable labels from a scaled-down thumbnail, non-overlapping overlays, stable composition, quiet-state aesthetics, no epileptogenic flashing, legible missing-data states and clear LIVE/REPLAY status. Art review is human check, not fake machine assertion.

---

# Production release gates and evidence manifest

## Mandatory release gates
G0 Repo audit: actual checkout revision, currently implemented routes/capabilities, auth model, asset pipeline, available hardware. State unknowns, not guesses.
G1 Data proof: at least one real source event with source ID, canonical record and correct authorized projection.
G2 Pixels proof: that event reaches browser data collector, scene binding and displayed mark, with timestamp/cursor, trace and reproducible demonstration.
G3 Continuity proof: two art-quality scenes rotate; fairness/interrupt/cooldown assertions; idle and partial inputs truthful.
G4 Recovery proof: disconnect, restart, source gap, cursor expiration, context loss, scene failure, memory caps.
G5 Privacy proof: public data minimization, replay label, no private leakage in snapshot, delta, inspector, error or logs.
G6 UX/perf proof: screenshots 1080p + 4K, accessible controls, 30/60 fps targets tested on specified hardware; no unsubstantiated performance claims.
G7 Deployment proof: build, tests, canonical gates, documentation, version/bump as repo requires, permitted merge/release/deploy, endpoint and verification of deployed code.

## Failure behavior
Never mark a gate PASS without exact command, result, environment and evidence path. `NOT RUN`, `BLOCKED`, `FAIL`, `PASS` are distinct. A checked box cannot stand in for execution.

## Proposed manifest
```json
{
  "revision": "actual SHA, never invented",
  "routes": ["/ui/observatory", "/ui/kiosk"],
  "capture": {"type": "real-recorded-or-live", "source_id": null, "event_id": null, "sanitized": true},
  "evidence": {"lineage_report": null, "browser_trace": null, "screenshot_1080p": null, "screenshot_4k": null},
  "gates": {"G0": "NOT RUN", "G1": "NOT RUN", "G2": "NOT RUN", "G3": "NOT RUN", "G4": "NOT RUN", "G5": "NOT RUN", "G6": "NOT RUN", "G7": "NOT RUN"},
  "deployed_url": null
}
```
Replace placeholders with actual values via existing CI/reporting conventions. This is a *shape example*, not a fabricated report.

## Must-have tests
- source-to-mark provenance property/integration test;
- shuffled source enumeration -> same graph/order/scene result;
- duplicate and late events / clock skew / stream restarts;
- stream resume expired cursor -> consistent resnapshot;
- bounded scene/event memory, resource disposal under repeated transitions;
- display fallback and reduced motion;
- authorization revoked during SSE/WS with no leak;
- replay/capture with real event manifest + redaction;
- no false health for missing source; scene prerequisites accurate;
- route aliases redirect and share one render implementation;
- CI generated docs/schemas drift gate;
- measurable browser/perf integration fixture and longer optional soak.

---

# Observatory cinema / aesthetic system

## Scene grammar
Every scene contains: a clear focal region, no more than a handful of prioritized semantic callouts, explicit observed-vs-decorative layers, legible source freshness and mode identity, stable spatial continuity, and graceful handling when data is sparse. Describe color via canonical existing design tokens and semantic intents; do not duplicate palette.

### Mesh Cosmos
```
┌──────────────────────────────────────────────┐
│ MAJORDOMUS // OBSERVATORY          ● LIVE     │
│                                              │
│       ○──────◉       ∙                       │
│      / \    /  \                              │
│     ◉───◎──●────◉       (edges only if real) │
│              \     ○                        │
│                                              │
│ MESH • observed N / eligible sources         │
└──────────────────────────────────────────────┘
```
Actual peer identities and edges from verified canonical sources. Ambient stars are clearly background.

### Event River
```
┌──────────────────────────────────────────────┐
│ EVENT RIVER                    ● LIVE        │
│                                              │
│      · • · · → ·· •   ⇢ ⇢                     │
│     ────────→──────────────                  │
│       · ·   ◇    · ·                         │
│                                              │
│ last observed HH:MM:SS • events/window       │
└──────────────────────────────────────────────┘
```
Particles are tied to real event count or explicitly summarized aggregates.

### Intent Nebula
Use canonical intent → issue → execution → evidence relationships, omit if not supported. Spatial proximity is visual layout, not causality.

### Token Spectrum
Token readings must have known origin, token type, window, units, provider and estimated/billed distinction. Without telemetry, scene is unavailable, not random equalizer bars.

### System Pulse
Use actual sampled time series and clear stale/unknown states. No synthetic 'healthy' waveform.

### Delivery Tree
Only recorded commits, PRs, released artifacts, tests and evidence with real links. Do not confuse git history with deployment success.

### Cooperation Choreography
Only explicit interactions between authenticated/identified peers or linked execution records. Never infer communications based purely on time coincidence.

### Ambient Intelligence
Beautiful low-frequency purely decorative composition paired with honest factual status and last real observation. Motion is not 'work'.

## Transitions
Crossfade safe baseline. Shared-entity morph only with matching canonical stable IDs. Priority interrupts preserve legibility. Up-next preloading must not double-subscribe backend or leak GPU resources. Director transitions tested as deterministic with controlled monotonic test clock.

## Artistic quality definition
A hall viewer sees shape and hierarchy within 3 seconds, notices meaningful movement within 20 seconds, understands whether LIVE or REPLAY immediately, and can step closer to inspect verifiable entities. Screenshots at reference resolutions must be included in final report, not generated art sold as UI captures.

---

# CONSOLIDATED V2 FOUNDATIONAL MATERIAL

# MAJORDOMUS OBSERVATORY V2 — SINGLE-PASTE EXECUTION PACK

**V2 instructions override earlier contradictory wording. Treat all repository interface references as audit hypotheses until verified in current checkout.**


---

## V2 ADDENDUM: prompts/03-IMPLEMENTATION-ORDER-AND-STOP-RULES.md

# Strong execution policy: deliver vertical value and stop scope expansion

## Mandatory build order
1. **Prove source:** audit current repo (not claims in old conversations). Determine one true mesh source and one true activity source; authorization/redaction model.
2. **Prove end-to-end:** canonical snapshot+delta to exactly one scene, one real event with provenance inspector and browser instrumentation; no generic scene framework until this works.
3. **Prove reliability:** reconnect/resume/gap + error UI and bounded queues in one scene.
4. **Prove rotating two-scene gallery:** scene lifecycle and deterministic fair Director. Hard requirement: actual live or recorded real observations, no synthetic exhibition.
5. **Polish installation:** 1080p/4K typography, fullscreen/kiosk handling, ambient idle, performance/leak test.
6. **Extend only where data exists:** remaining 6 scene renderers; mark PARTIAL or UNAVAILABLE cleanly rather than inventing sources. Avoid arbitrary telemetry adapters.
7. **Cross-surface verification:** docs, derived UI, schema, tests, governance, security, actual deployment where authorized.

## Anti-scope-creep
No generalized plugin marketplace, multi-renderer abstraction zoo, novel transport protocol, new persistent event DB, custom graph engine, rewritten `/ui` framework, speculative AI telemetry connectors, or manual duplicate registries unless justified by actual blockages with measured evidence. Optimize first real scene before expanding.

## Agent orchestration
Separate independent audits, art work and tests only if repository mechanisms support nonconflicting agent ownership. A single integrating owner validates all changes. Preserve atomic commits and working route. Do not use unreviewed blanket admin/bypass options.

## Stop rules
- Source not observed? STOP claiming LIVE and render truthful unavailable state; continue nondependent parts.
- Public projection unsafe? Public mode MUST fail closed; private authorized mode may proceed.
- Proof harness cannot trace source → pixel? Not shippable as live.
- WebGL slow or flaky? Choose Canvas2D fallback and report measurements.
- Additional scene lacks canonical signal? Ship unavailable legend, not dummy data.
- Performance gate unmet? Downscale effects, not integrity guarantees.
- Merge/deploy requires approval? Do not bypass; report block.

## Final reporting
At each slice show **real user-visible outcome** + recorded commands + evidence. Distinguish implemented, verified, blocked and proposed. End report with only existing code/URLs/tests and outstanding gaps. Never claim completed tests based on intent.


---

## V2 ADDENDUM: contracts/01-EVIDENCE-LINEAGE-AND-TRUTH.md

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


---

## V2 ADDENDUM: contracts/02-STREAM-CONSISTENCY-AND-RESILIENCE.md

# Stream semantics, distributed consistency and unattended recovery

## Design audit required
Identify actual source semantics (ordered? per-source cursor? retention? historical query? mesh identity?). Do not declare global exactly-once or global total ordering without proof. Prefer at-least-once delivery with idempotent projection + bounded resync, and explain possible gaps.

## Envelope
Use existing schemas if present; otherwise introduce a narrowly typed event envelope with `source_id`, `source_epoch`, `event_id`, `source_sequence?`, `occurred_at?`, `observed_at`, `cursor?`, `schema_version`, `redaction_class`, `payload_type`. A cursor is opaque and scoped to its source/stream; don't use wall time as cursor unless backend guarantees semantics.

## Recovery state machine
`DISCONNECTED → CONNECTING → SNAPSHOT_SYNC → STREAMING → STALE → RESYNC → STREAMING` with `UNAVAILABLE` distinct from idle. Each transition includes reason, timestamp, last-known cursor and counters. Backoff has jitter and cap. On expired cursor or integrity gap take a fresh authorized snapshot and resume from supported watermark. Deduplicate with bounded ID window. For reordering, only reorder within an explicit bounded/source-specific policy; otherwise annotate uncertainty.

## Storm control
Separate domain event ingestion from 30/60fps drawing. Process capped chunks each frame; batch visual effects without discarding semantic counters. Coalesce only with mathematically correct counts and an explicit aggregation window. Queue limits are configurable, measured and tested. On overflow, never silently resume as though complete: mark `GAP`, resync if possible.

## Hard failure drills
- Server kill/restart while projector runs.
- Wi-Fi/mesh partition and reconnection.
- Source clock skew, late arrivals, identical timestamps, duplicate IDs.
- Truncated cursor retention, schema changes and corrupted delta.
- Extremely chatty source and long quiet periods.
- Public policy change during stream; terminate/re-authorize, never retain older privileged stream.
- Renderer context loss, hidden tab, DPR change, full-screen exit.

## Identity and causality
Canonical node identity comes from verified Majordomus identity source, not labels, IP or topology guesswork. Cooperation arrows require an explicit source-backed interaction/correlation. Causality is not inferred from temporal proximity.


---

## V2 ADDENDUM: contracts/03-SCENE-DIRECTOR-STATE-MACHINE.md

# Deterministic Director + scene lifecycle contract

## Algorithm, not a suggestion
For each eligible scene compute `priority = base_weight + bounded_signal_relevance + fairness_debt - repetition_penalty - resource_penalty`. Exact coefficients belong in canonical versioned policy, not scattered UI constants. Critical operational interrupt is a separate priority class and does not compete with normal art rotation. Weighting is explainable and subject to deterministic tests.

Eligibility = required source available **and** its data fit-for-purpose; `connected` alone insufficient. Scenes can be `FULL`, `PARTIAL`, `AMBIENT_ONLY`, `UNAVAILABLE`. If NONE has valid operational signals, select explicitly labeled ambient/no-signal state.

Minimum dwell, maximum dwell, signal cooldown and recovery hysteresis protect against flashing. Ensure fair rotation: test that any continuously eligible scene is shown at least once within a bounded scheduling horizon under bounded normal event activity. Critical alerts may preempt, but their visible diagnostic status remains even as gallery continues.

## Lifecycle invariants
At most one active expensive renderer per viewport, transition double-buffer only within memory budget. `prepare → mount → activate → suspend → dispose` with idempotent cleanup, cancellation of subscriptions, timers and GPU allocations. Never create a backend stream per scene. Repeated rotations must not increase listeners/subscriptions/memory asymptotically.

## Inspector and manual control
Keyboard/manual lock overrides normal rotation but not mandatory safety notices. Director `explain` returns eligibility, weights, cause for switch, last shown, next candidates and source quality. Persist only appropriate non-sensitive local preferences.

## Deterministic test cases
Same sequence/capability snapshots/seed => identical schedule and scene transforms; replay with distinct refresh rate => identical semantic event counts; unavailable source => no fake activity; repeated incident => bounded interruption; scene failure => isolated failure and fallback; all available scenes => no starvation; quality reduction never alters semantic counters.


---

## V2 ADDENDUM: design/02-STORYBOARD-AND-SCREEN-GRAMMAR.md

# Art direction V2 — physical installation storyboard

## Screen composition
Large-screen hierarchy: 70–80% generative visual field; 10–20% restrained narrative context; optional status strip for source freshness, LIVE / RECORDED REPLAY / NO SIGNAL and scene title. Avoid small dense text. Display modes 1920×1080 and 3840×2160, plus portrait adaptability if evidence supports it. Art is art; operational semantics remain inspectable.

## Scene sketches (conceptual, not fake screenshots)
```
MESH COSMOS                    EVENT RIVER
       o       o              >>>>  . .  >>>
   o -- O -- o                 ╲═══════╱
        |   o                    event lanes
        o                     Count / window / sources

INTENT NEBULA                  SYSTEM PULSE
  Intent                      .--.  --.-  .---
   ├ Milestone                  /  \/    \/
   │  └ Issue                 health / latency / events
   └ Evidence                labels + units + freshness

DELIVERY TREE                  COOP CHOREOGRAPHY
    release                       agent A
    /    \                         │ confirmed event
 tests  evidence                   ▼
    |       |                      agent B
 commits  gates             no inferred communications
```
**Do not show edges or metrics without records.** Scene may animate idle geometry only if operational meaning is explicitly absent.

## Screen grammar
- Colors convey semantics by canonical design token: normal activity, degraded, critical, unavailable, replay. No fresh semantic palette in JS.
- Motion is layered: ambient (decorative), operational (source-backed), attention (warning), transition (director). Never use warning color randomly as decoration.
- Every main scene has a fixed camera-safe title/legend zone and optional inspector; rotate/burn-in shift gently without moving status offscreen.
- Director crossfade 0.8–2.0s initial design hypothesis, tune on reference hardware. Interruptions prioritize readability over spectacle.
- Legibility walk test: observer at 3–5 meters identifies scene, LIVE vs REPLAY, mesh health in ≤30s, and never mistakes no-signal for normal.
- Respect reduced-motion and WCAG-relevant controls; provide non-canvas data alternative from same projection.

## Art review gates
Capture actual running screenshots AND 15–30 second recordings of live, idle, partial/stale, recorded replay, source outage. Review at hall distance on realistic 4K and 1080p setup, including night brightness. Record actual notes and corrective changes. Source audit and visual review are separate gates; attractive pixels cannot rescue dishonest data.


---

## V2 ADDENDUM: verification/02-EXECUTABLE-ACCEPTANCE-AND-EVIDENCE.md

# Executable acceptance protocol (V2)

## Definition of done: no green checkbox without artifact
Each gate requires reproducible command, environment/reference hardware, raw output location, status and linked failing issue if not satisfied. If no live source available, report `BLOCKED_UNAVAILABLE_SOURCE` and show truthful no-signal rendering; do not mark live integration proven with fixtures.

### G0 canonical audit
Evidence of actual source code locations, available capabilities, route, event semantics, permission model, design tokens. Produce `audit/source-map.md` with claim vs observed code, cited file path/line/commit. Prior discussions are hypotheses, not verified implementation.

### G1 real source → pixels
Choose one genuine event from working Majordomus. Record canonical ID and event provenance. Show ingestion, transformation, network transport and render instrumentation. Inspector links to source where authorized. If impossible, block instead of inventing traffic.

### G2 reliable director
Run virtual-time deterministic unit tests for scheduling; scene fairness, priority interrupt/cooldown, readiness-driven skipping, manual lock and recovery. Confirm each scene initialized/disposed exactly once per lifecycle and no multiplied stream subscriptions.

### G3 stream failure
Automated browser integration: reconnect with persisted cursor, duplicate event, cursor expiration, overflow, server restart, source disabled, malformed envelope, authorization change; correct freshness/error semantics and zero fabricated event pulses.

### G4 privacy and provenance
Server-side property tests for redacted snapshot/delta/inspector/replay/cache/error pathways. Public policy must survive nested and newly added fields, not merely a blacklist of known strings. Prefer explicit allowlist schemas. Verify authorization on drilldown independently.

### G5 24/7 readiness
Run reproducible shorter soak test in CI and optional 24h physical kiosk soak on specified hardware. Report fps p50/p95, frame time, heap and GPU resource drift, CPU, reconnect count, scene churn, leaks and errors. Long-run PASS only after actual run.

### G6 art and hall UX
Independent visual acceptance: legibility from distance, composition, live/replay/partial labels, scene differentiation, clarity under inactivity, smooth scene changes, high-DPI screenshots from running system. These are manual signoffs with evidence, never automated aesthetic claims.

### G7 integration & shipping
Existing quality gates, generated schema/docs/UI drift checks, reproducible build, security audit, branch/release protections. Any deployment claim contains a real URL and a postdeploy smoke check; permission blocked = explicit blocked result.

## Evidence manifest structure
```yaml
schema_version: '1'
repository_commit: '<actual hash>'
source_audit: 'audit/source-map.md'
gates:
  - id: G1
    status: PASS|FAIL|BLOCKED|NOT_RUN
    command: '<actual command>'
    evidence_paths: []
    observed_at: '<actual timestamp>'
    hardware: '<device/browser/version where relevant>'
    limitations: []
```
Do not substitute fictional manifest values or generated pass counts. If files are produced, verify existence. Have a final matrix of all gates with PASS/FAIL/BLOCKED/NOT_RUN and links.


---

# ORIGINAL CONSOLIDATED SPECIFICATION

# COMPLETE SINGLE-PASTE PROMPT — MAJORDOMUS OBSERVATORY

Read all sections as one integrated directive. Implementation agent must inspect actual repo. In case of conflict, the REAL-DATA-ONLY rule dominates.

## FILE: prompts/00-MASTER-EXECUTION.md

# MASTER EXECUTION PROMPT — MAJORDOMUS OBSERVATORY

You are a senior Rust/web systems architect, generative-art director, SRE and adversarial QA engineer working *inside the actual repository* `korczis/prismatic-majordomus`.

## Read first
Read **every file** in this prompt pack before implementation, including the reference consolidation, sketches, contracts, playbook and verification. Treat them as one integrated specification. No earlier standalone prompt supersedes this unified pack. Follow repository `.ai/`, AGENTS.md, CLAUDE.md, existing doctrines, ADRs and permission boundaries. If they conflict with this pack, document the conflict, prefer current enforceable project policy, and implement the closest safe equivalent.

## Mission
Ship `/ui/screensaver` as **MAJORDOMUS OBSERVATORY / Infinite Living Screens**: a fullscreen, large-LCD, installation-grade, rotating gallery of cinema-quality data art powered by **actual** Majordomus local and mesh activity, with trustworthy freshness/provenance and uninterrupted kiosk operation. It must function as technical visualization, operational awareness, hall installation, demo, and marketing communication. A viewer should get the point from a distance in 30 seconds and a skeptic should be able to inspect evidence underlying every semantically meaningful visual feature.

## Deliver, not just ideate
Inspect -> inventory -> formulate evidence-backed plan -> implement thin vertical slices -> test -> integrate docs/governance -> benchmark -> release/deploy if authorized -> verify -> report exact evidence. Do not stop at speculative design. Avoid endless planning or expanding to unrelated architecture. Fix adjacent defects only when blocking correctness/security/reuse. Prefer deletion of duplicate systems over adapters on top of adapters.

## Mandatory outcomes
1. Canonically registered `/ui/screensaver` route, reachable in UI, appropriate layout, stable URLs.
2. Reusable typed Observatory projection drawing on EXISTING capability/data/event interfaces, with real snapshot and updates; demonstrable mesh and activity scenes as first milestones.
3. Typed discoverable scene registry, scene lifecycle, auto rotating Director with fair scheduling, meaningful event-driven focus and deterministic replay.
4. Eight scene designs and implementations with capability-aware availability/fallback: Mesh Cosmos, Event River, Intent Nebula, Token Spectrum, System Pulse, Delivery Tree, Cooperation Choreography, Ambient Intelligence. No fabricated activity when feeds are absent; scene disabled/degraded with explanation when data absent.
5. p5.js / Canvas2D / WEBGL as justified by repo asset architecture and profiling; no runtime CDN dependency by default; no unrelated UI stack rewrite.
6. Private/public/real-recorded replay boundaries enforced server-side; public cannot expose private prompts, credentials, IPs/hostnames/topology or confidential work unless explicitly allowlisted and authorized.
7. Unattended kiosk: fullscreen user-gesture handling or supported browser kiosk launch, 1080p/4K composition, reduced-motion, anti-burn-in, FPS adaptation, recovery, resource cleanup, offline/stale indications.
8. Cross-link drilldowns to relevant `/ui` entities; diagnostic overlay showing sources, freshness, status, validity, provenance, visual meaning and REPLAY badge.
9. Tests from unit through browser, security, performance, reconnect, soak framework, drift gates; documentation and GH Pages updated according to actual repo rules.
10. Exact final report: changed code, actual data flows, tests, benchmarks, privacy gates, docs, deployment result, evidence and unresolved blockers.

## Product is not a decorative dashboard
A static fake screenshot is NOT done. A pretty animation untethered to observed data is NOT done. Eight unfinished scenes are NOT done. A working projector fed by fabricated events labelled LIVE is a critical failure. A flaky WebGL installation that fails after six hours is NOT done. A beautiful scene hiding a failed mesh is not truthful observability.

## Execution priorities / ordered critical path
**P0** inspect reality, canonical route, real mesh/activity data, explicit demo/live markings. **P1** deterministic real two-scene vertical slice + auto rotation + reconnect. **P2** extensible scene registry + remaining capability-aware scenes + cross-linked inspection. **P3** cinematic polish + 4K kiosk hardening + security + long-run performance. **P4** generated docs/marketing + approved release/deploy. Preserve working increments and quality gates at each phase.

## Autonomy and collaboration
Use existing Majordomus intent/milestone/issue/agent/peer orchestration if implemented; otherwise avoid inventing unsupported workflow. Parallelize only nonconflicting worktrees/owned modules. Never have agents race over shared Git working tree or release configuration. Prefer tiny verifiable commits and phase-specific CI. Follow branch protections and required approvals. Do not bypass checks, approval, auth or security. On blocked deployment, finish locally and report blocker precisely. Do not hallucinate passed checks or shipped releases.

## Acceptance proof
- An event originating in a real capability is observed, attributed, serialized, transported, consumed, rendered and inspectable to its source.
- A new peer, failed node, issue/execution transition, release or tool event is visualized *only where the corresponding source exists and is authorized*.
- A quiet mesh remains artistically alive with explicitly decorative motion but no synthetic live events.
- Source loss visually marks stale/unknown, recovery resynchronizes from cursor/snapshot; duplicate events do not duplicate facts.
- Auto rotation is fair, bounded and responsive to significant events, and cannot become permanently trapped by minor alerts.
- Public output is safe BEFORE reaching browser, including event stream, introspection and caching.
- All eight scenes have genuine behavior or correct availability diagnostics; absence of a provider never magically produces numbers.
- Automated and manual evidence demonstrate performance, cleanup, responsive layout, accessible controls and documented operation.

## Closing report format
`Repository audit | Implemented slices | Canonical model | Event transport | Scenes/director | Design/UX | Security | Benchmark hardware/measurements | Test commands/results | Generated docs/GH Pages | Commits/PR/release/deploy URLs (only if real) | Unresolved risks | Evidence traces`.


## FINAL OVERRIDING REQUIREMENT — REAL DATA ONLY
All production/runtime exhibition modes (including marketing and demo) MUST use real observed live records or clearly-labeled REPLAY of real captured, authorized, redacted records. Synthetic events/metrics/topology MUST NEVER flow to a running exhibition. Synthetic data is permitted ONLY in isolated automated tests and explicitly non-deployable conceptual sketches. Decorative motion does not represent operational activity. When data does not exist, display unknown/unavailable/idle and reason. This overrides any earlier wording suggesting synthetic demonstration.


---

## FILE: prompts/01-REPOSITORY-DISCOVERY.md

# Repository Discovery / Gap Audit

Before design hardens, inspect actual HEAD, branch, commit, working tree, repo governance, existing open work and related PRs. Protect uncommitted user work; do not reset/overwrite it. Current repo code is authority, this prompt pack is not a repo snapshot.

## Enumerate and link evidence
- UI routes and route registry, `/ui`, `/ui/mesh`, `/ui/roadmap`, `/ui/issues`, `/ui/prs`, `/ui/evidence`, `/ui/delivery`, server components and browser assets.
- Data plane: `mesh.peers`, `mesh.events`, `monitor.stream`, source executor, `source::ask_all`, peer/session/work/execution/evidence/usage capabilities, actual schemas and field semantics. Determine which names still exist.
- Stream plane: current monitor API e.g. `/api/v1/monitor` if present, cursor semantics, polling/SSE/WebSocket support, retention windows, event IDs, delivery ordering, auth and backpressure.
- Canonical UI metadata e.g. `web.surfaces`, capability registries, OpenAPI/MCP derivation, typed Rust schema generation, import paths.
- Design: current token authority (historically discussed `share/design/tokens.yaml`), UI component conventions, asset bundling, CSP, static file serving, JS dependencies, privacy configurations.
- Build/test: cargo workspace, just recipes, actions, release gates, GH Pages deployment, browser tests, fixtures, benchmark facilities, ADR/governance workflows.
- Infrastructure: mesh identity/security, localhost broker/daemon where present, Jetson/remote kiosk deployment affordances, frontend security model, event storage ownership, resource limits.

## Produce audit matrix
`candidate capability | actual source path/command | authoritative field schema | freshness | privacy classification | stable ID | streaming strategy | scene consumers | missing gap | evidence`.

## Resolve contradictions
If historical discussion asserts a capability that no longer exists, note `absent` and choose compatible current mechanism. Never hardcode paths from this document without verification. Never expose nonexistent CLI/API endpoints simply to match previous prose. Do not make up data during gaps. Keep first vertical slice independent of any speculative economics instrumentation.

## Initial proof of transport
Use minimal sample data from **actual** canonical adapter and demonstrate end-to-end IDs + timestamps + source IDs + reconnect behavior before expanding art. Verify permissions and content security policy. Inspect available replay/log semantics and avoid performing privileged operations when just reading.

## Decision memo
Deliver short `Reuse / Extend / Reject` decisions for p5.js vs native canvas/WebGL, transport, schema, scene registration, derived docs, source storage and public redaction. Record trade-offs, avoid gratuitous frameworks.


---

## FILE: prompts/02-IMPLEMENTATION-PLAYBOOK.md

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


---

## FILE: contracts/OBSERVABILITY-CONTRACT.md

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


---

## FILE: contracts/DIRECTOR-AND-SCENES.md

# DIRECTOR AND SCENES — CANONICAL CONTRACT

## Scene registry
Typed descriptors: `id`, `label`, `required_signals`, `optional_signals`, `availability_reason`, `focus_event_kinds`, `default_dwell_range`, `priority_class`, `privacy_class`, `resource_tier`, `inspector_kind`, `fallback_kind`. Derive all UI selectors/help/docs from registry. No duplicate arrays in JS and Rust. Every descriptor has an evidence-backed source binding or explicitly artistic ambient character.

## Scenes and visual grammar
| Scene | Canonical fact source (discover actual) | Artistic projection | Never pretend |
|---|---|---|---|
| Mesh Cosmos | verified mesh peers/topology/health | nodes, source-backed edges, pulses | links, peers, routing protocols not observed |
| Event River | canonical monitor events | flow lines/particles, time bands, event glyphs | traffic when idle |
| Intent Nebula | real intents, milestones, issues, executions, evidence | knowledge graph, animated paths for real transitions | guessed dependencies |
| Token Spectrum | provider-authorized token usage / cache / pricing evidence | spectral bars and trails | estimated equals billed, unknown tokens |
| System Pulse | runtime observed metrics with windows and units | breathing rings, sparklines | healthy by default |
| Delivery Tree | build/test/commit/release proof | growing branches | release without deployment proof |
| Cooperation Choreography | explicit peer-agent events | choreography/pulses | message passing inferred from concurrency alone |
| Ambient Intelligence | honest current health and availability + decorative clock | slow ambient topology-less motion | invented telemetry |

## Visual Truth Contract
A visual mark is either `data-semantic` (traceable source and exact mapping) or `decorative` (no factual claim). Mixed animation needs a legend distinguishing channels. Inspector shows source ID, current staleness, sampling/window, transform, aggregation, links, privacy truncation and evidence availability. Never assign a real-causality explanation to aesthetic randomness.

## Director states
`INITIALIZING`, `DISCOVERING`, `READY`, `PLAYING`, `TRANSITIONING`, `INTERRUPTED`, `DEGRADED`, `RECOVERING`, `SUSPENDED`.

Transitions must be explicitly documented and unit-tested. Supervisor catches scene failures, disposes resources and selects next healthy eligible scene. Do not drive decisions with unbounded asynchronous race conditions.

## Suggested default policy (not fixed inventory)
- Dwell regular scenes circa 30–90s; ambient 60–180s, configurable based on scene metadata.
- Significant event can boost relevant scene priority; impose min dwell, max dwell, interrupt cooldown and fairness window.
- Incident hold has explicit exit condition, not 'be red forever'.
- All durations are defaults and should be benchmarked/adjustable through canonical configuration, not scattered constants.
- Capability loss makes scene degraded/ineligible; display warning and transition safely. Never quietly substitute fake data.
- Auto selects from sources actually available; manual scene selection reveals why source is unavailable.

## Event taxonomy and mapping
Peer observed, peer changed, peer stale, tool invocation, session/execution start/finish, test pass/fail, PR merge, release, monitor health and economic usage are *candidate categories only*. Register mapping only if confirmed actual canonical event or correlated evidence exists; reject extrapolations.

## Explainability
Each director decision includes `event_id?`, `reason`, `eligible_scene_ids`, `selected_scene_id`, `policy_version`, `decided_at`; bounded logging with no private payload. Each scene supports a legend and target canonical entity resolver.


---

## FILE: design/ART-DIRECTION-AND-SKETCHES.md

# ART DIRECTION AND TECHNICAL SKETCHES

## Core brief
'A Semoscena-like living architecture of system data': sophisticated generative art, scientific instrumentation, music-visualizer smoothness, production-grade mission control. View from 3–10 meters on a hallway LCD; clear major message in 30 seconds, technical drilldown up close. Visual inspiration only: space-like constellations, rivers of particles, spectral bands, radial pulse, branching crystal trees, choreographed light and quiet ambient rings. Do not copy copyrighted design or imply real data with illustrative artwork.

## Primary screen / wireframe
```
┌──────────────────────────────────────────────────────────────────────────────┐
│ ◆ PRISMATIC / MAJORDOMUS         OBSERVATORY              LIVE • 00:00 LOCAL │
│ ──────────────────────────────────────────────────────────────────────────── │
│                                                                              │
│                         ·         ◎  node                             ·      │
│                ·───╴○───────╴◉─────┄╴ ○                              ·       │
│         ○    ·             ╲   ⟶  SOURCE-BACKED PULSE                 ·       │
│                    ○────────◎────────○                                       │
│                      ╲     ·                         ·                       │
│                       ◉                            ⋆                         │
│                                                                              │
│       MESH CONSTELLATION                                  SOURCE: VERIFIED   │
│                                                                              │
│ ──────────────────────────────────────────────────────────────────────────── │
│  01 / 08   MESH COSMOS       ● ● ◉ ○ ○ ○ ○ ○     SIGNALS: FRESH / PARTIAL     │
└──────────────────────────────────────────────────────────────────────────────┘
```
All numerals, topology, and statuses in this sketch are PLACEHOLDERS, never runtime facts.

## Director flow
```
Canonical Sources ---> Authorized Snapshot/Event Projection ---> Source Health
                                       |                               |
                                       v                               v
                               Scene Eligibility  <----------- Freshness Policy
                                       |
                                Director / Fairness
                                       |
                                Transition Manager
                                       |
                    ┌──────────────────┴──────────────────┐
                    |                                     |
               Scene Renderer                       Inspector/Legend
                    |                                     |
              Data art canvas                 Provenance/links/status
```

## Varying screen composition
1. Mesh Cosmos: central distributed constellation; pulse trails only for observed events; minuscule secondary labels.
2. Event River: horizontal flowing bands representing real event categories and event rate; aggregate counts.
3. Intent Nebula: relational graph, work clusters, intent-to-evidence trails with clickable evidence.
4. Token Spectrum: spectral ribbons derived from observed token usage; unknown provider -> unavailable.
5. System Pulse: ring geometry based on sampled metrics; never present unknown as green.
6. Delivery Tree: branch growth only tied to proven changes/releases.
7. Cooperation Choreography: intersecting trajectories represent documented agent/peer exchange, not assumed chatter.
8. Ambient Intelligence: gentle breathing particles and rings, with unobtrusive truthful system status; ambient motion has no operational semantics.

## Palette semantics
Colors derived from actual canonical design tokens. Suggested *semantic relationships*, not raw independent hex source of truth: cool cyan for observed healthy/info, neutral silver for unknown, warm amber for degraded, restrained red for proven failure, violet for structured exploration, white for currently selected. Differentiate with geometry/labels for accessibility. Color should convey system semantics, not random decoration.

## Motion lexicon
- Event = one short pulse or count-annotated aggregate, source-linked.
- Persistent node = stable center/mass, low-motion alive-state.
- Verified edge = stable geometric relationship; animation only on sourced interaction.
- Work completion = finite outward ring; avoid fireworks for every log line.
- Missing data = fade/diagnostic hatch, not success pulse.
- Ambient 'breathing' = label as decorative art, independent of telemetry.
- A scene switch = spatial continuity if entities share stable canonical IDs; otherwise crossfade.

## Controls mockup
```
  [◀] [AUTO ON] [▶]   [SCENE: MESH COSMOS ▼] [⌘ INFO] [⛶ FULLSCREEN]
      quality AUTO • reduced motion OFF • LIVE real data • 8 scenes registered
```
Use actual scene count from registry. Auto-hide controls in kiosk, keyboard support and no accessibility regressions.

## Visual and interactive sketch provenance
Earlier in-chat proof-of-feel used a CSS/canvas mini animation with 9 predefined nodes and 14 predefined edges and particle interpolations. That was explicitly a **conceptual illustration only**, not a working Majordomus topology; never transplant those fixed values into the production live view. The reference HTML is an artistic sketch only and contains NO authoritative telemetry.

## Art acceptance
Vary visual rhythm and composition across scenes; ensure legible large-scale hierarchy; no continuous flashing; safe ambient idle; motion reduction; no WebGL resource leaks; maintain 60 FPS goal when feasible and graceful quality downgrade without falsifying state.


---

## FILE: verification/TEST-MATRIX-AND-RELEASE.md

# VERIFICATION / RELEASE GATES

## Data provenance / truth
- Producer emits actual event -> capability -> projection -> authorized transport -> client deduplicator -> renderer mark -> inspectable source link. Capture IDs at each hop (redacted in public exports).
- No event: no semantic pulse. Idle decorative particles permitted only as unambiguously ambient.
- No health source: unavailable, NOT healthy. No token source: not zero tokens. No real edge evidence: do not draw operational link.
- Derived/inferred grouping explicitly distinct from directly observed relationships; explain confidence.
- A source returns partial data: show completeness/warnings, never silently erase uncertainty.
- Older timestamp becomes stale; reconnection restores fresh only after evidence.
- Replay strictly sourced to real recorded authorized events, with REPLAY badge; do not relabel replay as LIVE.

## Director / registration
- Same inputs produce same scene schedule with seeded deterministic policy.
- Weight/fairness prevents eligible scene starvation; min/max dwell and cooldown enforced.
- New scene descriptor automatically feeds chooser, docs, controls and other derived surfaces.
- Missing capability -> no fabricated fallback; proper skip or explicit unavailable view.
- Scene crash isolated; renderer continues with honest error; cleanup invoked exactly once.

## Streaming / network
- Initial snapshot + cursor transitions; duplicate, reordering, cursor invalidation, event gap, queue pressure and intermittent offline.
- Backoff jitter, reconnection and snapshot resync; resubscription does not leak handles.
- Test protected stream and public stream separately, including cached responses.
- One slow client cannot grow server memory without bound.

## Security
- Public endpoint MUST NOT serialize unauthorized: provider secrets, full prompts, environment, internal network identifiers, sensitive repo paths, private issues or identities. Test both snapshot and stream and nested metadata.
- Query `?mode=public` MUST NOT escalate permissions; user/role authorization server-side.
- Text content escaped in labels/overlays; CSP review; no third-party CDN required to run.
- No logging raw sensitive payloads in client errors or diagnostic exports.

## Rendering and kiosks
- 1080p/4K golden screenshots from deterministic recorded real-data fixtures; distinguish fixture replay from production live.
- Resize/devicePixelRatio/visibility changes, GPU context loss, scene teardown, browser refresh, long idle, huge event bursts.
- FPS p50/p95/p99 and event-to-render measured against reference hardware; goal 60 FPS or adaptive 30 FPS if necessary; avoid promises.
- Memory trends and resource counts monitored; setup optional extended 24-hour soak test; claim long-term reliability only if measured.
- Reduced-motion and keyboard/focus/accessibility alternative.
- Safe anti-burn-in repositioning and full-screen gesture constraints.

## CI / governance / docs
- Follow canonical repo fmt/lint/unit/integration/web E2E/check commands (discover, never invent).
- Cross-surface IDs, ordering, metadata, generated asset and API docs drift checked.
- Screenshot and marketing claims contain source/freshness or explicitly note replay.
- No bypass of protected branches. Merge/deploy only authorized and all gates green.

## Evidence report table template
| Proof | Source/command | Result | Artifact / link | Known caveat |
|---|---|---|---|---|
| Route registered | | | | |
| Genuine peer source | | | | |
| Real event -> visual pulse | | | | |
| Director rotation | | | | |
| Source absent handling | | | | |
| Cursor reconnect | | | | |
| Privacy redaction | | | | |
| Browser / FPS | | | | |
| Soak (if run) | | | | |
| Docs / deploy | | | | |

## Release definition
Release must remain inspectable, maintainable and truthful under failure. Prefer first deploy real Mesh Cosmos + Event River before polishing all eight unsupported screens. Eight descriptions without live source correctness are not release completion.


---

## FILE: reference/CONVERSATION-CONSOLIDATION.md

# CONVERSATION CONSOLIDATION — OCTOBER 13/14, 2026

This document consolidates all user intent and assistant design iterations relevant to Observatory. It is NOT a verbatim chat transcript. Quotes and claims from prior assistant messages are proposals unless reverified against the current checkout. A prompt pack cannot prove the application already implements a feature.

## User intent chronologically
1. Request for a `/ui/screensaver` route using sexy p5.js, semoscena-like information architecture, beautiful data-art representation of Majordomus internals and mesh, as functional fullscreen 4K hall LCD, operational dashboard and marketing installation.
2. Request for multiple screens that gradually rotate, reflect real data, art/tech art, real-time, reliable, derived/inferred, autonomously operating.
3. Request to bundle the entire conversation, all tech sketches and designs into one detailed prompt pack.
4. Final categorical rule: *everything must use REAL data; inferred/derived MJ way.* This supersedes any earlier permissive 'synthetic demo' feature wording.

## Initial conceptual architecture
`canonical Majordomus capabilities (mesh/events/intent/evidence/economics) -> observable state projection -> initial snapshot + events -> typed scene model -> p5.js WEBGL or Canvas2D -> kiosk or authenticated UI`. Names of capabilities (`mesh.peers`, `mesh.events`, `monitor.stream`, `web.surfaces`, `/api/v1/monitor`) originated in prior conversation; implementer must verify existence in actual code and reuse or adapt. Must respect .ai rules, stable identities, canonical ordering and UI design token generation.

## Scenes gathered from both iterations
1 Mesh Constellation / Mesh Cosmos. 2 Activity River / Event River. 3 Intent Galaxy / Intent Nebula. 4 System Heartbeat / System Pulse. 5 Token Spectrum. 6 Delivery Tree. 7 Cooperation Choreography. 8 Ambient Intelligence. Scene selection capability aware; 8 scenes planned, do not fake unsupported real sources.

## Live visual mapping examples
- Verified node arrival -> node blooms; stale node dims with explicit stale status (not inferred dead).
- Real tool invocation/peer message -> short particle/edge pulse, only if source-backed.
- Recorded intent -> execution -> evidence relation -> illuminated graph path.
- Verified test completion -> bounded cyan wave, verified failure -> amber/red semantic status.
- Real release event -> emphasis on delivery scene, provenance to actual release.
- Token telemetry -> measured spectrum and estimated/reconciled economics separated.
- Quiet system -> pleasant slow background motion, explicitly decorative.

## Director proposal
Scene rotation 30–90 sec baseline and 60–180 sec ambient. Priority boosts for new peers, releases, incidents; fairness, minimum and maximum dwell, cooldown, manual lock, recovered status, quiet mode. Director's reason can be inspected. User-facing earlier interactive panel was a local *simulation of director controls*, not real telemetry.

## 4K hall / art
Dark deep-space canvas, sparse big type, turquoise/amber/violet accents controlled by canonical design system, slow orbital dynamics, controlled bloom and particles; screenshot concept showed network nodes, paths, bottom status strip, 4 metric cards. This mockup was a static conceptual simulation, not real mesh. Scene transitions dissolve, spatial morph when IDs match, crossfade otherwise. Anti-burn-in, quality adaptation, offline resilience, reduced motion and memory leak prevention.

## Data contract
`ObservatorySnapshot { schema_version, generated_at, completeness, sources, nodes, edges, metrics, cursor }`, `ObservatoryEvent { id, cursor, timestamp, kind, severity, source, entity_ids, payload }`. Sketch types, not committed API guarantees. Add typed event evidence, privacy scope, freshness and semantic vs decorative flag.

## Intended URLs
`/ui/screensaver`, `?scene=mesh|activity|intents|heartbeat|tokens|delivery|coop|ambient`, `?mode=kiosk|public|replay`, `?auto=1`, `?motion=reduced`, `?quality=auto`. Query params NOT permissions. Later stronger request excludes synthetic demo mode: demonstration is only real, recorded, authorized, redacted REPLAY; synthetic fixtures strictly test-only.

## Implementation sequence and judgments
Initial user-requested high-detail assistant prompt had 4 scenes, expanded to 8 with auto Director. Final principle: first working route and genuine mesh/event stream; then director/rotation; then other capability-aware scenes, then art perfection, reliability, docs and release. Must not convert 'walk an extra mile' into uncontrolled architectural scope creep. Preserve data reality and usable first release.

## Critical caveat
No actual GitHub code was re-audited during creation of this bundle. The assistant previously made specific claims about routes, monitoring and design tokens; treat them as potentially stale investigation leads only. Acceptance requires code and test evidence. Do not invent exact routes, sources, latency, version, deploy or actual screenshot.

## Final truth rule
No synthetic production runtime source, even for demo. A replay mode may use only genuinely captured, privacy-cleared events and must be obviously replay. Test fixtures may be synthetic if they are used solely in tests (not in running displays or marketing claims). If no observations exist, show honest idle/unknown, artistically rich nonsemantic ambient animation and connection status.
