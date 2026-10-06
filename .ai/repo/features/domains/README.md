---
schema: context/v1
id: ai.repo.features.domains
kind: context
title: Product domains
description: The few things the product controls, one domain per file; every feature names exactly one, and each domain's members, interfaces and evidence are derived from the features that name it.
status: active
scope: subtree
providers: ["*"]
audience: [human, agent]
composition: extend
order: 110
tracks: [apps/majordomus-cli/src/product.rs, site/templates/domain.html, site/templates/domains-section.html, site/templates/partials/domain-map.html]
---

# Product domains

A **domain** is one of the few things the product controls, stated once in one Markdown
file here (ADR 0104):

```text
<id>.md      one product domain      schema: domain/v1
```

The front matter holds the title, a one-sentence `headline` (what the product does about
it), a one-sentence `problem` (the failure it answers) and a `weight` (the order of the
homepage map and of every list of domains). The body says what the domain covers and what
it does not.

A domain never lists its features. A feature names its domain in its own `domain:` field,
and the members, the interfaces they reach, the guarantees, use cases, rules and moments
behind them and the route `/domains/<id>/` are derived by the executable. A stable feature
that names no domain while any is declared, or names one that does not exist, fails
`majordomus product validate`; a domain no stable feature names is shown nowhere.

## Adding a domain

```bash
cp .ai/repo/features/domains/context.md .ai/repo/features/domains/<id>.md
$EDITOR .ai/repo/features/domains/<id>.md   # id, title, headline, problem, weight, body
$EDITOR .ai/repo/features/<feature>.md      # domain: <id> on each feature it holds
majordomus product domains                  # the members, derived
just derive                                 # the site dataset and the pages follow
```
