# A12: Performance, a11y, mobile hardening

**Depends on:** A04,A05,A06,A07,A08,A09,A10,A11. **Scope:** One coherent reviewable atom.

## Assignment to Opus/Fable
Measure real production bundles vs `homepage.toml` budgets (340000 asset bytes, 8000 inline JSON initially); lazy load graphs/charts, eliminate blocking assets, fix contrast/CLS/keyboard. Do not silently raise budget to pass. Test representative devices and reduced motion.

## Deliverables
Measured before/after budgets and visual/mobile accessibility fixes.

## Proof / acceptance gate
Current script budgets pass; browser axe/manual checks where available; no accidental overflow 320px; no critical data hidden behind JS.

## Release checkpoint
PUBLIC SLICE: publish and production smoke/asset verification.

## Execution contract
1. Read the master prompt and verified repository anchors. Inspect current files, canonical commands, and relevant `.ai` rules. Do not guess schema/CLI names.
2. Specify smallest implementation and tests, including at least one adversarial/negative case where data contracts change.
3. Implement, regenerate via existing tooling, execute focused and canonical validation; repeat to check drift/idempotence.
4. Review provenance, source links, public/private filtering and performance if affected.
5. Document meaningful architectural invariants, and extend real `.ai` enforcement when generalized policy changes.
6. Observe versioning doctrine and PR checks. Merge and progressively deploy eligible stable public slice, then validate published commit with existing `scripts/pages` commands.
7. Record exact evidence in intent/issue/session handover: files, commands, exit codes, commit SHA, release decision, Pages observed SHA, blockers.

**Non-negotiable:** Never substitute a screenshot or a GitHub workflow green light for live publication proof. Do not bypass failing checks, invent registry facts or report unverified deployment as done.
