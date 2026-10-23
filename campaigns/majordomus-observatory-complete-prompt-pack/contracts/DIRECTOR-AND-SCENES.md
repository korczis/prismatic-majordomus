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
