+++
title = "English only"
description = "English only"
weight = 103
[extra]
kind = "rule"
slug = "project-english-only-4"
identity = "project.english-only@4"
status = "active"
source = ".ai/repo/rules/project/english-only.v4.md"
+++
{% raw %}

## Rationale

The repository is read by people and tools that share one language; a second language in a comment or a commit message is a second rulebook for whoever cannot read it. That reason is about **working artifacts** — everything a contributor or an agent must read in order to change this repository — and for those the rule admits no exceptions.

It says nothing about the published site. A page written for a reader in that reader's language is the product speaking to its audience, not a rulebook a contributor has to decode. Making the site's reach depend on the repository's internal working language is the wrong axis, so version 2 names the two populations apart instead of forcing one answer on both (ADR 0077).

Version 3 adds one place, not one population. The handbook is the product explained to a reader, written in that reader's language like a page of the site, and it is too large and too interactive to be a page of the site: an edition is its texts, its widgets and its tests, in one directory. That directory is content addressed to a reader, held to the same two-sided declaration as a translated page, and everything that builds it — the generator, the derivations, the gate — stays English like any other working artifact (ADR 0149).

Version 4 adds a third place, and it is the one that makes the other two rare. The owner ordered on 2026-10-19 that the published site be always fully multilingual — Czech and English at least — derived and translated automatically, with nobody keeping two language versions. Every page of the site is now written once, in English, and its copy in each other language is derived from the English build and a translation memory (ADR 0218). The memory is content addressed to a reader exactly as a translated page is — every entry is an English segment of a published page and its translation — so it is admitted under the same two-sided declaration, as an edition: one directory per language under `site/i18n/`, which says its language in its own `edition.toml`. Everything that builds it — the pass, the translator's carrier, the build — stays English.

## Required behaviour

Code, comments, commits, documents and governance records are written in English, with no exceptions.

Published content addressed to a reader may be written in that reader's language when all three hold:

- it lives under `site/content-src/`, the authored source of the published site, or is the generated projection of such a page under `site/content/`;
- the page declares its own language in its front matter, as `[extra] lang = "<language>"`;
- it is declared in `scripts/ci/english-only-allow.txt` as `translation <path> <language> <why>`, and the declared language is the one the page declares.

An edition of the handbook may be written in its reader's language when all three hold:

- it is one directory directly under `handbook/`, named for its language;
- the directory declares that language in its own `edition.toml`, as `lang = "<language>"`;
- it is declared in `scripts/ci/english-only-allow.txt` as `edition <directory> <language> <why>`, and the declared language is the one the directory declares.

The permission covers the files of that directory and nothing beside it: `handbook/build.py` and the derivations next to it are working artifacts and are written in English.

The site's translation memory of one language may be written in that language when all three hold:

- it is one directory directly under `site/i18n/`, named for its language;
- the directory declares that language in its own `edition.toml`, as `lang = "<language>"`;
- it is declared in `scripts/ci/english-only-allow.txt` as `edition <directory> <language> <why>`, and the declared language is the one the directory declares.

No other part of the repository acquires this permission, and none of the conditions is waivable by review.

## Failure behaviour

`scripts/ci/english-only-check` fails the change. A translation declared for a path outside the published site, an edition declared for a path that is not a directory of the handbook, a declaration without a reason, and a declaration whose language disagrees with the page's or the edition's own are each a refusal rather than a warning — the last one because an exemption that only one side agrees with is how a list entry becomes a laundering mechanism.

## Verification

`scripts/ci/english-only-check` decides the part of this rule an alphabet can decide: a
letter that occurs in Czech, Slovak or Polish and never in English, or a whole script —
Cyrillic, Greek, CJK — in any authored file. It is deliberately partial and says so: English
prose carrying a sentence of another language spelled in ASCII passes it, and always will.
What it buys is that the common case here cannot arrive unnoticed. Proper nouns, files that
use a foreign string as test data, and published translations are declared, each with its
reason, in `scripts/ci/english-only-allow.txt`; the rest is review.

`test/cases/403_published_translation.sh` proves the three conditions: that an undeclared page in another language still fails, that a declaration outside the published site is refused, and that a declaration whose language the page does not carry is refused. `test/cases/2984_a_handbook_edition_is_built_from_the_tree.sh` proves the same three for an edition.
{% endraw %}
