# A03: Homepage story and value framing

**Depends on:** A02. **Scope:** One coherent reviewable atom.

## Assignment to Opus/Fable
Redesign within `site/data/homepage.toml` narrative boundaries. Make first viewport instantly explain who, why, worker-vs-repository distinction, concrete case, proof and install. Preserve editorial-only `marketing.toml`, status truth, recorded run. Use interactive-but-progressive hierarchy; do not make homepage 20 sections again.

## Deliverables
Released semantic hero, first-visit journey, story-order declaration and proof CTA.

## Proof / acceptance gate
Homepage check bidirectional story, marketing line-count checks, no manual facts, CTA route smoke, 320px layout check.

## Release checkpoint
FIRST PUBLIC SLICE: release decision, merge, publish, verify actual public SHA.

## Execution contract
1. Read the master prompt and verified repository anchors. Inspect current files, canonical commands, and relevant `.ai` rules. Do not guess schema/CLI names.
2. Specify smallest implementation and tests, including at least one adversarial/negative case where data contracts change.
3. Implement, regenerate via existing tooling, execute focused and canonical validation; repeat to check drift/idempotence.
4. Review provenance, source links, public/private filtering and performance if affected.
5. Document meaningful architectural invariants, and extend real `.ai` enforcement when generalized policy changes.
6. Observe versioning doctrine and PR checks. Merge and progressively deploy eligible stable public slice, then validate published commit with existing `scripts/pages` commands.
7. Record exact evidence in intent/issue/session handover: files, commands, exit codes, commit SHA, release decision, Pages observed SHA, blockers.

**Non-negotiable:** Never substitute a screenshot or a GitHub workflow green light for live publication proof. Do not bypass failing checks, invent registry facts or report unverified deployment as done.
