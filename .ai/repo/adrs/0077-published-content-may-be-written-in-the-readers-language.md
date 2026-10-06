---
schema: adr/v1
id: adr-0077
kind: adr
title: Published content may be written in the reader's language; the repository's own artifacts may not
status: proposed
date: 2026-09-17
tags:
  - language
  - governance
  - site
  - publication
related:
  - rule:project.english-only
  - claim:authored-files-are-english
  - claim:published-translation-is-declared
  - file:.ai/repo/rules/project/english-only.v2.md
  - file:scripts/ci/english-only-check
  - file:scripts/ci/english-only-allow.txt
  - file:docs/claims/authored-files-are-english.md
  - file:docs/claims/published-translation-is-declared.md
  - file:site/templates/base.html
  - file:site/content-src/proof-carrying-documentation.md
  - file:site/content-src/dokumentace-kterou-lze-odmitnout.md
  - test:test/cases/403_published_translation.sh
provenance:
  origin: authored
---

# 77. Published content may be written in the reader's language; the repository's own artifacts may not

## Context

`project.english-only` v1 said, in one sentence and without qualification:

> Code, comments, commits, documents and governance records are written in English, with no
> exceptions.

It is a blocking rule and it is wired to `scripts/ci/english-only-check`, which decides the
part an alphabet can decide: a letter that occurs in Czech, Slovak or Polish and never in
English, or a whole non-Latin script, in any tracked authored file. The site's content tree is
in scope. There is no debt baseline in this tree, so one hit fails the gate.

That rule was written when the statement was exhaustively true of this repository: every
artifact in it was a **working artifact** — something a contributor or an agent reads in order
to change the code. For a working artifact the rule's reason is exactly right, and this
decision does not weaken it. A comment in another language is a second rulebook for whoever
cannot read it, and the repository is worked on by people and agents who think in a language
that is not English, which is precisely why the gate exists.

Since then the repository grew a second kind of artifact that v1 never had to distinguish: the
**published site**. `site/content-src/` holds pages written for a reader who is not working on
this repository at all. A page addressed to a Czech reader, in Czech, is not a second rulebook
for a contributor; it is the product speaking to its audience.

The trigger was concrete. Two long-form articles were written from one evidence base, one in
English and one in Czech — not a translation, two independently composed pieces. The English
one could be published. The Czech one could not, and neither of the two escape hatches beside
the check applied: `name` is for proper nouns, `fixture` is for a file that uses a foreign
string as *data*, and the check's own header states that **neither exempts a sentence**.
Declaring the article as a `fixture` would have been a misuse of an exemption whose
documentation forbids exactly that, and `--write-baseline` would have laundered it into known
debt. The honest options were to change the rule or not to publish.

## Decision

**The repository's own artifacts are written in English, with no exceptions. Published content
addressed to a reader may be written in that reader's language, under three conditions, each
of which is decided by the gate rather than by review.**

1. **Location.** It lives under `site/content-src/`, the authored source of the published
   site, or is the generated projection of such a page under `site/content/`. Nowhere else in
   the repository acquires this permission — not `docs/`, not `.ai/`, not a comment, not a
   commit message.
2. **Declaration.** It is declared in `scripts/ci/english-only-allow.txt` with a third kind,
   `translation <path> <language> <why>`, carrying the language and a required reason, in the
   same shape as the existing two kinds.
3. **Self-declaration, checked both ways.** The page itself declares its language in its front
   matter (`[extra] lang = "<language>"`), and the check refuses a declaration whose language
   does not match the page's own. A one-sided declaration — an allow-list entry nothing on the
   page agrees with — is a finding, not a pass.

The rule moves to version 2 with the distinction written into its statement. The failure
behaviour is unchanged for everything that is not published content.

## Consequences

**The gate stays strict where it was strict.** Everything the v1 check refused, the v2 check
still refuses: a Czech sentence in a `.rs` comment, a Slovak word in a rule, a commit message
in another language, a Czech paragraph in `docs/`. The permission is confined to a tree whose
purpose is publication, and confined further to files that declare themselves.

**The exemption cannot be acquired by being added to a list.** Because the page must declare
its own language and the check compares the two, an entry added to the allow list to silence a
finding fails unless the page really is what the entry says it is. This is the same property
the `fixture` kind gets from its required reason, made decidable rather than reviewable.

**The site must stop asserting one language for every page.** `site/templates/base.html`
hardcoded `<html lang="en">` on every page of the site. A published Czech page under that
template would misreport its language to screen readers and to search engines, which is a real
accessibility defect (WCAG 3.1.1), so `lang` now derives from the page's declared language and
defaults to English. Reciprocal `hreflang` links between a page and its counterpart in the
other language are emitted from the same front matter.

**The rule's population grows by a category rather than by exceptions.** The alternative
considered was to keep v1 and publish only English. That was rejected because it makes the
site's reach a function of an internal working-language rule, which is the wrong axis: the
reason English is mandatory *inside* the repository — that a contributor must be able to read
every artifact that binds them — says nothing about an article addressed to someone who will
never open the repository at all.

**What remains undecidable is unchanged and still stated.** The check is an alphabet test. A
Czech sentence spelled in ASCII passes it, in v2 exactly as in v1, and the rule stays wider
than the gate. What v2 adds is that the *permitted* case is now typed and cross-checked
instead of being impossible.
