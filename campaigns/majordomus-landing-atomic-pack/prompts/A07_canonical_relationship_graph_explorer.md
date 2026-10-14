# A07: Canonical relationship graph explorer

**Depends on:** A02,A04. **Scope:** One coherent reviewable atom.

## Assignment to Opus/Fable
Build narrowly scoped graph JSON projection from canonical intent/feature/claim/surface relations that truly exist; choose Cytoscape only if bundle budget permits. Stable IDs and deterministic layout; graph/list fallback; click node→source/claim/proof; touch/keyboard support; no hand-entered graph.

## Deliverables
Graph UI plus list view and typed relation dataset.

## Proof / acceptance gate
Cross-surface entity identity test, orphan-edge failure, zero-registration fixture and mobile graph fallback.

## Release checkpoint
PUBLIC SLICE: publish and verify interactive + static fallback.

## Execution contract
1. Read the master prompt and verified repository anchors. Inspect current files, canonical commands, and relevant `.ai` rules. Do not guess schema/CLI names.
2. Specify smallest implementation and tests, including at least one adversarial/negative case where data contracts change.
3. Implement, regenerate via existing tooling, execute focused and canonical validation; repeat to check drift/idempotence.
4. Review provenance, source links, public/private filtering and performance if affected.
5. Document meaningful architectural invariants, and extend real `.ai` enforcement when generalized policy changes.
6. Observe versioning doctrine and PR checks. Merge and progressively deploy eligible stable public slice, then validate published commit with existing `scripts/pages` commands.
7. Record exact evidence in intent/issue/session handover: files, commands, exit codes, commit SHA, release decision, Pages observed SHA, blockers.

**Non-negotiable:** Never substitute a screenshot or a GitHub workflow green light for live publication proof. Do not bypass failing checks, invent registry facts or report unverified deployment as done.
