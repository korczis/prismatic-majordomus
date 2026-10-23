# MISSION 08 — ATOMIC DEPLOY TRAIN

Discover actual build, CI, release, deploy and version scripts; do not guess commands. Use canonical Rust binary and existing workflows; Just/shell as thin ergonomic wrappers. Document authoritative paths used.

Suggested independently shippable sequence:
R1 shared executor → UI action + execution page + event link;
R2 intent composer → validated plan preview and materialization;
R3 issues/milestones → eligible start and completion evidence;
R4 mesh assignment → durable claim/recovery;
R5 interactive graph → correlated events, node inspector and mobile;
R6 PR/release/deploy → verified closure and dogfood.

For every Rn gate: compile, format, lint, focused tests, integration/browser tests for modified surfaces, schema and generated artifact drift, security checks, relevant benchmarks, negative governance fixtures, documentation links and UI smoke. Commit a coherent slice, open PR, satisfy protected branch policy, merge, bump version only per discovered doctrine, deploy by established mechanism, verify actual revision. Record artifacts and failure details. Do not wait for a gigantic batch to reach R6.

Acceptance matrix:
- UI input leads to same capability as CLI/API/MCP;
- canonical intent revised and materialized idempotently;
- execution events and replay show correct work after reconnect;
- peers cannot double-own same lease;
- graph matches canonical relationships without client-specific state;
- evidence maps to criteria and blocks unsupported closure;
- rules changed only with validator and test proof;
- docs/Pages generated and validated;
- code released, deployed and observed, or explicitly blocked by external limitation.

Create `/ui` dogfood walkthrough using real IDs and timestamps. If missing external dependencies, prove local boundary and mark blocked surfaces. Do not fabricate release URLs, benchmark results, or screenshots.
