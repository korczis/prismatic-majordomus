# Metrics and UX contract

| Metric | Formula / caution |
|---|---|
| Cost / accepted outcome | comparable-period attributed cost / independently accepted outcomes; report sample size and exclusions |
| Cost per attempted intent | attributed + declared overhead cost / started intents; includes failed attempts |
| Context efficiency | input/output/cache composition, never a universal quality score |
| Cache rate | cache_read / (eligible cache_read + eligible fresh input); disclose provider semantics |
| Tool overfetch | tool bytes or estimated subsequent tokens when correlation verified; treat as opportunity not proof |
| Compaction | pre/post context reduction, not automatically cost savings |
| Outcome rate | accepted outcomes / begun eligible outcomes; avoid selection bias |
| Coverage | actually observed telemetry windows or requests / expected, denominator assumptions disclosed |
| Net savings | baseline - treatment measured comparable cost, CI + quality non-inferiority |

## User journeys

1. Open `/ui` overview → identify what is observed, unknown, estimated or reconciled.
2. Filter provider/model/repo/intent/time → source-backed charts update and maintain URL state.
3. Open IDD intent → trace usage to execution → accepted outcome test/PR/CI evidence.
4. Inspect worker handover/mesh path and unallocated cost; correct *policy* not history facts.
5. Compare optimization candidate against baseline, uncertainty and quality gates.
6. Observe live update after a measured event, reconnect via resume cursor, see staleness during outage.
7. Use CLI/API/MCP for equivalent records; safe public Pages never reveals sessions/identities/secrets.

## Chart contract

Each chart has series/source, units, time zone, grouping, query bounds, event coverage, freshness, missingness semantics and provenance. Empty chart says "No observations"; zero line only from measured zeros. Historical recomputation respects versioned pricing.
