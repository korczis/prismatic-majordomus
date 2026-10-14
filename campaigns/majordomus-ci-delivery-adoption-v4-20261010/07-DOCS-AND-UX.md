# M07: Docs and UX

## Goal
Make changed capabilities understandable, actionable and automatically kept current.

## Issues

## Issue V4-07-01: Developer docs migration

**Implementation sequence**
1. Update `docs/CI.md`, relevant README, `.ai/` developer guidance and diagnostic docs with actual command names and executed examples.
2. Explain fast versus full validation, gate impact/freshness, cache keys, failure recovery, perf evidence and escape hatch policy.
3. Delete stale instructions and update references; keep generated tables generated.

**Acceptance / evidence**
- [ ] Every documented command has executable example or justified exception
- [ ] Doc links and command-proof gate pass

### Required invariant per issue
- Treat existing repo model as canonical; update it rather than adding an independent source of truth.
- Link work to intent/milestone, execute focused positive+negative tests, record evidence and gate results.
- Update relevant README/ADR/rules/claims only when the repo policy requires, and derive generated docs.
- Check scope conflicts with concurrent PRs; publish only verified results. Capture previous/current timing on affected path.

## Issue V4-07-02: Site and landing delta

**Implementation sequence**
1. Determine last actually deployed site commit/version versus current verified release and list product deltas.
2. Derive feature/changelog/status snippets from canonical registry, releases and evidence; explain user benefit in accessible language.
3. Present fast PR feedback, autonomous convergence and verified delivery only as live where production checks prove it.
4. Test mobile navigation, semantic headings, no broken links, asset budget, accessibility and SEO.

**Acceptance / evidence**
- [ ] Release and homepage version come from one canonical source
- [ ] No unsupported speed/autonomy claim published

### Required invariant per issue
- Treat existing repo model as canonical; update it rather than adding an independent source of truth.
- Link work to intent/milestone, execute focused positive+negative tests, record evidence and gate results.
- Update relevant README/ADR/rules/claims only when the repo policy requires, and derive generated docs.
- Check scope conflicts with concurrent PRs; publish only verified results. Capture previous/current timing on affected path.

## Issue V4-07-03: Troubleshooting and adoption

**Implementation sequence**
1. Create source-backed runbooks for cold cache, cache poisoning, missing verdict, derive lock, stale PR, tag/release refusal and rollback.
2. Document the self-hosting and consumer install path; make it work without internal machine names or private services.
3. Generate migration guide for users on previous `.ai/` versions with compatibility tests.

**Acceptance / evidence**
- [ ] New user executes demo end-to-end
- [ ] Runbooks proven via negative fixtures

### Required invariant per issue
- Treat existing repo model as canonical; update it rather than adding an independent source of truth.
- Link work to intent/milestone, execute focused positive+negative tests, record evidence and gate results.
- Update relevant README/ADR/rules/claims only when the repo policy requires, and derive generated docs.
- Check scope conflicts with concurrent PRs; publish only verified results. Capture previous/current timing on affected path.

## Issue V4-07-04: Docs derivation invariant

**Implementation sequence**
1. Wire generated docs to existing `scripts/derive`/site generator and `derive-check`, maintaining no hand-maintained copies.
2. Write drift test changing a sample capability and proving relevant derived docs follow; reverted source restores prior output.
3. Check published site against deployed commit hash and actual source of version.

**Acceptance / evidence**
- [ ] No drift between registry, docs, site and CLI
- [ ] Golden snapshots deterministic

### Required invariant per issue
- Treat existing repo model as canonical; update it rather than adding an independent source of truth.
- Link work to intent/milestone, execute focused positive+negative tests, record evidence and gate results.
- Update relevant README/ADR/rules/claims only when the repo policy requires, and derive generated docs.
- Check scope conflicts with concurrent PRs; publish only verified results. Capture previous/current timing on affected path.

## Milestone exit gate
- [ ] Human-readable docs accurately reflect shipped behavior
- [ ] Site passes accessibility and contract checks
