# A01: Intent, milestone and proof governance

**Depends on:** A00. **Scope:** One coherent reviewable atom.

## Assignment to Opus/Fable
Express outcome as canonical intent: comprehensible product, project adoption, navigable evidence, zero duplication, mobile reliability, deterministic publication. Derive milestone/issue decomposition and measurable acceptance evidence using real intent primitives, not new YAML tracker. Make live deployment a distinct criterion.

## Deliverables
Typed intent/plan/evidence ownership, per-atom criteria and linked issue/milestones, explicit blocker taxonomy.

## Proof / acceptance gate
Intent validates and work links resolve; cannot mark criterion done without evidence; negative regression test where appropriate.

## Release checkpoint
Governance-only: follow version/release policy, no artificial Pages publication.

## Execution contract
1. Read the master prompt and verified repository anchors. Inspect current files, canonical commands, and relevant `.ai` rules. Do not guess schema/CLI names.
2. Specify smallest implementation and tests, including at least one adversarial/negative case where data contracts change.
3. Implement, regenerate via existing tooling, execute focused and canonical validation; repeat to check drift/idempotence.
4. Review provenance, source links, public/private filtering and performance if affected.
5. Document meaningful architectural invariants, and extend real `.ai` enforcement when generalized policy changes.
6. Observe versioning doctrine and PR checks. Merge and progressively deploy eligible stable public slice, then validate published commit with existing `scripts/pages` commands.
7. Record exact evidence in intent/issue/session handover: files, commands, exit codes, commit SHA, release decision, Pages observed SHA, blockers.

**Non-negotiable:** Never substitute a screenshot or a GitHub workflow green light for live publication proof. Do not bypass failing checks, invent registry facts or report unverified deployment as done.
