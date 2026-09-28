---
schema: adr/v1
id: adr-0071
kind: adr
title: A feature exists only when its delivery is computed and every dimension passes
status: proposed
date: 2026-09-15
tags:
  - delivery
  - features
  - verification
  - publication
provenance:
  origin: authored
related:
  - file:docs/DELIVERY.md
  - file:apps/majordomus-cli/src/delivery/mod.rs
  - file:apps/majordomus-cli/src/delivery/revision.rs
  - file:apps/majordomus-cli/src/delivery/public.rs
  - file:apps/majordomus-cli/src/capability/builtin/delivery.rs
  - file:.ai/repo/ci/pages.yaml
  - file:scripts/pages
  - test:test/cases/373_delivery_is_computed.sh
  - test:test/cases/374_delivery_unknown_is_not_delivered.sh
  - file:.ai/repo/adrs/0023-product-features-are-objects-of-the-layer-and-the-landing-page-is-a-projection.md
  - file:.ai/repo/adrs/0041-proof-is-a-recorded-execution-and-inputs-unchanged-is-not-proven.md
---

# 71. A feature exists only when its delivery is computed and every dimension passes

## Context

[ADR 23](0023-product-features-are-objects-of-the-layer-and-the-landing-page-is-a-projection.md)
made a product feature an object of the layer and every fact a surface says about it a
derivation — except one. Whether the feature is *there* was never derived. A feature file
declares `status: stable`, the site renders it, and nothing distinguishes a feature that is
merged, published, tested and linked from one that is merged on a branch nobody deployed.

The words used for that difference drift the way prose drifts. "Done" meant merged to one
worker, green on CI to another, visible on the site to a third. [ADR 41](0041-proof-is-a-recorded-execution-and-inputs-unchanged-is-not-proven.md)
refused the same collapse for claims: a test file that exists and a test that ran are not
one state. The product had no equivalent, and the obvious one — a `delivered: true` or a
`tested: true` beside the feature — is a hand-written assertion that nothing produced and
nothing invalidates.

## Decision

**A feature exists only when every one of six dimensions passes**, and each dimension is a
verdict computed on every read:

```text
EXISTS = ON_MASTER ∧ DEPLOYED ∧ PUBLICLY_VERIFIED ∧ REQUIRED_TESTS_CURRENT ∧ TEST_EVIDENCE_PUBLISHED ∧ UI_LINKED
```

- **A verdict has three values, and unknown is never pass.** `pass`, `fail` and `unknown`,
  each with the sentence that decided it and a remediation. A site that could not be
  reached is not deployed; a clone without the trunk is not on master. A conjunction over
  nothing measured is not a vacuous yes.
- **Delivery and development are different types.** `DeliveryState` is `delivered` or
  `not_delivered { stage, blocking }`; the stage — `not_implemented`,
  `implemented_on_branch`, `on_master`, `tested`, `deployed` — exists only inside the second
  variant. A stage cannot be read as delivery because it cannot be put where delivery goes.
  The stage is a ladder: a rung is reached only when every rung below it passes.
- **Nothing is stored.** No feature file, no ledger line and no generated artifact carries a
  verdict. The schema of a feature is unchanged.
- **`ON_MASTER` is read from git.** The paths that implement a feature are derived from what
  its file already names: the source file of each capability module (the registry's
  `source_path`), `lib/<command>.sh` of each shell command (the file `bin/majordomus`
  sources to dispatch it), and the `implementation` of each claim. The latest commit
  touching them at `HEAD` must be an ancestor of `origin/<trunk>`, or the paths must be
  identical on both — a squash merge carries the content without the commit. A change only
  on the current branch is `implemented_on_branch`.
- **`DEPLOYED` is an ancestry check against the public identity.** The site serves the
  document `deploy.identity` of `.ai/repo/ci/pages.yaml` names; the commit in it must
  contain the feature's trunk revision. The identity is fetched once per report, because
  containment is not the question `scripts/pages verify` answers — it answers equality.
- **`PUBLICLY_VERIFIED` reuses `scripts/pages`, and does not re-derive it.** Deployed, a
  clean identity (`dirty: false`), `scripts/pages verify --timeout 0` confirming the site
  from outside, and `scripts/pages built` reading GitHub's own build of the published
  branch. Their exit codes are the vocabulary: 0 pass, 10 fail, 12 unknown.
- **The network is injectable.** `MAJORDOMUS_DELIVERY_SITE_URL` replaces the site's
  address; a `file://` directory with an identity document is a site both this module and
  `scripts/pages verify` read through the same `curl`, so the behavioural cases never reach
  the network.
- **Two capabilities, one declaration.** `delivery.report` (every feature, ordered by id)
  and `delivery.feature` (one), projected to the command line (`delivery report`,
  `delivery show`), HTTP, OpenAPI and MCP. Both are reads; both are waived from the
  benchmark as an external dependency and not cached.

### Phase 1

This record lands the model and the three dimensions that need no recorded evidence.
`REQUIRED_TESTS_CURRENT`, `TEST_EVIDENCE_PUBLISHED` and `UI_LINKED` are computed as
`unknown` with the reason *evidence model lands with PR #577*. They are dimensions of the
type now so that phase 2 fills them instead of widening it — and so that, until it does, no
feature can be reported as existing. `EXISTS` is false for every feature on the day this
lands, and that is the true answer.

The public site does not show delivery yet. `PUBLIC_FEATURE_FIELDS` in `src/product.rs` is
the allow-list of what a public page may carry; the site's status slice is a later change
that adds to it deliberately.

## Consequences

- "Is it done?" has one answer with a derivation behind it: `majordomus delivery show <id>`.
- A merged feature the site has not caught up with reads `on_master` with `deployed: fail`
  and the served commit named — the silent lag that `scripts/pages built` was written for
  becomes visible per feature.
- The report reaches the network and GitHub. On a machine without either the publication
  dimensions are `unknown`, which is correct and slower to read than a cached answer would
  be; a cache here would answer about a publication that may have moved.
- The path derivation is shallow, as ADR 41's is: a change to a helper no module, command or
  claim names does not move a feature's revision. A feature that names nothing implementable
  fails `on_master` with the remediation to name it.

## Alternatives rejected

- **`status: delivered` or `tested: true` in the feature file.** The defect in one field: a
  person's assertion that nothing produced and nothing can invalidate.
- **A boolean `exists` beside a stage.** It permits `exists: true` with `stage: on_branch`.
  Two variants of one type make that combination unrepresentable.
- **Deciding `DEPLOYED` with `scripts/pages verify --commit <revision>`.** It asks whether the
  site serves exactly that commit, so a feature merged before the last publication would read
  as not deployed. Containment is the question, and the one read it needs is the identity.
- **Treating an unreachable site as the last known state.** It is the collapse of unknown into
  pass that this record exists to refuse.
