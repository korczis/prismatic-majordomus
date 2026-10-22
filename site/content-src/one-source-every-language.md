+++
title = "One source, every language"
description = "Nobody maintains a second version of majordomus.dev. How every language of the site is derived from the English build and a memory of translated segments, held to its original in a real browser, and where a hand-written translation is admitted as a second source because a gate holds it to the first."
template = "page.html"
[extra]
lang = "en"
related = ["written-once-derived-everywhere/", "proof-carrying-documentation/", "a-doubt-is-settled-by-an-experiment/"]
[extra.translations]
cs = "jeden-zdroj-kazdy-jazyk/"
+++

On 19 October 2026 the owner ordered that majordomus.dev be, from then on, always fully multilingual, Czech and English at the least, with a language switch, and that every language be derived and translated automatically. In the words [ADR 0218](@/adrs/adr-0218.md) (accepted) records: "I do not want to maintain two language versions; it must work automatically, derived, inferred." Nearly every page of the site was already generated from English records, so the question was how a second language could exist without anyone writing it twice.

## Three ways, and the one taken

The decision weighs three. A second content tree, a Czech file beside every page, would have made every generator write twice and every template key its strings, while the data the templates read still rendered in English: the two versions the owner refused, written by machines instead of people. Translation at build time, by asking a model during the build, would have made two builds of one commit differ, given CI a credential to hold and let a model outage take the site down. The third is the one taken: a pass over the built pages and a memory of translated segments.

## A pass over the built site

After Zola, the accessibility pass and the diagrams, `scripts/site-build` runs `scripts/lib/site-i18n.mjs` over every page it wrote. A page is cut into segments: each run of inline content between block boundaries, and each attribute a reader meets, such as a title, an alt text or the page's description. Code, preformatted text, keyboard input, SVG and anything marked `translate="no"` are never a segment. The copy under `/cs/` is the English page with every segment the memory knows replaced, every internal link pointed at its counterpart, the page's language and canonical address set, the alternates added and a notice that the text is machine translated. Because the pass reads the rendered HTML, it covers templates, generated data and content alike, and a template has nothing to do to be translated.

## A memory keyed by the English

The memory, under `site/i18n/<lang>/memory/`, holds one entry per segment, keyed by a hash of the English with its inline markup replaced by numbered placeholders. The same words in two templates are one entry. A changed English segment has a new key, so a stale translation is never served: the segment falls back to English until it is translated again. A translation is admitted only with the structure of its source, every placeholder exactly once and nested as in the English, so a translator can reorder words and can never drop a link or a piece of code.

Translation is a separate, automatic step outside the build. The build lists what the memory lacks and publishes its coverage at `/cs/i18n.json`. `scripts/site-translate run` asks a model through the advisor adapters of [ADR 0098](@/adrs/adr-0098.md), the same adapters a reasoning record uses, and its export and import carry batches to translators outside the process. The glossary and the style a translator is given live beside the memory, in the language's guide, and the directory declares its language in an edition file, which is how [project.english-only](@/rules/project-english-only-4.md) admits text that is not English.

## Held to its original in a browser

The blocking rule [project.the-site-is-offered-in-every-language](@/rules/project-the-site-is-offered-in-every-language-1.md) states what a copy owes. Every page has a copy in every declared language, and no copy is written by hand. In every copy each link lands on a page the build wrote and stays in that copy's language. The switch and the alternates lead to every copy and back. At every width the design declares, the top bar of a copy has the height and the rows of the English page's bar. That last clause has a history: the first Czech copy was published with its top bar wrapped onto a second line, because the Czech labels were longer and a row of buttons had been translated as one sentence, and nothing measured the copy against its original.

The `site-languages` gate now does. It loads every page of every copy in Chromium, answered from the build itself with no network, and holds each to the rule, naming the page and what it measured. A translation that would break the bar is fixed in the memory, never by excluding the page or the width. Case 3590 proves the pass that writes the copies, including the refused translations and a second pass that changes nothing.

## Where an article is a second source

An amendment of 21 October 2026 records the one exception the gates now enforce. Check 21 of `scripts/site-check` requires every article the navigation lists to read in every language the site declares, the members of a translation group naming each other. A translation names the article it was made from and the digest of that article's body, and check 21 recomputes the digest and refuses a translation its source has moved past, without ever rewriting it. So the decision's first point now reads that English is the only source, except an article's declared translation, which check 21 holds to its source's digest.

Such a page is carried into its language's copy unchanged. The machine copy of the English article names the hand-written translation as its canonical page, so a reader and a search engine both meet the authored text. The alternative, dropping the article counterparts from the check and deleting those pages, would have kept the original wording and removed the one place where a translation is proved to keep up with its source. This article is one of them: its Czech counterpart names this page and the digest of what you are reading.

## What it does not do yet

When the decision was written, search, the sitemap and the scripts' own strings were outside the pages the pass reads. Measured on 22 October 2026, the sitemap carries every copy with its alternates, and case 3590 holds it to them. The search index lists every copy, but a machine copy's entry keeps the English title and description; only an authored translation's entry is in its language. The scripts of the Czech homepage carry no English phrase, and that is the one page measured. The translation is machine translation and says so on every translated page, with a link to the original. Coverage is measured on every build, and a gate that refuses a regression in it is the decision's next step, not yet a gate. Every change to English prose costs a translation of the segments it changed, and only of those.

## How to check it

`/cs/i18n.json` on the live site reports the coverage of the memory, and any page under `/cs/` shows its notice and links back to the English. `bash test/run.sh 3590_the_site_is_derived_in_every_language` runs the case that holds the pass, `node scripts/lib/site-i18n-probe.mjs` runs the gate over a build, and `scripts/site-check --articles` holds the article translations to their sources.

ADR 0218 is accepted, and its amendment is part of it. How one declaration reaches every surface is in [Written once, derived everywhere](@/written-once-derived-everywhere.md), how a repository refuses its own documentation in [Proof-carrying documentation](@/proof-carrying-documentation.md), and how a question is put to the advisors that also translate in [A doubt is settled by an experiment](@/a-doubt-is-settled-by-an-experiment.md).
