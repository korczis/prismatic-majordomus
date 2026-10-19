+++
title = "The handbook"
description = "the handbook: one interactive page per language, built from this tree; what is written and what is derived, and the gate that refuses a change which removes what a chapter rests on"
weight = 33
[extra]
source = "docs/HANDBOOK.md"
+++

{% raw %}

The handbook explains Majordomus to a reader in that reader's language: what it is, how a
session enters it, what refuses a change, where memory and rules live, how machines
coordinate, and what the repository's own numbers are. It is one interactive page per
language, and it is built from this repository.

The first edition is Czech, `handbook/cs/`. A second language is a second directory.

## Build it

<!-- majordomus:unrun writes several megabytes under target/handbook/ and reads the whole history; the gate handbook-check runs it on every change -->
```sh
python3 handbook/build.py
```

The page lands in `target/handbook/cs/prirucka-majordomusu.html`. Open it in a browser; it
needs no server. The build takes about seventeen seconds and reads only this tree.

## What is written and what is derived

<div class="overflow-x-auto" tabindex="0">

| Part | Where it comes from |
|---|---|
| the prose of a chapter | written by hand, in the edition's language, under `handbook/<language>/` |
| the reading room: every document, decision, rule, claim and use case, rendered in place | the files git tracks, read by `handbook/corpus.py` |
| links between documents, and what links back | computed from those files |
| the map of kinds, the flows, and what surrounds one object | `docs/generated/graph.json`, compacted by `handbook/graph.py` |
| the explorer, the gates, the repository in numbers | the registry, `.ai/repo/ci/gates.yaml` and the history, read by `handbook/derive_data.py` |
| "what this chapter rests on", under each chapter | the chapter's declared anchors and what its text links to, resolved against the graph |
| every number in a caption | a placeholder the build fills |

</div>


Nothing in the second column is typed into a chapter. The built page is not committed.

An edition chooses no colour either. Its stylesheet names the roles `share/design/tokens.yaml`
declares, the build inlines the projection that gives those roles their values in both
themes, and the charts read the categorical series from the page at run time; `design-check`
holds an edition to that like every other surface.

## What keeps it in step with the repository

Each chapter declares what it rests on in `handbook/<language>/chapter_sources.py`: documents,
product features, capability modules, commands, decisions, rules and claims, as references
into the repository's graph. The build refuses, with exit 10:

- a reference the graph no longer carries, because the thing was removed or renamed;
- a chapter that cites nothing of the repository;
- a caption that asks for a figure the derived data does not have;
- derived data whose keys changed under the page that reads them (`handbook/shape/`).

The gate `handbook-check` runs that build on every change, so the change that renames a
document a chapter is about fails where it is made. The fix belongs to the same change: amend
the chapter, or keep the name. The rule is
[`project.the-handbook-is-built-from-the-tree`](https://github.com/korczis/prismatic-majordomus/blob/@source-ref@/.ai/repo/rules/project/the-handbook-is-built-from-the-tree.v1.md)
and the decision is
[ADR 0149](https://github.com/korczis/prismatic-majordomus/blob/@source-ref@/.ai/repo/adrs/0149-a-handbook-edition-is-a-declared-locale-tree-built-from-the-tree-it-describes.md).

## The outlook chapters

The part "Výhled" carries the owner's strategic report in the repository: a forecast, a
comparison with the market, the report measured against what the repository records, and what
the token economics can and cannot claim. Each statement in it wears one of three tags:

<div class="overflow-x-auto" tabindex="0">

| Tag | What it is | Where it comes from |
|---|---|---|
| repository | a fact of this tree | `outlook-data.json`, derived by `handbook/derive_data.py` from the history, `site/data/generated/plan.json`, `.ai/repo/project/intents/`, the committed evidence ledger and `docs/generated/economics.json` |
| model | an estimate of the report, not a measured figure | `handbook/cs/outlook-model.json`, written once and read by both the captions and the charts |
| external | a claim of a public source | the source, linked beside the claim |

</div>


A repository figure reaches a caption only as a placeholder the build fills, so a figure the
data no longer has fails the build like any other. The economics chapter follows
[ADR 0082](https://github.com/korczis/prismatic-majordomus/blob/@source-ref@/.ai/repo/adrs/0082-token-economics-are-measured-from-matched-runs-and-published-only-past-a-declared-rule.md):
the context reduction is counted, a total-token saving is unproven until matched runs pass the
publication rule, and the usage real sessions observe is not recorded in the tree until the
milestone `execution-telemetry` lands. When the verdict changes, the shape of
`outlook-data.json` changes with it, and the chapter that states the verdict is rewritten in
the same change.

The outlook is published the way the handbook is: the page `python3 handbook/build.py` writes
is published by hand as a Claude Artifact. Its browser test is `handbook/cs/tests/pw-outlook.js`.

## An edition and the language rule

This repository's working artifacts are English. An edition is content addressed to a reader,
like a translated page of the site, and
[`project.english-only`](https://github.com/korczis/prismatic-majordomus/blob/@source-ref@/.ai/repo/rules/project/english-only.v3.md) admits it under three
conditions: it is `handbook/<language>/`; its `edition.toml` declares that language; and
`scripts/ci/english-only-allow.txt` declares it as `edition <directory> <language> <why>`. The
generator beside the editions is not inside one and stays English.

To add an edition, copy an existing one, translate its texts, change `lang` in its
`edition.toml`, rename the directory to that language, and declare it. The derivations are
shared.

## Tests

`test/cases/2984_a_handbook_edition_is_built_from_the_tree.sh` builds the edition and proves
each refusal by mutation. The edition's own browser tests are under `handbook/cs/tests/`; they
need Playwright and a browser, read the built page, and are run by hand:

<!-- majordomus:unrun needs a browser and the built page under target/handbook/; it is the edition's own browser test and is run by hand -->
```sh
node handbook/cs/tests/pw-evidence.js
```

## What is not here yet

The published site does not offer the handbook and has no language switch. Both would be
projections of what this directory builds. The handbook is also published, by hand, as an
artifact outside this repository; that copy is built by the same command.

## Related

- [`PERSONAL_MODE.md`](@/docs/personal-mode.md), the chapter of the handbook that exists as an English document too.
- [`CI.md`](@/docs/ci.md) for how a gate is selected.
- [`DOCTRINE.md`](@/docs/doctrine.md) for what a rule is and what proves one.
{% endraw %}
