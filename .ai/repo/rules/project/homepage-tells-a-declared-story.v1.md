---
id: project.homepage-tells-a-declared-story
version: 1
kind: rule
title: The homepage tells the story its declaration names, in that order, and the site is indexed by one declared policy
description: site/data/homepage.toml declares the homepage's sections as an ordered list of ids; the rendered homepage carries exactly those sections, in that order, each with a link or a derived figure, links no feature the product model does not call stable, and carries every command the distribution model gives a newcomer. site/data/nav.toml declares which sections are receipts under [indexing]; the pages below them are noindex and out of the sitemap, and every other page is indexable and in it.
statement: Declare the homepage's argument as an order of sections and the site's indexing as a list of receipt sections, and hold the built site to both in both directions; a section added without a place in the story, or a page indexed by accident, is refused before it is published.
status: active
class: blocking
depends_on: [project.navigation-is-declared-once@1, project.derived-once@1]
tags: [web, homepage, navigation, projections, documentation]

x-majordomus:
  tests: [scripts/ci/homepage-check, test/cases/333_homepage_narrative.sh]
---

# Rationale

A homepage drifts one defensible section at a time. This one reached twenty-two full-bleed
sections and 42,229px at 320px before anyone decided it should: every matrix, every argument
and every list had been given a band of its own, each reasonable alone, and together they
were a sitemap a visitor had to read before learning what the product was. Trimming it once
did not stop it happening again, because nothing said what the page was for.

The same was true of what search engines were shown. Of about a thousand published routes,
three quarters were the repository's audit trail — one page per issue, per capability, per
session — published at the same altitude as the product pages and listed in the same sitemap,
so a search for the product found its backlog first.

Both are the shape this repository refuses everywhere else: a structural decision nobody
wrote down, so no projection could be held to it.

# Required behaviour

The homepage's sections are declared once, as `order` in `site/data/homepage.toml`, and the
rendered page is held to that list exactly — no section rendered that the list does not name,
no section named that is not rendered, none out of order. Every declared section carries at
least one link or one derived figure. The homepage links no feature whose status in the
product model is not `stable`. Its install section carries the install, next-step and
verification commands `data/registry/distribution.json` gives.

The product inventories the homepage shows are the model's, both ways. The worker tools it
names are exactly the providers of `data/registry/product.json`: one added to the model and not
rendered is a stale page, and one rendered that the model does not name is a provider typed into
a template. Each card of its trust strip carries the value its own dataset gives — the release
the installer resolves and the command that confirms it from `data/registry/distribution.json`,
the commit this build was made from as `build.json` serves it, the executed use cases from the
catalogue — and reads `unknown` where a dataset carries nothing, so a card with nothing behind
it cannot pass as a known one.

What the homepage makes every visitor download is declared too, as `[budget]` in
`site/data/homepage.toml`: the bytes of scripts and stylesheets it loads from the site, and the
bytes of graph data it carries inline. A page over either budget, or linking an asset the build
did not produce, is refused. No page of the site loads the Mermaid runtime without a diagram to
render — the homepage did, for its whole life, and paid the heaviest file on the site for nothing.

The sections whose detail pages are receipts are declared once, as `[indexing] unlisted` in
`site/data/nav.toml`. Every page below such a section carries a robots `noindex` and is absent
from `sitemap.xml`; every other page is indexable and present in it, unless its own front
matter asks for `noindex`, in which case it is absent from the sitemap too. The pages stay
published and linked: being unlisted is about ranking, not reachability.

# Failure behaviour

`scripts/ci/homepage-check` exits 10 with one finding per violation, naming the section, the
feature, the command or the page; `scripts/site-check` reports its findings and fails with it,
so the Pages build refuses to publish a homepage or a sitemap that has drifted from its
declaration.

# Verification

`test/cases/333_homepage_narrative.sh` builds a small site tree and proves each check both ways:
a clean tree passes, and a page loading Mermaid with no diagram, scripts or inline JSON over the
declared budget, no budget declared, a linked asset missing, an undeclared section, a declared section not rendered, sections out of
order, an empty section, a draft feature linked, a missing install command, a provider of the
model the page does not name, a name the model does not carry, a trust card that disagrees with
its dataset, a trust card missing altogether, a receipt page without noindex, a receipt page in
the sitemap and an indexable page missing from it each fail with a finding that names what is
wrong.
