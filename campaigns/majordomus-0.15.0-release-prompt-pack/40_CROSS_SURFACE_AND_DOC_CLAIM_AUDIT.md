# Milestone 4 — Cross-surface and documentation truth audit

Goal: one canonical intent state, many truthful projections.

Trace canonical intent model into every applicable surface: CLI, structured JSON, REST, OpenAPI/Swagger, MCP, Cockpit, generated docs/site data.

For one fixture compare identity, stage, criteria, evidence summary, verdict, blockers/gaps if exposed. Shape may differ; semantics may not.

Verify public operations originate from canonical capability declarations, not transport-specific registries.

If Cockpit has intent UI, prove it consumes backend/canonical data and does not decide satisfaction client-side. Distinguish unknown/unsatisfied/satisfied. If there is still no meaningful intent UI, do NOT build broad UI today; keep docs truthful and defer it.

Audit authored claims in README, homepage, features, intent docs, limitations, roadmap, architecture and release-facing prose. Pay special attention to words such as automatic, intent-driven, satisfied, complete, derives a plan, opposition, session loads intent, reconciles GitHub, Cockpit shows intent.

Each claim must be classified shipped/advisory/planned/rejected/historical and supported by executable evidence.

Run canonical claim-proof/site/reference/link/generated checks.

Deliver:

```text
surface | canonical source | representation | test | result
claim | location | classification | supporting evidence
```
