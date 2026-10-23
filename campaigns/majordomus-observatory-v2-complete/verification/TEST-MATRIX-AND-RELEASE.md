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
