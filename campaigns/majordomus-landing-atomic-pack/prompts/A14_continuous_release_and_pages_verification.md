# A14: Continuous release and Pages verification

**Depends on:** A03,A05,A06,A07,A08,A09,A10,A11,A12,A13. **Scope:** One coherent reviewable atom.

## Assignment to Opus/Fable
Verify existing fast-path YAML matches `scripts/pages paths` and `.ai/repo/ci/pages.yaml`; keep canonical build/check and scheduled repair. Enforce source SHA → generated data hash → gh-pages publication → GitHub built → HTTP served SHA + smoke + docs. Include bounded retry and clear pending state. Benchmark existing SLO, optimize cold path only with evidence.

## Deliverables
A reusable release checkpoint in repo-native tooling (not another orchestration layer), Pages diagnostics, runnable dry-run evidence.

## Proof / acceptance gate
Simulated stale/missing public state blocks success; GitHub errors typed; no gh-pages force-push or bypass.

## Release checkpoint
PUBLIC FINAL SLICE: controlled release, production SHA+docs verified.

## Execution contract
1. Read the master prompt and verified repository anchors. Inspect current files, canonical commands, and relevant `.ai` rules. Do not guess schema/CLI names.
2. Specify smallest implementation and tests, including at least one adversarial/negative case where data contracts change.
3. Implement, regenerate via existing tooling, execute focused and canonical validation; repeat to check drift/idempotence.
4. Review provenance, source links, public/private filtering and performance if affected.
5. Document meaningful architectural invariants, and extend real `.ai` enforcement when generalized policy changes.
6. Observe versioning doctrine and PR checks. Merge and progressively deploy eligible stable public slice, then validate published commit with existing `scripts/pages` commands.
7. Record exact evidence in intent/issue/session handover: files, commands, exit codes, commit SHA, release decision, Pages observed SHA, blockers.

**Non-negotiable:** Never substitute a screenshot or a GitHub workflow green light for live publication proof. Do not bypass failing checks, invent registry facts or report unverified deployment as done.
