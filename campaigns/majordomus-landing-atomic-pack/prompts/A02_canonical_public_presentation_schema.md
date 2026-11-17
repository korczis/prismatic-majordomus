# A02: Canonical public presentation schema

**Depends on:** A00,A01. **Scope:** One coherent reviewable atom.

## Assignment to Opus/Fable
Review product registry, feature schema, site data and design model. Define tiny typed public entity projection with IDs, visibility/status, source/proof and relations using existing primitives. Disallow source-list duplication. Reuse stable deterministic ordering. Keep generated artifacts under existing generator.

## Deliverables
Shared public projection and schema/drift tests; migration of relevant existing entries without adding another registry.

## Proof / acceptance gate
Randomized discovery produces identical output; private objects filtered; duplicate/missing references fail with actionable errors.

## Release checkpoint
Deploy if public projection pages change; verify live canonical SHA.

## Execution contract
1. Read the master prompt and verified repository anchors. Inspect current files, canonical commands, and relevant `.ai` rules. Do not guess schema/CLI names.
2. Specify smallest implementation and tests, including at least one adversarial/negative case where data contracts change.
3. Implement, regenerate via existing tooling, execute focused and canonical validation; repeat to check drift/idempotence.
4. Review provenance, source links, public/private filtering and performance if affected.
5. Document meaningful architectural invariants, and extend real `.ai` enforcement when generalized policy changes.
6. Observe versioning doctrine and PR checks. Merge and progressively deploy eligible stable public slice, then validate published commit with existing `scripts/pages` commands.
7. Record exact evidence in intent/issue/session handover: files, commands, exit codes, commit SHA, release decision, Pages observed SHA, blockers.

**Non-negotiable:** Never substitute a screenshot or a GitHub workflow green light for live publication proof. Do not bypass failing checks, invent registry facts or report unverified deployment as done.
