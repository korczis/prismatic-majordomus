# Majordomus Public Surface: Atomic Autonomous Execution Pack

Target: latest `korczis/prismatic-majordomus`, with `master` as observed default publication branch. The checkout and canonical `.ai` are authoritative; never blindly trust a prompt's assumptions about filesystem, release policy, or runtime.

## Real repository anchors verified during pack preparation

- `site/data/homepage.toml` owns the ordered homepage chapters (`hero`, `recognise`, `map`, `how`, `refuses`, `surfaces`, `proof`, `install`) and asset budgets (`assets_bytes = 340000`, `inline_json_bytes = 8000`). `scripts/ci/homepage-check` enforces bidirectional agreement.
- `site/data/marketing.toml` owns editorial positioning, not generated counts, capabilities or versions; `scripts/site-check` enforces its line budget and rejects numeric claims.
- `docs/PRODUCT.md`, `.ai/repo/features/`, `apps/majordomus-cli/src/product.rs`, `site/data/registry/product.json` express the one-feature/one-model projection, enforced via `project.product-surface-derived` and ADR 0023.
- `scripts/generate-site-data`, `scripts/site-build`, `scripts/site-check`, `scripts/pages` are existing generation / publication entry points. `scripts/pages` advertises `build`, `check`, `fingerprint`, `current`, `paths`, `budget`, `benchmark`, `built`, `rebuild`, `verify`, and `verify-docs` subcommands.
- `.github/workflows/pages.yml` is a fast-path adapter over `scripts/pages` and uses GitHub Pages with `gh-pages`. `.ai/repo/ci/pages.yaml` is the model cited in that workflow. Existing scheduled publication repair must be preserved.
- Never presume a Pages deployment can be complete on merge alone: confirm the externally served commit and smoke routes with canonical verification.

## Definition of 'atomic'

Each prompt is one reviewable, testable, releaseable vertical change with explicit sources, files to *discover*, validation, and output artifacts. Split further if necessary, not into layers that cannot deploy. Every completed slice produces: evidence → canonical gates → release decision → push/merge using repo workflow → Pages publication proof → updated state/progress ledger. For a slice that has no public-output change, record N/A for publication rather than manufacturing a dummy Pages build.

## Truth/safety constraints

- Work through Majordomus native intent/milestone/issues/session/mesh tooling *if actually present*. Do not fabricate commands.
- A passing unit test is not a proof of published Pages. A successful Pages push is not a proof of externally visible version.
- An absent GitHub permission, missing secret, unavailable remote, failed gate, or external outage must become a **typed blocker with exact evidence**. Never mark green, never bypass gates or branch protection, and continue only safe independent work.
- Version bumps are determined by existing Elm-like / semantic / release doctrine: *do not presume every atom merits minor*. Batch atoms when policy does not permit individual release; record why.
- Update `.ai` after discovering a general reusable invariant, not by adding a cosmetic rule per UI widget. Prefer existing validators and executable gates; no decorative governance.
- No synthetic statistics, screenshots represented as live proof, fabricated demos, duplicate inventories, or hard-coded versions.
- No React/SPAs, backend rewrite or wholesale stack migration; use existing Zola/Tailwind/Flowbite/Alpine/Cytoscape/ECharts selectively when supported and justified.

## The release loop, applied after each eligible atom

1. Rebase/sync with canonical branch, inspect outstanding work and worktrees; avoid overlapping file ownership and racey changes.
2. Record intent criterion, impacted canonical entities, and proof plan.
3. Implement smallest vertical slice; add negative, positive, and cross-surface tests.
4. Regenerate once via canonical tooling; validate `scripts/pages current`, site build/check, appropriate `.ai`/schema/design/CI tests. Rebuild is not needed for purely non-site changes unless gates require it.
5. Re-run the same generation/check sequence and confirm idempotence; inspect diff for generated drift or private data leakage.
6. Use current version/release decision policy; bump only when required by it; generate release notes/provenance when appropriate. Re-run gates after bump.
7. Commit, PR, review, merge, push using existing repo controls. Never force-push protected branches or circumvent checks.
8. If public-affecting: use canonical Pages path and `scripts/pages verify --url <canonical-public-url> --commit <deployed-sha> --smoke` plus `scripts/pages verify-docs ...` as applicable. Confirm deployment `built` state. Record delayed/unavailable state as pending, not success. If existing retry/remedy mechanism is appropriate, use it without duplicate triggering.
9. Update milestone criterion, issue, evidence ledger, session handover, docs and `.ai` only as justified by generalized invariant. Publish changelog/status summary.
10. Start next independent atom. A failed deployment does not justify rolling additional unverified public changes blindly into an unknown state.

Do not retry infinitely: bounded attempts, retain exact exit codes and proof. Keep prompt-file names and atom IDs in your progress reports.


---

# MASTER PROMPT — Autonomous Public Surface Release Train

You are Claude Opus/Fable operating in `korczis/prismatic-majordomus`. Read `00_README.md` and execute atoms A00–A15 in dependency order. Goal: polished, mobile-first, project-oriented public site + GH Pages + docs, *all derived/inferred/proved*, with automatic `.ai` enforcement and progressive, verified releases.

### Mandatory preflight
- Refresh repo, inspect branch, worktrees, PRs, current CI/deploy, `.ai`, AGENTS/CLAUDE instructions, current metadata/schema and site. Record current HEAD and published commit. Do not assume deployment is clean.
- Locate exact executable gates; verify `scripts/pages` help/implementation and the CI model. Capture baseline durations, asset size, performance, a11y and mobile issues. Read docs/PRODUCT.md and relevant ADRs.
- Register one top-level intent and milestones with acceptance criteria/evidence using actual Majordomus primitives. If the primitives are absent, file this in the existing native work tracking architecture; don't invent a new tracker.
- Produce an ownership/dependency graph from `manifest.json`; run non-conflicting atoms in parallel through the existing mesh only if it demonstrably handles leases, files and handoffs. Otherwise run sequentially. Do not wait for a person on ordinary choices.

### Workflow discipline
For each atom: inspect → plan → implement → negative tests → positive tests → docs → `.ai` generalized rule/gate update where warranted → local verification → merge/release decision → deployed commit verification where applicable → evidence and handover. Stop that atom at a real blocker, isolate it, and continue independent atoms only if safe.

### Constraints
Existing `homepage.toml` is declarative story order, `marketing.toml` is editorial, public inventories/counts live in canonical registries. Existing Flowbite/Tailwind/design tokens and `scripts/pages` are foundations, not disposable legacy. Do not create second content model, handcrafted search catalogue, fake transcripts, separate navigation arrays, or a new CI pipeline. Validate machine-enforced status of every claim, and distinguish built from planned. New features and public objects should propagate with zero consumer-specific registrations. No leaking private/local `.ai`, secrets, addresses or absolute filesystem paths into public assets.

### Release cadence
Deploy after every coherent **public-affecting, validated vertical slice** (e.g. IA+navigation, proof UX, explorer, docs/search, mobile polish) rather than after every tiny commit. For slices merged concurrently, deployment is of their combined source commit. Version bump per *actual* doctrine, not arbitrary minor per edit. Verify live commit and route set. A local build or successful push is not sufficient. Never claim success without links/hashes/logs.

### Completion contract
- All atoms passed or have explicit external blockers, all deliverables checked, no source-of-truth duplication, no regression on existing homepage/prod model constraints.
- Public Pages serves the final merged source SHA, docs and representative routes return expected content, release metadata agrees.
- Demo proof navigable: problem → feature → guarantee → executable evidence → source. One new canonical object propagates into relevant docs/search/nav without manual consumer changes.
- Final evidence report includes per-atom status, source files, test commands/outcomes, `.ai` changed validators/gates, release/version actions, PR and merge SHAs, Pages verified SHA, timings and known blockers. Never conflate 'implemented', 'merged' and 'live'.

Execute A00 first, then the dependency graph. No analysis-only stopping point.


---

# Release checkpoints and gates

## Critical path
A00 → A01 → A02 → A03 → A04 → A05 → A06/A07/A08 → A09 → A10/A11 → A12 → A13 → A14 → A15.
The manifest is the authority for dependencies; this line only sketches the flow.

## Recurring release gates
- **Truth:** source reference and public scope valid; generated data current; claim status backed by evidence.
- **Correctness:** `.ai` machine validators, domain/schema/product checks, homepage check, site build and site check, tests appropriate to atom.
- **UX:** interaction semantics, keyboard/mobile and no-JS baseline; static assets within declared budgets.
- **Safety:** no local/private paths, credentials, hidden `.ai` data, or fake runtime demos.
- **Delivery:** PR/branch protections pass; version bump follows policy; canonical Pages publish and external HTTP verification of merged commit.

## Decision states
`planned → implemented → locally_proved → merged → release_prepared → publication_pending → published_verified` or `blocked` with provenance. Never jump from `locally_proved` to `published_verified`.

## Example release evidence (template, never prefilled)
```yaml
atom: A06
source_sha: <actual SHA>
version_decision: <bump / no bump with rule>
local_gates:
  - command: <actual command>
    exit_code: <actual integer>
pr: <actual link>
merged_sha: <actual SHA>
pages:
  status: <pending / verified / blocked>
  public_url: <resolved canonical URL>
  observed_sha: <actual SHA, never guessed>
  smoke: <pass/fail/pending>
blockers: []
```


---

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


---

# A01: Intent, milestone and proof governance

**Depends on:** A00. **Scope:** One coherent reviewable atom.

## Assignment to Opus/Fable
Express outcome as canonical intent: comprehensible product, project adoption, navigable evidence, zero duplication, mobile reliability, deterministic publication. Derive milestone/issue decomposition and measurable acceptance evidence using real intent primitives, not new YAML tracker. Make live deployment a distinct criterion.

## Deliverables
Typed intent/plan/evidence ownership, per-atom criteria and linked issue/milestones, explicit blocker taxonomy.

## Proof / acceptance gate
Intent validates and work links resolve; cannot mark criterion done without evidence; negative regression test where appropriate.

## Release checkpoint
Governance-only: follow version/release policy, no artificial Pages publication.

## Execution contract
1. Read the master prompt and verified repository anchors. Inspect current files, canonical commands, and relevant `.ai` rules. Do not guess schema/CLI names.
2. Specify smallest implementation and tests, including at least one adversarial/negative case where data contracts change.
3. Implement, regenerate via existing tooling, execute focused and canonical validation; repeat to check drift/idempotence.
4. Review provenance, source links, public/private filtering and performance if affected.
5. Document meaningful architectural invariants, and extend real `.ai` enforcement when generalized policy changes.
6. Observe versioning doctrine and PR checks. Merge and progressively deploy eligible stable public slice, then validate published commit with existing `scripts/pages` commands.
7. Record exact evidence in intent/issue/session handover: files, commands, exit codes, commit SHA, release decision, Pages observed SHA, blockers.

**Non-negotiable:** Never substitute a screenshot or a GitHub workflow green light for live publication proof. Do not bypass failing checks, invent registry facts or report unverified deployment as done.


---

# A02: Canonical public presentation schema

**Depends on:** A00,A01. **Scope:** One coherent reviewable atom.

## Assignment to Opus/Fable
Review product registry, feature schema, site data and design model. Define tiny typed public entity projection with IDs, visibility/status, source/proof and relations using existing primitives. Disallow source-list duplication. Reuse stable deterministic ordering. Keep generated artifacts under existing generator.

## Deliverables
Shared public projection and schema/drift tests; migration of relevant existing entries without adding another registry.

## Proof / acceptance gate
Randomized discovery produces identical output; private objects filtered; duplicate/missing references fail with actionable errors.

## Release checkpoint
Deploy if public projection pages change; verify live canonical SHA.

## Execution contract
1. Read the master prompt and verified repository anchors. Inspect current files, canonical commands, and relevant `.ai` rules. Do not guess schema/CLI names.
2. Specify smallest implementation and tests, including at least one adversarial/negative case where data contracts change.
3. Implement, regenerate via existing tooling, execute focused and canonical validation; repeat to check drift/idempotence.
4. Review provenance, source links, public/private filtering and performance if affected.
5. Document meaningful architectural invariants, and extend real `.ai` enforcement when generalized policy changes.
6. Observe versioning doctrine and PR checks. Merge and progressively deploy eligible stable public slice, then validate published commit with existing `scripts/pages` commands.
7. Record exact evidence in intent/issue/session handover: files, commands, exit codes, commit SHA, release decision, Pages observed SHA, blockers.

**Non-negotiable:** Never substitute a screenshot or a GitHub workflow green light for live publication proof. Do not bypass failing checks, invent registry facts or report unverified deployment as done.


---

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


---

# A04: Shared design tokens and components

**Depends on:** A02. **Scope:** One coherent reviewable atom.

## Assignment to Opus/Fable
Inspect design tokens, generated design projection and Flowbite usage. Establish reusable heading, cards, evidence, status, command, breadcrumb, provenance and relation components with token-backed colors/density. No duplicate CSS palettes or full framework rewrite.

## Deliverables
Minimal semantic component library reused by landing/docs and design snapshots.

## Proof / acceptance gate
Design generator/check idempotent; semantic color contrast and no component-specific invented product data.

## Release checkpoint
Deploy with next visible slice if no standalone user-facing improvement; do not bump for CSS-only atom contrary to policy.

## Execution contract
1. Read the master prompt and verified repository anchors. Inspect current files, canonical commands, and relevant `.ai` rules. Do not guess schema/CLI names.
2. Specify smallest implementation and tests, including at least one adversarial/negative case where data contracts change.
3. Implement, regenerate via existing tooling, execute focused and canonical validation; repeat to check drift/idempotence.
4. Review provenance, source links, public/private filtering and performance if affected.
5. Document meaningful architectural invariants, and extend real `.ai` enforcement when generalized policy changes.
6. Observe versioning doctrine and PR checks. Merge and progressively deploy eligible stable public slice, then validate published commit with existing `scripts/pages` commands.
7. Record exact evidence in intent/issue/session handover: files, commands, exit codes, commit SHA, release decision, Pages observed SHA, blockers.

**Non-negotiable:** Never substitute a screenshot or a GitHub workflow green light for live publication proof. Do not bypass failing checks, invent registry facts or report unverified deployment as done.


---

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


---

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


---

# A07: Canonical relationship graph explorer

**Depends on:** A02,A04. **Scope:** One coherent reviewable atom.

## Assignment to Opus/Fable
Build narrowly scoped graph JSON projection from canonical intent/feature/claim/surface relations that truly exist; choose Cytoscape only if bundle budget permits. Stable IDs and deterministic layout; graph/list fallback; click node→source/claim/proof; touch/keyboard support; no hand-entered graph.

## Deliverables
Graph UI plus list view and typed relation dataset.

## Proof / acceptance gate
Cross-surface entity identity test, orphan-edge failure, zero-registration fixture and mobile graph fallback.

## Release checkpoint
PUBLIC SLICE: publish and verify interactive + static fallback.

## Execution contract
1. Read the master prompt and verified repository anchors. Inspect current files, canonical commands, and relevant `.ai` rules. Do not guess schema/CLI names.
2. Specify smallest implementation and tests, including at least one adversarial/negative case where data contracts change.
3. Implement, regenerate via existing tooling, execute focused and canonical validation; repeat to check drift/idempotence.
4. Review provenance, source links, public/private filtering and performance if affected.
5. Document meaningful architectural invariants, and extend real `.ai` enforcement when generalized policy changes.
6. Observe versioning doctrine and PR checks. Merge and progressively deploy eligible stable public slice, then validate published commit with existing `scripts/pages` commands.
7. Record exact evidence in intent/issue/session handover: files, commands, exit codes, commit SHA, release decision, Pages observed SHA, blockers.

**Non-negotiable:** Never substitute a screenshot or a GitHub workflow green light for live publication proof. Do not bypass failing checks, invent registry facts or report unverified deployment as done.


---

# A08: Feature/domain/guarantee drill-down

**Depends on:** A02,A04,A06. **Scope:** One coherent reviewable atom.

## Assignment to Opus/Fable
Revise existing feature/domain/guarantee templates, not rebuild product registry. Provide clear problem→feature→surface→guarantee→rule/test→source trails. Status semantics sourced from claims; clear advisory vs enforced. Filterable views with Alpine progressive enhancement.

## Deliverables
Navigable product explorer with provable support paths.

## Proof / acceptance gate
Every public entity exactly one page, every relation resolves, no invented guarantee or status. Assert representative journey E2E.

## Release checkpoint
PUBLIC SLICE: live proof links and entity pages verified.

## Execution contract
1. Read the master prompt and verified repository anchors. Inspect current files, canonical commands, and relevant `.ai` rules. Do not guess schema/CLI names.
2. Specify smallest implementation and tests, including at least one adversarial/negative case where data contracts change.
3. Implement, regenerate via existing tooling, execute focused and canonical validation; repeat to check drift/idempotence.
4. Review provenance, source links, public/private filtering and performance if affected.
5. Document meaningful architectural invariants, and extend real `.ai` enforcement when generalized policy changes.
6. Observe versioning doctrine and PR checks. Merge and progressively deploy eligible stable public slice, then validate published commit with existing `scripts/pages` commands.
7. Record exact evidence in intent/issue/session handover: files, commands, exit codes, commit SHA, release decision, Pages observed SHA, blockers.

**Non-negotiable:** Never substitute a screenshot or a GitHub workflow green light for live publication proof. Do not bypass failing checks, invent registry facts or report unverified deployment as done.


---

# A09: Docs architecture and connected reference

**Depends on:** A05,A08. **Scope:** One coherent reviewable atom.

## Assignment to Opus/Fable
Converge landing/docs presentation, generated CLI/MCP/API and knowledge documentation. Use canonical capability registry / OpenAPI, not duplicate endpoint/tool lists. Docs route and front-matter navigation metadata derived where supported. Preserve ADR and public-source citation links.

## Deliverables
Unified docs IA with related-entity trails; generated reference, correct canonical/version/source link metadata.

## Proof / acceptance gate
Generator parity tests CLI/API/MCP, link and anchor checks, no orphan docs and no status inflation.

## Release checkpoint
PUBLIC SLICE: `scripts/pages verify-docs` against published SHA.

## Execution contract
1. Read the master prompt and verified repository anchors. Inspect current files, canonical commands, and relevant `.ai` rules. Do not guess schema/CLI names.
2. Specify smallest implementation and tests, including at least one adversarial/negative case where data contracts change.
3. Implement, regenerate via existing tooling, execute focused and canonical validation; repeat to check drift/idempotence.
4. Review provenance, source links, public/private filtering and performance if affected.
5. Document meaningful architectural invariants, and extend real `.ai` enforcement when generalized policy changes.
6. Observe versioning doctrine and PR checks. Merge and progressively deploy eligible stable public slice, then validate published commit with existing `scripts/pages` commands.
7. Record exact evidence in intent/issue/session handover: files, commands, exit codes, commit SHA, release decision, Pages observed SHA, blockers.

**Non-negotiable:** Never substitute a screenshot or a GitHub workflow green light for live publication proof. Do not bypass failing checks, invent registry facts or report unverified deployment as done.


---

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


---

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


---

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


---

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


---

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


---

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
