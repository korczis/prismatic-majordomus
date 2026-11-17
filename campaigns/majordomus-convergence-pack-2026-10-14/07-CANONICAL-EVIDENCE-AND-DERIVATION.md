# Mission 07 — canonical evidence and zero-drifts across surfaces

Inspect `docs/DOCTRINE.md`, `docs/SITE_CLAIMS.md`, `docs/CLAIMS.yaml`, actual Rust capabilities, shell, HTTP, OpenAPI, MCP, UI, site, governance and tests. Ensure all comments respect precise available source. No new parallel registry unless repo truly lacks a suitable canonical one.

- Enumerate canonical operations, claims, rules, policies, skills, profiles and projections, verify stable typed IDs, deterministic order and machine-readable provenance.
- Preserve status lattice: `observed`, `derived`, `inferred`, `planned`, `unknown`, `stale`, `verified`, `rejected` based on real repository definitions; if status taxonomy differs, map explicitly without semantic invention. Evidence content-addressing where existing design permits.
- Doctrine proof chain: principle → validator → enforcement point → failure propagation → behavioral/mutation test → CI → public claim. Detect orphan validators and unenforced declared rules. Test negative behavior, not just registry linkage.
- Implement or extend one canonical projection per existing mechanism. New entity should appear automatically in relevant CLI/JSON/API/MCP/OpenAPI/Cockpit/site/skills/completions without consumer-specific registration.
- Eliminate accidental filesystem/HashMap/concurrency order and front-end independent inventories. Use stable tie-breakers. Preserve explicit user sorts/filters without changing canonical default.
- Maintain versioned schema and migration rules. Tests for legacy and newer entities, historical file/test reference resolution (seen fixed via #1145), deleted paths that never existed, delayed derivation and data drift.
- Build `claim -> code -> test -> CI result -> release -> deployed observed` drilldown. Missing result returns UNKNOWN or STALE; never green by inference.
- Keep `docs/CLAIMS.yaml` as canonical if still so, regenerate SITE_CLAIMS and validate each guarantee. Downgrade inaccurate marketing promises rather than implement unsupported leaps solely to preserve copy.
- Publish validation diagnostics with exact path, entity id, source, expected, actual, remediation command.

Acceptance: cross-surface contract and generated drift tests pass, stable deterministic arrays across permutations, actionable failure for missing evidence, no newly duplicated source inventories.
