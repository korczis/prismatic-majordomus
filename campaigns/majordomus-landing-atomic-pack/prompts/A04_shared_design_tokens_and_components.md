# A04: Shared design tokens and components

**Depends on:** A02. **Scope:** One coherent reviewable atom.

## Assignment to Opus/Fable
Inspect design tokens, generated design projection and Flowbite usage. Establish reusable heading, cards, evidence, status, command, breadcrumb, provenance and relation components with token-backed colors/density. No duplicate CSS palettes or full framework rewrite.

## Deliverables
Minimal semantic component library reused by landing/docs and design snapshots.

## Proof / acceptance gate
Design generator/check idempotent; semantic color contrast and no component-specific invented product data.

## Release checkpoint
Deploy with next visible slice if no standalone user-facing improvement; do not bump for CSS-only atom contrary to policy.

## Execution contract
1. Read the master prompt and verified repository anchors. Inspect current files, canonical commands, and relevant `.ai` rules. Do not guess schema/CLI names.
2. Specify smallest implementation and tests, including at least one adversarial/negative case where data contracts change.
3. Implement, regenerate via existing tooling, execute focused and canonical validation; repeat to check drift/idempotence.
4. Review provenance, source links, public/private filtering and performance if affected.
5. Document meaningful architectural invariants, and extend real `.ai` enforcement when generalized policy changes.
6. Observe versioning doctrine and PR checks. Merge and progressively deploy eligible stable public slice, then validate published commit with existing `scripts/pages` commands.
7. Record exact evidence in intent/issue/session handover: files, commands, exit codes, commit SHA, release decision, Pages observed SHA, blockers.

**Non-negotiable:** Never substitute a screenshot or a GitHub workflow green light for live publication proof. Do not bypass failing checks, invent registry facts or report unverified deployment as done.
