---
id: project.english-only
version: 2
kind: rule
title: English only
description: The repository's own artifacts are written in English; published content addressed to a reader may be written in that reader's language, declared and confined to the published site.
statement: Code, comments, commits, documents and governance records are written in English, with no exceptions. Published content addressed to a reader may be written in that reader's language, only under the authored source of the published site, only when the page declares its own language, and only when it is declared with that language and a reason.
status: active
class: blocking
depends_on: []
tags: [language]

x-majordomus:
  tests: [scripts/ci/english-only-check]
---

# Rationale

The repository is read by people and tools that share one language; a second language in a comment or a commit message is a second rulebook for whoever cannot read it. That reason is about **working artifacts** — everything a contributor or an agent must read in order to change this repository — and for those the rule admits no exceptions.

It says nothing about the published site. A page written for a reader in that reader's language is the product speaking to its audience, not a rulebook a contributor has to decode. Making the site's reach depend on the repository's internal working language is the wrong axis, so version 2 names the two populations apart instead of forcing one answer on both (ADR 0077).

# Required behaviour

Code, comments, commits, documents and governance records are written in English, with no exceptions.

Published content addressed to a reader may be written in that reader's language when all three hold:

- it lives under `site/content-src/`, the authored source of the published site, or is the generated projection of such a page under `site/content/`;
- the page declares its own language in its front matter, as `[extra] lang = "<language>"`;
- it is declared in `scripts/ci/english-only-allow.txt` as `translation <path> <language> <why>`, and the declared language is the one the page declares.

No other part of the repository acquires this permission, and none of the three conditions is waivable by review.

# Failure behaviour

`scripts/ci/english-only-check` fails the change. A translation declared for a path outside the published site, a declaration without a reason, and a declaration whose language disagrees with the page's own are each a refusal rather than a warning — the last one because an exemption that only one side agrees with is how a list entry becomes a laundering mechanism.

# Verification

`scripts/ci/english-only-check` decides the part of this rule an alphabet can decide: a
letter that occurs in Czech, Slovak or Polish and never in English, or a whole script —
Cyrillic, Greek, CJK — in any authored file. It is deliberately partial and says so: English
prose carrying a sentence of another language spelled in ASCII passes it, and always will.
What it buys is that the common case here cannot arrive unnoticed. Proper nouns, files that
use a foreign string as test data, and published translations are declared, each with its
reason, in `scripts/ci/english-only-allow.txt`; the rest is review.

`test/cases/403_published_translation.sh` proves the three conditions: that an undeclared page in another language still fails, that a declaration outside the published site is refused, and that a declaration whose language the page does not carry is refused.
