# A15: Final dogfood and zero-registration audit

**Depends on:** A12,A13,A14. **Scope:** One coherent reviewable atom.

## Assignment to Opus/Fable
Run one synthetic new public feature/claim fixture through discovery→product→site→search→links→tests, then revert fixture without drift. Execute beginner/integrator/skeptic/maintainer user journeys. Inspect deployed SHA vs source and all merged milestones. Cleanup worktrees and documentation debt.

## Deliverables
Final evidence dossier with per-atom pass/block, current release SHA, publication SHA, gate/test timings, `.ai` invariants and remaining debt.

## Proof / acceptance gate
Two builds byte-stable where intended; all required checks pass; public SHA matches final HEAD; final intent criteria closed only with evidence.

## Release checkpoint
Final verify production Pages + docs; no false completion if still pending.

## Execution contract
1. Read the master prompt and verified repository anchors. Inspect current files, canonical commands, and relevant `.ai` rules. Do not guess schema/CLI names.
2. Specify smallest implementation and tests, including at least one adversarial/negative case where data contracts change.
3. Implement, regenerate via existing tooling, execute focused and canonical validation; repeat to check drift/idempotence.
4. Review provenance, source links, public/private filtering and performance if affected.
5. Document meaningful architectural invariants, and extend real `.ai` enforcement when generalized policy changes.
6. Observe versioning doctrine and PR checks. Merge and progressively deploy eligible stable public slice, then validate published commit with existing `scripts/pages` commands.
7. Record exact evidence in intent/issue/session handover: files, commands, exit codes, commit SHA, release decision, Pages observed SHA, blockers.

**Non-negotiable:** Never substitute a screenshot or a GitHub workflow green light for live publication proof. Do not bypass failing checks, invent registry facts or report unverified deployment as done.
