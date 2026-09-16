# Delivery

Whether a product feature **exists** is computed, never recorded. A feature exists only when
every one of six dimensions passes:

```text
EXISTS = ON_MASTER ∧ DEPLOYED ∧ PUBLICLY_VERIFIED ∧ REQUIRED_TESTS_CURRENT ∧ TEST_EVIDENCE_PUBLISHED ∧ UI_LINKED
```

Anything short of that is **not delivered**. The decision is
[ADR 71](../.ai/repo/adrs/0071-a-feature-exists-only-when-delivery-is-computed-and-every-dimension-passes.md);
the code is `apps/majordomus-cli/src/delivery/`.

## Reading it

```bash
majordomus delivery report               # every feature, one row per feature
majordomus delivery report --check       # exit 10 unless every feature exists
majordomus delivery show evidence        # one feature: each dimension, its reason, its remedy
majordomus delivery report --format json # the document GET /api/v1/delivery returns
```

The same answers are `GET /api/v1/delivery`, `GET /api/v1/delivery/feature?id=<id>` and the
MCP tools `majordomus_delivery` and `majordomus_delivery_feature`. One declaration,
`capability/builtin/delivery.rs`, projects all of them.

## Verdicts

Every dimension is `pass`, `fail` or `unknown`, with the sentence that decided it and a
remediation. **Unknown is never pass.** A site that could not be reached is not deployed; a
clone without `origin/master` is not on master.

## States

A feature is `delivered` or `not_delivered`. Only the second carries a **development stage**,
and the stage is a different type from delivery:

| stage | means |
|-------|-------|
| `unknown` | git could not say where its implementation is |
| `not_implemented` | it names no implementation, or nothing committed one |
| `implemented_on_branch` | its latest implementation revision is not on the trunk |
| `on_master` | its implementation is on the trunk |
| `tested` | on the trunk, every required test current |
| `deployed` | tested, and contained in what the public site serves |

The stage is a ladder: a rung is reached only when every rung below it passes, so a feature
whose tests are unknown is `on_master` even when the site already serves it. `blocking` lists
the dimensions that do not pass.

## The dimensions

### `on_master`

The paths a feature is implemented by are derived from what its file names, never written
down: the source file of each capability module, `lib/<command>.sh` of each shell command,
and the `implementation` of each claim in `docs/CLAIMS.yaml`. The latest commit touching them
at `HEAD` must be an ancestor of `origin/<trunk>`, or the paths must be byte-identical on
both. A change only on the current branch fails with `implemented_on_branch`; a feature
naming nothing implementable fails with the remediation to name it; a clone without the
trunk is unknown.

### `deployed`

The public site serves the identity document `.ai/repo/ci/pages.yaml` names in
`deploy.identity` (`build.json`), at the `base_url` of `site/config.toml`. Deployed means the
commit in that document contains the feature's trunk revision. It is read once per report.
An unreachable site, or a served commit this clone does not have, is unknown.

### `publicly_verified`

Deployed, and the publication verified from outside by the script that owns it:

1. the identity says its tree was clean (`dirty: false`);
2. `scripts/pages verify --url URL --commit SHA --timeout 0` confirms the site serves it;
3. `scripts/pages built --commit <origin/gh-pages>` reports GitHub's own build as built.

Exit 10 of either script is a fail, exit 12 is unknown.

### `required_tests_current`, `test_evidence_published`, `ui_linked`

Phase 1 computes these as `unknown`, reason *evidence model lands with PR #577*. They exist
in the type so that phase 2 fills them — and so that, until it does, **no feature exists**.

## Hermetic runs

`MAJORDOMUS_DELIVERY_SITE_URL` replaces the site's address. A `file://` directory holding a
`build.json` is a site that both the delivery module and `scripts/pages verify` read through
the same `curl`: `test/cases/373_delivery_is_computed.sh` and
`test/cases/374_delivery_unknown_is_not_delivered.sh` never touch the network.

## Not yet

The public site does not render delivery. `PUBLIC_FEATURE_FIELDS` in `src/product.rs` is the
allow-list of what a public page carries, and the status slice that adds delivery to it is a
later change.
