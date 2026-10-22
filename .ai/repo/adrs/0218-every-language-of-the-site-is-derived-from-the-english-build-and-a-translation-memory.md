---
schema: adr/v1
id: adr-0218
kind: adr
title: Every language of the site is derived from the English build and a translation memory
status: accepted
date: 2026-10-19
tags:
  - site
  - language
  - derivation
related:
  - rule:project.english-only
  - file:.ai/repo/adrs/0077-published-content-may-be-written-in-the-readers-language.md
  - file:.ai/repo/adrs/0149-a-handbook-edition-is-a-declared-locale-tree-built-from-the-tree-it-describes.md
  - file:.ai/repo/rules/project/english-only.v4.md
  - file:scripts/lib/site-i18n.mjs
  - file:scripts/site-translate
  - file:scripts/site-build
  - file:site/i18n/languages.json
  - file:site/i18n/cs/edition.toml
  - file:site/templates/partials/language-switch.html
  - test:test/cases/3590_the_site_is_derived_in_every_language.sh
provenance:
  origin: authored
---

# 218. Every language of the site is derived from the English build and a translation memory

## Context

On 2026-10-19 the owner ordered that majordomus.dev be, from then on, always fully
multilingual — Czech and English at the least — with a language switch, and that every
language be derived, inferred and translated automatically: "I do not want to maintain two
language versions; it must work automatically, derived, inferred, the prismatic way."

The site then had 2,137 pages and about 880,000 words, nearly all of them generated from
English records (the plan, the documents, the rules, the decisions, the registry), and four
hand-written Czech pages admitted by `project.english-only` v2 (ADR 0077). Three ways to make
it bilingual were on the table:

- **A second content tree** (`*.cs.md` beside every page, Zola's multilingual mode). Every
  generator would write twice, every template would need its strings keyed, and the data the
  templates read (`site/data/generated/*.json`) would still render in English. It is the
  "two language versions" the owner refused, written by machines instead of people.
- **Translation at build time**, by asking a model during `scripts/site-build`. A build would
  stop being a pure function of the tree: two builds of one commit would differ, CI would need
  a credential, and a model outage would take the site down.
- **A pass over the built pages and a memory of translated segments** — chosen.

## Decision

1. **English is the only source.** Nobody writes a page in another language. The four
   hand-written Czech pages stay as they are, and are carried into the Czech copy unchanged.

2. **Every other language is a pass over the built English site.** After Zola, the
   accessibility pass and the diagrams, `scripts/site-build` runs `scripts/lib/site-i18n.mjs`
   over every page it wrote. A page is cut into segments: each run of inline content between
   block boundaries, and each attribute a reader meets (title, alt, aria-label, the page's
   description). Code, `pre`, `kbd`, SVG and anything marked `translate="no"` are never a
   segment. The copy under `/<lang>/` is the English page with each segment the memory knows
   replaced, every internal link pointed at its counterpart, `<html lang>` and the canonical
   URL set, `hreflang` alternates added, and the machine-translation notice revealed. Because
   the pass reads the rendered HTML, it covers templates, generated data and content alike:
   there is nothing a template must do to be translated.

3. **The memory is keyed by the English, not by the page.** `site/i18n/<lang>/memory/*.jsonl`
   holds `{k, en, <lang>}` per segment, `k` the hash of the English with inline markup replaced
   by numbered placeholders (`<t1>…</t1>`, `<t2/>` for code and images). The same words in two
   templates are one entry; an href that differs from page to page does not split them. A
   changed English segment has a new key, so a stale translation is never served — the segment
   falls back to English until it is translated again.

4. **A translation is admitted only with the structure of its source.** `admissible` requires
   every placeholder exactly once, nested as in the English; `scripts/site-translate import`
   and `run` refuse anything else. A translator can reorder words, never drop a link or code.

5. **Translation is a separate, automatic step, outside the build.** The build lists what the
   memory lacks in `target/site-i18n/missing.json` with the number of pages each segment is on
   and publishes its coverage at `/<lang>/i18n.json`. `scripts/site-translate run --adapter`
   asks a model through the advisor adapters of ADR 0098, and `export`/`import` carry batches
   to translators outside the process (a session's subagents). The glossary and style a
   translator is given live in `site/i18n/<lang>/guide.json`.

6. **The memory is content addressed to a reader**, so `project.english-only` v4 admits
   `site/i18n/<lang>/` as an edition: the directory says its language in `edition.toml`, and
   `scripts/ci/english-only-allow.txt` declares it with that language and a reason.

## Consequences

- A new language is one line of `site/i18n/languages.json`, a guide, an `edition.toml` and a
  translation run. No template, generator or page changes.
- Coverage is measured, not asserted. It is reported on every build; the gate that refuses a
  regression, then anything below complete coverage, is the next step of this decision.
- Every change to English prose costs a translation of the segments it changed. The cost is
  proportional to the change, because unchanged segments keep their keys.
- The translation is machine translation and says so on every translated page, with a link to
  the original.
- Search, the sitemap and the JavaScript strings of the site are not yet translated; they are
  outside the pages this pass reads.

### Amendment, 2026-10-21: an article's declared translation is a second source, held to the first

Point 1 said nobody writes a page in another language, and kept the four hand-written Czech
pages as its one exception. Since then the site holds articles to a stricter rule than that
exception allowed for. `scripts/site-check` check 21 (I3436, I3437, I3438) requires every article
the navigation lists to read in every language the site declares, with the members of a
translation group naming each other in one `[extra.translations]` table. A translation names the
article it was made from (`translated_from`) and the digest of that article's body
(`source_digest`). Check 21 recomputes the digest and refuses a translation its source has moved
past, without ever rewriting it. Point 1 and that gate disagreed. Train 3 carries six such pages
(I3075, I3077, I3078, I3080–I3082) and four more exist; the gate is the one being enforced.

So point 1 reads, from this date: **English is the only source, except an article's declared
translation, which check 21 holds to its source's digest.** Such a page is carried into its
language's copy unchanged, as the four Czech pages always were. The language's machine copy of
the English article names the authored translation as canonical. Every other page in every
other language is still the pass of point 2 over the English build. Nothing else is written by
hand, and a page that is neither is refused.

The alternative, dropping article counterparts from check 21 and deleting the ten pages, would
have kept point 1's wording. It would also have removed the one place where a translation is
proved to keep up with its source. The owner's order is a site that reads in every language and
stays current, so the gate stays and the record moves to it.

### Amendment, 2026-10-22: what the site's other surfaces carry in each language (I3868)

The consequence that said "Search, the sitemap and the JavaScript strings of the site are not
yet translated" was true when it was written and is not true of all three now. Measured on
majordomus.dev at 76c57859663:
- The sitemap carries every copy. 1,296 of its 2,592 locations are /cs/ routes, and every
  location names its alternates with hreflang. Case 3590 holds this for /cs/guide/.
- The search index (docs/index.json) lists every /cs/ route, 2,632 of its 5,513 pages. A machine
  copy's entry still carries the English page's title and description. Only a declared
  translation's entry reads in its own language.
- The homepage's own scripts and Alpine expressions carry no English phrase on /cs/. Their
  visible strings are markup the pass translates, such as the search field's placeholder. Other
  pages' scripts were not measured.

So the bullet reads, from this date: the search index's titles and descriptions of machine
copies are not yet translated, and the other two surfaces are.
