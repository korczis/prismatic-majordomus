---
schema: adr/v1
id: adr-0210
kind: adr
title: "The sentence that says what Majordomus is has one declaration, and every surface reads it"
status: proposed
date: 2026-10-18
tags:
  - product-adoption
  - chatgpt-project
related:
  - file:docs/HARDCODING_LEDGER.yaml
  - file:site/data/marketing.toml
  - file:scripts/generate-site-data
  - rule:project.homepage-tells-a-declared-story
  - file:.ai/repo/adrs/0023-product-features-are-objects-of-the-layer-and-the-landing-page-is-a-projection.md
  - file:.ai/repo/project/issues/I3134.yaml
  - file:.ai/repo/project/issues/I3135.yaml
  - file:.ai/repo/project/intents/a-visitor-understands-and-checks-majordomus.yaml
provenance:
  origin: extracted
  derived_from:
    - issue:I3100
    - file:docs/HARDCODING_LEDGER.yaml
    - file:site/data/marketing.toml
    - file:scripts/generate-site-data
    - file:.ai/repo/adrs/0023-product-features-are-objects-of-the-layer-and-the-landing-page-is-a-projection.md
    - file:.ai/repo/project/issues/I3134.yaml
    - file:.ai/repo/project/issues/I3135.yaml
    - file:.ai/repo/project/intents/a-visitor-understands-and-checks-majordomus.yaml
---

# 210. The sentence that says what Majordomus is has one declaration, and every surface reads it

## Context

docs/HARDCODING_LEDGER.yaml row product-sentence-has-two-wordings records three wordings (marketing.toml hero, README bold line and lead, project.json tagline grepped from README) and waits for one decision: which wording is the declaration. The owner ordered on 2026-10-13 that the landing page reflect and derive the positioning, and the hero was rewritten on 2026-10-16 (I2745).

## Decision

The declaration lives in the site copy the homepage already reads (site/data/marketing.toml); README's bold line, page titles, Open Graph descriptions and the launch kit are projections of it or are refused by homepage-check when they differ. The wording itself is the owner's.

## Alternatives rejected

- README as the declaration (the site greps it today)
- Leaving each surface its own wording

## Consequences

I3134 and I3135 implement it; the ledger row closes.

## Sources

Promoted on 2026-10-18 from the owner's ChatGPT project (workspace `chatgpt-majordomus`, issue I3100). The owner has not accepted it; it is proposed for the owner's decision.

- `chatgpt-majordomus:c250#2`
- `chatgpt-majordomus:c250#4`
