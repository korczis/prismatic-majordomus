# Canonical identity, provenance and reconciliation contract

## Distinct identities
- Logical artifact: durable conceptual uploaded item.
- Content blob: immutable bytes identified by cryptographic digest.
- Artifact revision: immutable snapshot of a logical artifact referencing blobs and source structure.
- Pack: composite logical artifact with versioned membership/manifest.
- Source segment: addressable original span/heading/path with extractor version.
- Intent/criterion/milestone/issue: existing project model, NOT copies.
- Execution run: accepted plan revision and authority context; may span multiple sessions.
- Execution episode: one agent/provider attempt.
- Operation: idempotency key, expected effects and external reconciliation status.
- Evidence: typed claim support, immutable provenance, validator and freshness.

## Suggested relation semantics
`segment proposes criterion`, `criterion served by issue`, `issue executed in run`, `task attempted by episode`, `PR implements issue`, `evidence validates criterion`, `release contains commit`, `deployment verified against release`. Use extant canonical linking schema rather than literal new tables.

## Revision edge cases
Same blob, new filename: preserve byte identity; decide logical relation by explicit identity. Rename heading: structural match plus provenance, not false delete+add by default. Changed acceptance: require evidence freshness evaluation. Deleted text: never delete immutable history. Concurrent pack revision: pin in-flight accepted plan, show reconciliation delta before switching. Multiple packs converging on one issue: maintain multiple source links with dedup.

## Progress
Report requirements discovered, criteria satisfied, implementation complete but not verified, blocked and superseded separately. Percent = satisfied applicable criteria / total applicable criteria only if criteria have comparable weight, otherwise give raw counts. Progress never comes from LLM assertion alone.
