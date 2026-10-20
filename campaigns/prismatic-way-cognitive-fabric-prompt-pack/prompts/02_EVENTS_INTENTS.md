# WAVE 02 — Event-driven intent reconciliation

Use current event/monitor/episode infrastructure. Implement small normalized source adapters for actual events, **not** new independent hooks and watcher daemons. Determine availability per client version; support absent hooks cleanly. Keep event inputs minimal: event_type, repo/worktree/session/actor identity, timestamp, source, causal/correlation key, source object ref, content digest, optionally scoped payload. Never ship secrets or raw transcripts to the mesh/public UI.

Derive idempotency key from stable source-native event identity plus namespace, not timestamp alone. Define causal parent, retry/replay semantics, monotonic/version boundaries. Deduplicate exact replay; coalesce noisy file events; cap cascades, queues and retries; expose diagnostics for drops and dead-lettered events. Event ordering must be deterministic for the same logical input even with parallel arrival.

Build `affected entities` computation via existing knowledge/entity dependencies and Git diffs. Use stable IDs, typed relations, scoped context resolution and evidence refs. Don't rescan `.ai/**` on every event. Match existing intents before proposing new ones. Score heuristics must be explainable; confidence is not proof. Ambiguous matches remain candidates. Ensure mutation writes through original canonical writers and task scope/mesh claim authorization.

Scenarios: repeated UserPromptSubmit, resumed session, renamed file, amended commit, out-of-order CI event, concurrent peer events, unmatched prompt, no semantic relevance, replay after crash, malformed event, wrong repository identity, and deliberate hook recursion. Add unit, property and integration tests. Instrument hook latency and delta size.

Acceptance: one fixture completes `event → intent identity → affected task/knowledge links → evidence → original state writer → cross-surface read model`; replay leaves exactly same IDs/state; no hot-path LLM requirement; no duplicate canonical store.
