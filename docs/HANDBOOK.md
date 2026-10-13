# The handbook

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

| Part | Where it comes from |
|---|---|
| the prose of a chapter | written by hand, in the edition's language, under `handbook/<language>/` |
| the reading room: every document, decision, rule, claim and use case, rendered in place | the files git tracks, read by `handbook/corpus.py` |
| links between documents, and what links back | computed from those files |
| the map of kinds, the flows, and what surrounds one object | `docs/generated/graph.json`, compacted by `handbook/graph.py` |
| the explorer, the gates, the repository in numbers | the registry, `.ai/repo/ci/gates.yaml` and the history, read by `handbook/derive_data.py` |
| "what this chapter rests on", under each chapter | the chapter's declared anchors and what its text links to, resolved against the graph |
| every number in a caption | a placeholder the build fills |

Nothing in the second column is typed into a chapter. The built page is not committed.

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
[`project.the-handbook-is-built-from-the-tree`](../.ai/repo/rules/project/the-handbook-is-built-from-the-tree.v1.md)
and the decision is
[ADR 0149](../.ai/repo/adrs/0149-a-handbook-edition-is-a-declared-locale-tree-built-from-the-tree-it-describes.md).

## An edition and the language rule

This repository's working artifacts are English. An edition is content addressed to a reader,
like a translated page of the site, and
[`project.english-only`](../.ai/repo/rules/project/english-only.v3.md) admits it under three
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

- [`PERSONAL_MODE.md`](PERSONAL_MODE.md), the chapter of the handbook that exists as an English document too.
- [`CI.md`](CI.md) for how a gate is selected.
- [`DOCTRINE.md`](DOCTRINE.md) for what a rule is and what proves one.
