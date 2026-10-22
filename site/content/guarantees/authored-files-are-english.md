+++
title = "Every authored file in this repository is spelled in English, and a proper noun or a test fixture that is not carries its reason beside the check"
description = "project.english-only says every artifact of this repository is written in English:"
weight = 172
[extra]
claim_id = "authored-files-are-english"
status = "guaranteed"
source = "docs/claims/authored-files-are-english.md"
+++
{% raw %}

## What it means

`project.english-only` says every artifact of this repository is written in English:
documents, code, comments, commit messages, the rules themselves. That mattered more than it
sounds. This repository is worked on in a language that is not English; several of the
people and agents who touch it think in that language; and a paragraph written in it reads
as perfectly fine to whoever wrote it and as noise to everyone else. Review catches a
document. It does not catch a sentence.

## How it works

Detecting a language from prose is a research problem, and a gate that guessed would be a
gate nobody trusts. What is decidable is the alphabet. A handful of letters carry diacritics
that occur in Czech, Slovak and Polish and never in English — `č ď ě ň ř š ť ů ž`, `ą ć ę ł
ń ś ź ż` — and whole scripts are unambiguous on sight: Cyrillic, Greek, the CJK ranges. A
file containing one of those is not English, whatever else is true of it.

**This is a partial check and it says so.** English prose with a Czech sentence spelled in
ASCII passes it, and always will. The rule stays wider than the gate, which is the honest
relation between the two; what the gate buys is that the common case — the one that has
actually happened here — cannot arrive unnoticed.

Typographic punctuation is allowed by range: this repository's prose uses the em dash, the
curly apostrophe and the ellipsis, and they never reach the test. Two other things are
allowed, and both are declared with a reason in `scripts/ci/english-only-allow.txt` rather
than by weakening the test:

- **`name <word>`** — a proper noun whose spelling is its own. Stripped wherever it appears,
  so a line of English naming one is English.
- **`fixture <path> <why>`** — a file that uses a non-English string as data, to prove the
  program handles one. A doctest asserting that an identifier's local part is opaque bytes
  has to contain opaque bytes; that is the test doing its job. The reason is required, and a
  fixture entry without one makes the check refuse to run rather than silently pass.

Neither exempts a sentence. A comment written in another language inside an exempted file is
still a violation of the rule; what the fixture exemption says is that this alphabet test
cannot tell prose from data in that file — a limit stated rather than hidden.

## How to see it

```
scripts/ci/english-only-check            report, and fail on new debt
scripts/ci/english-only-check --strict   fail on any finding, baseline ignored
```

The `english-only` gate runs it on every CI plan. It landed with an empty baseline.

## What it does not cover

A sentence of another language spelled in ASCII. "Toto je veta bez diakritiky" passes and
always will. The check decides an alphabet, not a language, and the rule is deliberately
wider than the gate — stating that plainly is better than a check that implies a guarantee
it cannot give.

It also does not read generated trees, session records or the checkout-local layer. A
projection carrying a foreign word carries it from its source, and the source is where the
finding belongs; a session record quotes what was said, and what was said is not always
English.

## Why it exists

Because review was the only thing catching it, and review catches a document rather than a
sentence. This repository is worked on in a language that is not English, by people and
agents who think in it, and a paragraph written in that language reads as perfectly fine to
whoever wrote it and as noise to everyone else.

## What proves it

The check itself, run against this repository in CI on every change, and
`test/cases/138_governance_gates.sh`, which proves it by mutation rather than by assertion
that it exists: a Czech letter in an authored file is named with its file and line, a whole
other script is too, a declared proper noun is stripped, a declared fixture is exempt only
with its reason, and the generated trees are out of scope because a projection carrying a
foreign word carries it from its source.

The gate prints its own denominator — how many files it measured and how many fixtures are
declared — rather than that count being written here. A number in prose is a number that
goes stale, and this one did: widening the selector to shebang scripts moved it the same
afternoon it was first written down.
{% endraw %}
