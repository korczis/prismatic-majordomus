# A06: Proof-first refusal walkthrough

**Depends on:** A03,A04. **Scope:** One coherent reviewable atom.

## Assignment to Opus/Fable
Use `site/data/generated/challenge.json`, terminal scenario evidence and product claims to make inspectable stepper showing refused→corrected→accepted. No mock execution; rendered steps inherit actual recorded outcome, command, evidence and status. Proper static fallback.

## Deliverables
Interactive proof with inspect/refusal reproduction links, source provenance and accessibility.

## Proof / acceptance gate
Test event sequence and outcome matches corpus; injection safety; non-JS readable; negative test altered evidence causes failure.

## Release checkpoint
PUBLIC SLICE: verified live proof with known SHA and evidence links.

## Execution contract
1. Read the master prompt and verified repository anchors. Inspect current files, canonical commands, and relevant `.ai` rules. Do not guess schema/CLI names.
2. Specify smallest implementation and tests, including at least one adversarial/negative case where data contracts change.
3. Implement, regenerate via existing tooling, execute focused and canonical validation; repeat to check drift/idempotence.
4. Review provenance, source links, public/private filtering and performance if affected.
5. Document meaningful architectural invariants, and extend real `.ai` enforcement when generalized policy changes.
6. Observe versioning doctrine and PR checks. Merge and progressively deploy eligible stable public slice, then validate published commit with existing `scripts/pages` commands.
7. Record exact evidence in intent/issue/session handover: files, commands, exit codes, commit SHA, release decision, Pages observed SHA, blockers.

**Non-negotiable:** Never substitute a screenshot or a GitHub workflow green light for live publication proof. Do not bypass failing checks, invent registry facts or report unverified deployment as done.
