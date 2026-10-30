# Cost and quality controls

Use existing `majordomus reasoning` modes and advisor selection. Current `docs/REASONING.md` says capability-based plans, zero advisors in CI and optional unavailable advisors; do not replace these policies. Enrich with measured economics only if available.

## Routing tiers
0. Deterministic / zero LLM: parser, Git diff, schema and graph.
1. Cheap local classification: only material ambiguous identity/category matches.
2. Existing reasoning assessment/consultation: justified impact/uncertainty.
3. Independent peer critique: highest materiality or unresolved contradictions, subject to policy.

## Metrics and attribution
`events_ingested`, `events_deduped`, `reconcile_latency_ms{p50,p95,p99}`, `sync_hook_duration_ms`, `affected_entity_count`, `LLM_calls_per_event`, `LLM_tokens_input/output/cache`, `inferred_candidates`, `verified_candidate_rate`, `rejected_candidate_rate`, `false_match_rate`, `conflicts`, `replay_convergence`, `cost_per_verified_intent`, `manual_interventions_per_completed_intent`, `freshness_age`, `privacy_violations`, `publication_drift`.

Always differentiate estimated token/cost telemetry from authoritative reconciled billing. Subscription products are not automatically fungible API quotas. Avoid scraping client internals or bypassing vendor rate limits. Never include raw prompt text or credentials in cost dashboards.

## Budget and decision function
Before expensive execution estimate expected benefit and cost from actual baseline; skip model work if deterministic proof already settles the issue. Set per-task caps for token, time, peer fanout and spend in existing policy surface, not a new parallel settings tree. Circuit-break failing peers. Falling back to local deterministic analysis is a valid and visible outcome.

## Experiment design
Measure against equivalent historical or contemporaneous tasks with same verification criteria. Record N, baseline, data sources, workload, uncertainty and confounders; do not claim causal ROI from one demo. Proposed threshold targets become enforceable only after baseline and documented decision. Count verified outcomes, not generated artifact volume.
