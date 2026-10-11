# Canonical projection requirements

For every numeric or status value write a contract row: `UI location | source capability / file or model | semantic definition | unit | range | timestamp | freshness window | authorization | link | unavailable state | test`.

Examples (not promises of pre-existing fields): PR open count computed from actual GitHub integration snapshot with sync metadata; CI p95 measured from bounded completed run cohort; mesh peer online status from lease/heartbeat observation; milestone progress derived from required accepted issues and evidence (never render guessed percent); deployment health from proven smoke result with observed_at and expiry; latest version from canonical release model, not hardcoded sidebar string.

Production network rules: avoid secrets in serialized HTML, event payloads or console logs; avoid unauthorized cross-project data. Validate data at Rust boundary. Do not invent placeholder endpoints. Search actual capabilities and OpenAPI first and extend canonical capability registry only if real domain producer exists. Every mutable UI action requires authorization and explicit server result.

Realtime: prefer established server live transport. Require event seq/revision, initial snapshot, dedup/reconnect, stale detection, backpressure and visible disconnected state; if only polling exists use bounded cadence and call it refreshed, not streaming. Snapshot from Pages carries deployment revision but is not live.

Charts: no fake history generated from latest count, no auto-generated smoothing mistaken for evidence; unavailable series shows empty-state + where to enable measurement. A synthetic series belongs exclusively to deterministic test fixtures.
