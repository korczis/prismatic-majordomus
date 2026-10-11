# A10: Public-safe search and keyboard UX

**Depends on:** A02,A05,A09. **Scope:** One coherent reviewable atom.

## Assignment to Opus/Fable
Build small deterministic static search index from public canonical pages and selected entity metadata. Use existing built-in search where possible; no remote backend or private data. Keyboard shortcut `/` or Cmd/Ctrl+K with accessible dismissal and empty states. Respect asset budgets.

## Deliverables
Search index, page-level affordance, responsive accessible search dialog and searchable docs/features/guarantees.

## Proof / acceptance gate
Private-data regression fixtures; search results resolve; ranking deterministic; no index drift; basic E2E keyboard + mobile.

## Release checkpoint
PUBLIC SLICE: publication and route/index smoke.

## Execution contract
1. Read the master prompt and verified repository anchors. Inspect current files, canonical commands, and relevant `.ai` rules. Do not guess schema/CLI names.
2. Specify smallest implementation and tests, including at least one adversarial/negative case where data contracts change.
3. Implement, regenerate via existing tooling, execute focused and canonical validation; repeat to check drift/idempotence.
4. Review provenance, source links, public/private filtering and performance if affected.
5. Document meaningful architectural invariants, and extend real `.ai` enforcement when generalized policy changes.
6. Observe versioning doctrine and PR checks. Merge and progressively deploy eligible stable public slice, then validate published commit with existing `scripts/pages` commands.
7. Record exact evidence in intent/issue/session handover: files, commands, exit codes, commit SHA, release decision, Pages observed SHA, blockers.

**Non-negotiable:** Never substitute a screenshot or a GitHub workflow green light for live publication proof. Do not bypass failing checks, invent registry facts or report unverified deployment as done.
