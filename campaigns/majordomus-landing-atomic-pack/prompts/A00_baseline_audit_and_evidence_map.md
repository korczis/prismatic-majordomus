# A00: Baseline audit and evidence map

**Depends on:** none. **Scope:** One coherent reviewable atom.

## Assignment to Opus/Fable
Audit exact HEAD, public deployment, mobile/desktop and current product claims. Inventory canonical producers/consumers, open PR/worktree risks, CI durations, Pages verification, design tokens, site generation and runtime capabilities. Capture measured baselines and critical user journeys. Do not edit UI yet.

## Deliverables
Evidence report with reproducible commands, current source/published SHAs, prioritized defects, risk matrix, execution DAG and public vs private boundary.

## Proof / acceptance gate
All claims tied to source/test/live inspection; no assumed existing features; baseline recorded.

## Release checkpoint
No deploy unless baseline explicitly detects current owed publication, in which case use existing repair path only.

## Execution contract
1. Read the master prompt and verified repository anchors. Inspect current files, canonical commands, and relevant `.ai` rules. Do not guess schema/CLI names.
2. Specify smallest implementation and tests, including at least one adversarial/negative case where data contracts change.
3. Implement, regenerate via existing tooling, execute focused and canonical validation; repeat to check drift/idempotence.
4. Review provenance, source links, public/private filtering and performance if affected.
5. Document meaningful architectural invariants, and extend real `.ai` enforcement when generalized policy changes.
6. Observe versioning doctrine and PR checks. Merge and progressively deploy eligible stable public slice, then validate published commit with existing `scripts/pages` commands.
7. Record exact evidence in intent/issue/session handover: files, commands, exit codes, commit SHA, release decision, Pages observed SHA, blockers.

**Non-negotiable:** Never substitute a screenshot or a GitHub workflow green light for live publication proof. Do not bypass failing checks, invent registry facts or report unverified deployment as done.
