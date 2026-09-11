+++
title = "The website's homepage and feature pages name no feature, module, command, provider or count of their own, and a stale product dataset fails the build before it can be deployed"
description = "The website's homepage, its /features/ pages and its capability matrix name no feature, no"
weight = 165
[extra]
claim_id = "landing-page-is-a-projection"
status = "guaranteed"
source = "docs/claims/landing-page-is-a-projection.md"
+++
{% raw %}

## What it means

The website's homepage, its `/features/` pages and its capability matrix name no feature, no
capability module, no public command, no provider and no count. Every one of those comes from
`site/data/registry/product.json`, which the executable writes from the features of the layer,
the capability registry, the command registry, the Why catalogue and the web topology. The
only hand-written words on the surface are positioning sentences and button labels in
`site/data/marketing.toml`, held to a line budget and carrying no number and no capability
claim.

A stale dataset cannot be deployed: `majordomus generate --check` fails a tree whose
`product.json` does not match the repository, so the homepage cannot describe an executable
this repository does not have.

## How it works

The templates read the dataset and render it. `scripts/site-check` decides the rest: it
refuses a template, a generator, a jq program or a navigation file that names a feature route
or a provider's own bootstrap or client-configuration file; it proves every public feature has
its page and every page under `/features/` was produced by a feature; it compares the marks
the matrix page shows with the marks the model derived, both directions; and it checks that
the telemetry the homepage prints is the dataset's.

Routes that moved are declared once, in `nav.toml` under `[[redirects]]`, and become Zola
aliases on the page they point at; a redirect whose target the generator did not produce fails
the build rather than publishing a link into nothing.

## How to see it

```bash
scripts/derive && scripts/derive-check       # the dataset matches the repository
scripts/site-build && scripts/site-check     # the page matches the dataset
bash test/run.sh 12_site_build
```

## What it does not cover

Which features are chapters of the homepage and in what order is editorial, and is declared in
the feature files themselves (`featured`, `weight`) rather than in the page. The positioning
sentences are written by a person; what is checked is that they carry no number and no
capability claim, not that they are true.

## Why it exists

The homepage was the last hand-maintained inventory in a repository where every other surface
is a projection. The gap between what the product did and what its public face said grew in
exactly the direction nobody measured, because nothing could fail when the two disagreed.
{% endraw %}
