# A09: Docs architecture and connected reference

**Depends on:** A05,A08. **Scope:** One coherent reviewable atom.

## Assignment to Opus/Fable
Converge landing/docs presentation, generated CLI/MCP/API and knowledge documentation. Use canonical capability registry / OpenAPI, not duplicate endpoint/tool lists. Docs route and front-matter navigation metadata derived where supported. Preserve ADR and public-source citation links.

## Deliverables
Unified docs IA with related-entity trails; generated reference, correct canonical/version/source link metadata.

## Proof / acceptance gate
Generator parity tests CLI/API/MCP, link and anchor checks, no orphan docs and no status inflation.

## Release checkpoint
PUBLIC SLICE: `scripts/pages verify-docs` against published SHA.

## Execution contract
1. Read the master prompt and verified repository anchors. Inspect current files, canonical commands, and relevant `.ai` rules. Do not guess schema/CLI names.
2. Specify smallest implementation and tests, including at least one adversarial/negative case where data contracts change.
3. Implement, regenerate via existing tooling, execute focused and canonical validation; repeat to check drift/idempotence.
4. Review provenance, source links, public/private filtering and performance if affected.
5. Document meaningful architectural invariants, and extend real `.ai` enforcement when generalized policy changes.
6. Observe versioning doctrine and PR checks. Merge and progressively deploy eligible stable public slice, then validate published commit with existing `scripts/pages` commands.
7. Record exact evidence in intent/issue/session handover: files, commands, exit codes, commit SHA, release decision, Pages observed SHA, blockers.

**Non-negotiable:** Never substitute a screenshot or a GitHub workflow green light for live publication proof. Do not bypass failing checks, invent registry facts or report unverified deployment as done.
