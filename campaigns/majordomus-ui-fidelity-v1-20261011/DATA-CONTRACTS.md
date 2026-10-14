# Canonical dashboard data contract and provenance

## Derive, do not duplicate
Audit the actual Rust capability registry and existing Cockpit's `dashboard.overview`, `repository.info`, `health.report`, `graph.list/get`, `executions.list/get/events`, `continuity.*`, `plan.*`, `intent.*`, PR integration and mesh capabilities. Names are candidate paths based on docs, not guaranteed availability on current HEAD. Extend or compose existing typed descriptors; do not build a second dashboard-specific ledger.

Each optional reading must carry: `value/state`, canonical ID, as-of timestamp or data revision, provenance/evidence reference, availability (`available`, `empty`, `not_measured`, `not_supported`, `stale`, `unauthorized`, `error`), and quality where available. No silent `unwrap_or_default()` converting errors into zeros.

| UI area | canonical evidence | contract pitfall |
|---|---|---|
| Repository/branch | VCS inspector and release record | different worktrees/branches |
| Active intents | typed intents + satisfaction evidence | issue completion != intent satisfied |
| Milestones | dependency-aware canonical plan | blocked milestone != zero progress |
| PR queue | live GitHub-integrated PR projection | queue snapshot != guaranteed current API state |
| CI latency | timestamped workflow run telemetry | queue delay vs runner time, p50 vs p95, sample N |
| Throughput | merged PR events/time window | opened != merged; duplicated webhook delivery |
| Deployed version | deployed artifact attestation & verified endpoint | source tag != deployment |
| Mesh peers | authenticated discovery, heartbeat | discovered != available/executable/authorized |
| Blockers | typed validated blocker edges | warnings != blocking defects |
| Activity stream | durable execution/evidence events | out-of-order delivery; replay duplicates |
| Roadmap | intent/milestone dependency graph | arbitrary percentages; guessed release dates |

## Projection API rules
- Existing shared executor provides authorization and execution IDs. Every UI mutation is a capability invocation with typed input and verifiable result; use CSRF protection and same-origin policies already present.
- For live views use existing event transport with cursor/last event ID/reconnect and a snapshot-before-events barrier to prevent gaps; if transport lacks these guarantees, implement at authoritative layer before presenting `Live`.
- API/schema/OpenAPI/MCP/CLI parity and deterministic ordering are mandatory. No duplicate client aggregation of domain logic.
- Charts use measured datasets (timestamps in ISO 8601 or epoch, units, sample count, window, aggregation contract), no random generator, no frontend hardcoded arrays. Tooltips expose source/evidence where safe.
- Distinguish read staleness from deployment staleness, and stale snapshot from live state.
