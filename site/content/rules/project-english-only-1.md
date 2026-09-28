+++
title = "English only"
description = "English only"
weight = 79
[extra]
kind = "rule"
slug = "project-english-only-1"
identity = "project.english-only@1"
status = "active"
source = ".ai/repo/rules/project/english-only.v1.md"
+++
{% raw %}

## Rationale

The repository is read by people and tools that share one language; a second language in a comment or a commit message is a second rulebook for whoever cannot read it.

## Required behaviour

Code, comments, commits, documents and governance records are written in English, with no exceptions.

## Failure behaviour

No command decides this rule; a reviewer does, and a change that violates it is not merged. Where a behavioural case covers part of it, that case is named below.

## Verification

`scripts/ci/english-only-check` decides the part of this rule an alphabet can decide: a
letter that occurs in Czech, Slovak or Polish and never in English, or a whole script —
Cyrillic, Greek, CJK — in any authored file. It is deliberately partial and says so: English
prose carrying a sentence of another language spelled in ASCII passes it, and always will.
What it buys is that the common case here cannot arrive unnoticed. Proper nouns and files
that use a foreign string as test data are declared, each with its reason, in
`scripts/ci/english-only-allow.txt`; the rest is review.
{% endraw %}
