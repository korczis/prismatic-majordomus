# A11: Project-oriented onboarding and install

**Depends on:** A03,A08,A09. **Scope:** One coherent reviewable atom.

## Assignment to Opus/Fable
Explain adoption for real repositories: install/init/doctor/ui as supported by existing CLI. Use distribution metadata for install command/version. Present use-case choice by pain point, actual commands, and consequences; no speculative one-curl commands or magic peer claims.

## Deliverables
Short onboarding and project explorer path, copyable canonical commands, no backend dependency for browsing.

## Proof / acceptance gate
Every shown command exists in current registry, recorded output marked, copy and no-JS fallback, no secret leakage.

## Release checkpoint
PUBLIC SLICE: verify install path and docs live.

## Execution contract
1. Read the master prompt and verified repository anchors. Inspect current files, canonical commands, and relevant `.ai` rules. Do not guess schema/CLI names.
2. Specify smallest implementation and tests, including at least one adversarial/negative case where data contracts change.
3. Implement, regenerate via existing tooling, execute focused and canonical validation; repeat to check drift/idempotence.
4. Review provenance, source links, public/private filtering and performance if affected.
5. Document meaningful architectural invariants, and extend real `.ai` enforcement when generalized policy changes.
6. Observe versioning doctrine and PR checks. Merge and progressively deploy eligible stable public slice, then validate published commit with existing `scripts/pages` commands.
7. Record exact evidence in intent/issue/session handover: files, commands, exit codes, commit SHA, release decision, Pages observed SHA, blockers.

**Non-negotiable:** Never substitute a screenshot or a GitHub workflow green light for live publication proof. Do not bypass failing checks, invent registry facts or report unverified deployment as done.
