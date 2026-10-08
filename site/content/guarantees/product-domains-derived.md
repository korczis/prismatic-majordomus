+++
title = "The product is filed under a few domains declared once as files of the layer; a feature names exactly one, and every domain's members, interfaces, guarantees and use cases are derived from the features that name it, so a stable feature cannot be left out of the product map without the model refusing it"
description = "A domain is one Markdown file under .ai/repo/features/domains/: a title, a one-sentence"
weight = 191
[extra]
claim_id = "product-domains-derived"
status = "guaranteed"
source = "docs/claims/product-domains-derived.md"
+++
{% raw %}

## What it means

A domain is one Markdown file under `.ai/repo/features/domains/`: a title, a one-sentence
promise, the failure it answers and an order. A feature names exactly one domain in its own
`domain` field. Everything a surface says about a domain — which features it holds, the
interfaces they reach, the guarantees and use cases behind them, the failure modes they
answer, and its route — is derived from the features that name it.

The model refuses what would make a map built on domains lie by omission: a stable feature
that names no domain while any is declared, a domain that does not exist, and a stable
feature filed under a domain that is not shown.

## How it works

The kind `domain` is declared in `share/kinds.yaml` with its JSON Schema, and the source
class `domain` discovers its files through the version-control index. `crate::product`
files each feature under the domain it names and derives every domain's view from its stable
members; `product.domains` projects that to the command line, HTTP and MCP, and the site's
`product.json` carries it through an allow-list (ADR 0104).

## How to see it

```bash
majordomus product domains                      # every domain, with its derived members
majordomus product list --domain <id>           # the features filed under one
majordomus product validate                     # refuses an unfiled or misfiled feature
```

## What it does not cover

Whether a feature is filed under the *right* domain. The model checks that every stable
feature names one domain that exists and is shown; which of them describes the feature best
is a product judgement the file records and nothing measures. The domains' own prose — the
promise and the failure each answers — is authored, and is held to the site's honesty and
weight checks like any other copy, not derived.

## Why it exists

The product had two vocabularies for one map: the why-areas, which file a feature under
every failure it touches, and the homepage's own grouping, which lived in a template. A
reader could not tell from the site which features made up a part of the product, and a new
feature joined no part of it until somebody edited the page. A domain is a partition the
features declare themselves, so the map is a projection and a feature that declares nothing
is refused rather than missing.

## What proves it

`test/cases/800_product_domains.sh` builds the model against fixture layers: a feature filed
under a domain appears in it, an unfiled or misfiled stable feature is refused with the
nearest domain named, a draft domain shows nothing, and the command line, HTTP and MCP answer
with the same value. `apps/majordomus-cli/tests/product.rs` holds the domain counts to members
counted once, and `scripts/ci/homepage-check`'s `map` check refuses a homepage whose drawn
domains, members or pages are not the model's.
{% endraw %}
