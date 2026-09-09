+++
title = "A diagnosis of a reader's own symptoms is deterministic counting over the catalogue's metadata, and every recommendation names the moments that produced it"
description = "majordomus why diagnose and the questionnaire on /why/ answer \"what is my operational"
weight = 145
[extra]
claim_id = "why-diagnosis-explainable"
status = "guaranteed"
source = "docs/claims/why-diagnosis-explainable.md"
+++
{% raw %}

## What it means

`majordomus why diagnose` and the questionnaire on `/why/` answer "what is my operational
problem" by counting, and every row of the answer carries the moments that produced it.

```text
selected            signal ids, or moment ids
resolves to         the moments that own them
an area scores      how many of those moments name it
a recommendation    a capability, command, claim, rule or use case those moments name,
                    with matched_because listing them
```

There is no weighting, no percentage and no model. A reader who disagrees with a row can see
exactly which of their own answers produced it.

## How it works

The signals are declared by the moments and by nothing else, so the questionnaire is the
catalogue's own metadata rather than a list of questions kept somewhere. A selected name is
resolved as a signal id first and a moment id second, so a person ticking symptoms and a
script naming moments reach the same answer. Names that resolve to neither are returned as
`unresolved` rather than dropped.

The website runs the same arithmetic over the same data: the page's script is given the
build-time projection of the catalogue and joins signal to moment to metadata; there is no
mapping from a symptom to a recommendation written in JavaScript.

## How to see it

```bash
majordomus why diagnose                                   # the questionnaire, from the moments' own signals
majordomus why diagnose --signal two-agents-one-bug       # a diagnosis, with matched_because on every row
majordomus why diagnose --signal nonsense --format json | jq '.unresolved'
cargo test -p majordomus-cli --test why
bash test/run.sh 98_why_catalogue
```

`apps/majordomus-cli/tests/why.rs` asserts that a signal and its moment reach the same
answer, that an unrecognised name is reported as unresolved rather than dropped, that an
empty selection is an empty diagnosis rather than an error, and that each row carries the
moment that produced it. `test/cases/98_why_catalogue.sh` runs the same from the command
line, including the questionnaire built from the catalogue's own signals.

## What it does not cover

It does not rank by severity or frequency, and it does not decide what a reader should do.
It reports what their answers imply about which parts of operations they are paying for, and
what in this repository addresses those parts.

## Why it exists

A diagnosis that cannot be argued with is advice, and advice from a tool about someone
else's operations is worth very little. The obvious implementations — a weighting per
signal, a score, a model trained on nothing — all produce a number that no reader can check
and no author can defend, and the first time such a number is wrong it is also unfalsifiable.

Counting is defensible: every row names the answers that produced it, so a reader who
disagrees disagrees with something specific. It also keeps the website honest, because a
page that did its own arithmetic would be a second opinion about the same data, and the two
would drift the first time either changed.
{% endraw %}
