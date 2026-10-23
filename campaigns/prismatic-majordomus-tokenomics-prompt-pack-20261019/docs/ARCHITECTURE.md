# Tokenomics architecture & cross-connections

```mermaid
flowchart TD
 A[Claude Code official telemetry] --> C[Canonical ingest and evidence ledger]
 B[Other provider measured adapters] --> C
 H[Lifecycle hooks and mesh monitor] --> I[Existing execution and IDD graph]
 C --> J[Evidence-backed attribution]
 I --> J
 G[Git CI and acceptance evidence] --> O[Accepted outcome projection]
 J --> E[Economics and coverage projection]
 O --> E
 P[Versioned price and budget policy] --> E
 E --> U[Shared capability/API/MCP/CLI projection]
 U --> V[UI and Cockpit live chart drilldowns]
 U --> S[Privacy-filtered Pages snapshot]
 E --> F[Optimization findings and experiments]
 F --> P
```

### Critical architectural connections

- IDD execution graph provides causal work ownership, **not** token measurements.
- Mesh monitor events provide lifecycle context, **not** invoice authority.
- Git/CI/verification gates define accepted outcomes, **not** activity proxies.
- `.ai/repo` stores normative measurement and routing policy, **not** raw events.
- `.ai/local` is per-checkout and non-public; preserve its isolation.
- Canonical capability declaration generates transport contracts instead of adding manually synchronized schemas.
- Pages static exports cannot be genuinely live unless an authenticated backend is separately available and intentionally exposed. Mark snapshots as snapshots.
- Derived optimization needs interventional measurement and quality constraints to graduate from advisory to enforced.
