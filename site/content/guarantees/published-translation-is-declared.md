+++
title = "A page of the published site written in a reader's language is declared with that language and a reason, and is refused unless the page declares the same language itself"
description = "project.english-only binds this repository to one working language, and version 2 names one"
weight = 175
[extra]
claim_id = "published-translation-is-declared"
status = "guaranteed"
source = "docs/claims/published-translation-is-declared.md"
+++
{% raw %}

## What it means

`project.english-only` binds this repository to one working language, and version 2 names one
population apart: a page of the published site written for a reader in that reader's language
(ADR 0077). Everything else is unchanged — a comment, a commit message, a rule, a document
under `docs/` must be English, with no exceptions.

The permission is not "the site tree is exempt". It is per page, and it costs three
declarations that must agree with each other.

## How it works

`scripts/ci/english-only-check` allows such a page only when all three hold:

- **It is published content.** The path is under `site/content-src/`, the authored source of
  the site, or is that page's generated projection under `site/content/`. A translation entry
  naming anything else makes the check refuse to run.
- **It is declared with a reason.** `translation <path> <language> <why>` in
  `scripts/ci/english-only-allow.txt`, in the same shape as the two older kinds. An entry with
  no reason is a list entry, not a declaration, and is refused.
- **The page says the same thing.** The page's own front matter carries
  `lang = "<language>"`, and the check compares the two. A page that declares nothing, or
  declares a different language than the list claims for it, is refused.

The third condition is the one that makes this an exemption rather than a loophole. An allow
list is a file anyone can append to in order to silence a finding; requiring the file itself to
say what it is means an entry that is not true of its page cannot pass.

## How to see it

```console
$ scripts/ci/english-only-check --strict
english-only-check: every authored file is spelled in English (N measured, N declared fixtures, N declared translations)
```

`bash test/run.sh 403_published_translation` proves each refusal by mutation: an undeclared
page in another language still fails, a declaration without a reason is refused, a declaration
outside the published site is refused, a declaration the page does not agree with is refused,
and a declared translation exempts its own file and not its neighbour.

## What it does not cover

The check is an alphabet test, in version 2 exactly as in version 1. It decides letters that
occur in Czech, Slovak or Polish and never in English, and whole non-Latin scripts. English
prose carrying a sentence of another language spelled in ASCII passes it, and always will, so
the rule stays wider than the gate.

It also does not decide whether a translation is *good*, whether it says the same thing as its
counterpart, or whether the two contradict each other. Nothing in this repository decides that
today; it is review.

## Why it matters

The alternative to a typed exemption is one of two worse things: a repository that cannot
publish a word to a reader who does not work in English, or a baseline file that launders
foreign prose into "known debt" and stops meaning anything. Naming the permitted case, and
making the page and the list check each other, keeps the gate strict everywhere it was strict
before — which is what makes it still worth obeying.
{% endraw %}
