# A05: Responsive navigation and docs IA

**Depends on:** A03,A04. **Scope:** One coherent reviewable atom.

## Assignment to Opus/Fable
Derive desktop/mobile/footer nav from one existing topology/docs metadata model. Improve audience paths Start, Understand, Use, Verify, Integrate; avoid hard-coded 3 nav copies. Use Flowbite/Alpine only as needed; no-JS fallback, keyboard accessibility, focus trap if dialog. Preserve stable URLs.

## Deliverables
Mobile drawer, desktop IA, breadcrumbs, working deep navigation.

## Proof / acceptance gate
E2E 320/375/390/430/768/1024 widths; tab/escape/back controls; dead links fail; identical nav source.

## Release checkpoint
PUBLIC SLICE: merge, canonical Pages deploy, smoke docs/nav at live SHA.

## Execution contract
1. Read the master prompt and verified repository anchors. Inspect current files, canonical commands, and relevant `.ai` rules. Do not guess schema/CLI names.
2. Specify smallest implementation and tests, including at least one adversarial/negative case where data contracts change.
3. Implement, regenerate via existing tooling, execute focused and canonical validation; repeat to check drift/idempotence.
4. Review provenance, source links, public/private filtering and performance if affected.
5. Document meaningful architectural invariants, and extend real `.ai` enforcement when generalized policy changes.
6. Observe versioning doctrine and PR checks. Merge and progressively deploy eligible stable public slice, then validate published commit with existing `scripts/pages` commands.
7. Record exact evidence in intent/issue/session handover: files, commands, exit codes, commit SHA, release decision, Pages observed SHA, blockers.

**Non-negotiable:** Never substitute a screenshot or a GitHub workflow green light for live publication proof. Do not bypass failing checks, invent registry facts or report unverified deployment as done.
