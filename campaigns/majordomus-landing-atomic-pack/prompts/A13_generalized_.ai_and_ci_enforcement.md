# A13: Generalized .ai and CI enforcement

**Depends on:** A02,A06,A08,A09,A10. **Scope:** One coherent reviewable atom.

## Assignment to Opus/Fable
Codify *cross-cutting* projection invariants into existing `.ai` rules/doctrines and real validator path. Extend `scripts/site-check`/`homepage-check` and CI where appropriate: status honesty, public visibility, no duplicate inventory, deterministic index, provenance, links, deployment ownership. Avoid a markdown-only rule.

## Deliverables
Enforceable single-model contracts, positive and deliberately-failing regression tests, docs explaining how to extend.

## Proof / acceptance gate
Prove each gate fails in a mutated test fixture and passes clean repository; no redundant workflow.

## Release checkpoint
If gate changes public output, publish. Otherwise merge after tests, update release ledger.

## Execution contract
1. Read the master prompt and verified repository anchors. Inspect current files, canonical commands, and relevant `.ai` rules. Do not guess schema/CLI names.
2. Specify smallest implementation and tests, including at least one adversarial/negative case where data contracts change.
3. Implement, regenerate via existing tooling, execute focused and canonical validation; repeat to check drift/idempotence.
4. Review provenance, source links, public/private filtering and performance if affected.
5. Document meaningful architectural invariants, and extend real `.ai` enforcement when generalized policy changes.
6. Observe versioning doctrine and PR checks. Merge and progressively deploy eligible stable public slice, then validate published commit with existing `scripts/pages` commands.
7. Record exact evidence in intent/issue/session handover: files, commands, exit codes, commit SHA, release decision, Pages observed SHA, blockers.

**Non-negotiable:** Never substitute a screenshot or a GitHub workflow green light for live publication proof. Do not bypass failing checks, invent registry facts or report unverified deployment as done.
