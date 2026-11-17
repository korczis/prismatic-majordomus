# Definition of Done and enforceable regressions

1. Every route produced by canonical web topology is reachable; every nav item resolves to served destination; no duplicate route catalogs.
2. Each route supports no-JS SSR reading and meaningful offline/permission/error states.
3. Each displayed metric or field has a mapping to a canonical producer, observation/provenance, and explicit staleness semantics. Tests fail if source is absent rather than filling zero.
4. All UI actions are real and executable only via declared backend capability and permission; negative tests assert denied and invalid inputs do not mutate state.
5. Mobile 320/375 widths show readable content, accessible keyboard drawers, no inaccessible table/graph, no trapped focus or horizontal page clipping.
6. Screenshot comparisons to references are visual layout targets, not exact dynamic text diffs; use stable test fixture for regression screenshots, never production mocks.
7. Graphs/charts lazy load and dispose; static accessible summaries exist when JS disabled. Motion obeys reduced-motion.
8. UI design originates in canonical token file and generated CSS passes drift validation.
9. `.ai/` rule added only alongside executable proof and registered gate in canonical CI model; superseded rules migrated/deleted where appropriate; negative tests prove refusal.
10. CI paths and caching do not silently skip affected tests; collect actual runtime measurements.
11. Release version chosen by repository policy, not demanded artificial bump; deploy only to authorized targets and verify source commit, health, smoke, access and rollback.
12. Live Pages documentation is derived from released verified facts. Distinguish published snapshot from runtime live Cockpit.
13. All work items reconciled to canonical intents/milestones/issues. No claims of autonomy, release, or deployment without actual execution records.
