+++
title = "Every page of the site is offered in every declared language, derived from the English and held to it in a browser"
description = "Every page of the site is offered in every declared language, derived from the English and held to it in a browser"
weight = 176
[extra]
kind = "rule"
slug = "project-the-site-is-offered-in-every-language-1"
identity = "project.the-site-is-offered-in-every-language@1"
status = "active"
source = ".ai/repo/rules/project/the-site-is-offered-in-every-language.v1.md"
+++
{% raw %}

## Rationale

On 2026-10-19 the owner ordered the site to be always fully multilingual, Czech and English at the least, derived and translated automatically, and never maintained twice (ADR 0218). The first Czech copy was published with its top bar wrapped onto a second line at 1280 px: the Czech labels were longer than the English ones, and a row of buttons had been translated as one sentence. Nothing measured the copy against its original, so nothing refused it. A copy derived by a machine is held by a machine, or it is not held.

## Required behaviour

- Every page the site builds is copied into every declared language by `scripts/site-build`; no page of another language is written by hand, except the authored pages `project.english-only` declares, which are carried unchanged and named canonical by their copies.
- In every copy, every link lands on a file the build wrote, and a link to a page that has a copy in that language leads to that copy.
- The switch names every declared language, leads to each copy and marks the one being read; the alternates name every copy, and each names this page back.
- At every width the design declares, the top bar of a copy has the height and the rows of the English page's bar, and nothing in it overflows.

## Failure behaviour

The `site-languages` gate (`scripts/lib/site-i18n-probe.mjs`) fails the change and names the page, the kind of finding and what it measured. A translation that would break the bar is fixed in the translation memory, never by excluding the page or the width.

## Verification

`scripts/lib/site-i18n-probe.mjs` loads every page of every copy in Chromium, answered from the build itself with no network, and holds each to the four requirements above. `test/cases/3590_the_site_is_derived_in_every_language.sh` proves the pass that writes the copies: translated segments, restored placeholders, rewritten links, refused translations, the switch, the notice, the canonical URL and a second pass that changes nothing.
{% endraw %}
