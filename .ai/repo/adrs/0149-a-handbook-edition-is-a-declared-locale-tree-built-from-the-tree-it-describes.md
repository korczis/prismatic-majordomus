---
schema: adr/v1
id: adr-0149
kind: adr
title: A handbook edition is a declared locale tree, built from the tree it describes
status: proposed
date: 2026-10-12
tags:
  - handbook
  - language
  - governance
  - derivation
related:
  - rule:project.english-only
  - rule:project.the-handbook-is-built-from-the-tree
  - file:.ai/repo/adrs/0077-published-content-may-be-written-in-the-readers-language.md
  - file:.ai/repo/adrs/0004-canonical-architecture-and-performance-truth.md
  - file:.ai/repo/rules/project/english-only.v3.md
  - file:.ai/repo/rules/project/the-handbook-is-built-from-the-tree.v1.md
  - file:scripts/ci/english-only-check
  - file:scripts/ci/english-only-allow.txt
  - file:handbook/build.py
  - file:handbook/cs/edition.toml
  - file:handbook/cs/chapter_sources.py
  - file:docs/HANDBOOK.md
  - test:test/cases/2984_a_handbook_edition_is_built_from_the_tree.sh
provenance:
  origin: authored
---

# 149. A handbook edition is a declared locale tree, built from the tree it describes

## Context

On 2026-10-12 a handbook of this product existed in Czech: seventy-seven chapters, an
embedded reader of every document, decision, rule, claim and use case, a map and flows
computed from `docs/generated/graph.json`, and a panel under each chapter naming what the
chapter rests on. It lived in a directory outside this repository and was rebuilt from
`origin/master` by a script on one machine. That arrangement had three defects, all the same
defect:

- **It could not be held to the tree.** Its build refused a chapter whose anchor had left the
  graph, and nothing ran that build when the tree changed. A rename here broke the handbook
  silently, and the handbook was found out the next time somebody rebuilt it.
- **Nobody else could build it.** The generator read a path on one disk.
- **It was outside every gate, every review and every record** this repository has.

Moving it in met `project.english-only`. Version 2 of that rule (ADR 0077) admits a reader's
language in exactly one place, a page under `site/content-src/` that declares its own
language. The handbook is the same population, content addressed to a reader, and it is not a
page: an edition is texts, widgets, a build and browser tests.

The owner ordered the move and the change to the rule on 2026-10-12, in these words: "move it
somewhere in the repository to have it inferred, derived, generated, in sync with the repo,
document it, enforce it via .ai, keep in sync; amend the ADRs, the rules, the enforcement; my
clearance, the architectural change is from me."

## Decision

1. **The handbook lives in `handbook/`.** `handbook/build.py` and the three derivations beside
   it (`derive_data.py`, `corpus.py`, `graph.py`) are working artifacts and are English. They
   read the tree they are in and nothing else: what git tracks, `docs/generated/graph.json`,
   the registry, the CI model, the history. Nothing is fetched.
2. **An edition is one directory, `handbook/<language>/`, with an `edition.toml` that declares
   that language.** Its texts are written by hand in that language. Every figure, list, graph
   and panel in it is derived.
3. **`project.english-only` moves to version 3** and admits an edition as a second place for
   the population version 2 named. The declaration is `edition <directory> <language> <why>`
   in `scripts/ci/english-only-allow.txt`, and it is two-sided like a translation: the
   directory is `handbook/<language>`, and its own `edition.toml` says the same language. An
   edition declared anywhere else, or for a directory that says something else, is refused.
4. **Each chapter declares what it rests on** in the edition's `chapter_sources.py`: documents,
   product features, capability modules, commands, decisions, rules, claims. The build resolves
   every reference against the repository's graph and refuses, with exit 10, a reference the
   graph no longer carries, a chapter that cites nothing, a caption asking for a figure the data
   does not have, and derived data whose shape changed under the page.
5. **The build is a gate.** `handbook-check` runs `python3 handbook/build.py --check` on every
   change, so a change that removes or renames what a chapter rests on fails where it is made.
   The built page is an ignored output under `target/handbook/`; it is not committed.

## Alternatives rejected

- **Pages of the site.** That fits version 2 without amending it, and it means rewriting the
  handbook as Zola pages and moving its generator into the site's templates. The site is one
  consumer of an edition, not its source; publishing an edition on the site is a later
  projection of `target/handbook/`, decided with the site's own owners.
- **An English handbook only.** The reader this one was written for reads Czech, and an
  English one would restate `docs/`. A second edition is one more directory under this
  decision, not a different mechanism.
- **Keeping it outside and vendoring the anchors.** A copy of the anchor list in this
  repository would be the two-places-one-fact shape ADR 0004 refuses.
- **Committing the built page.** It is four megabytes of derived output that would conflict on
  every landing.

## Consequences

- A change here that removes a document, a feature, a module, a command, a decision, a rule or
  a claim a chapter names fails `handbook-check`, and the fix is in the same change: amend the
  chapter, or keep the thing.
- The edition's own files are exempt from the alphabet test and from nothing else. Review still
  reads them.
- The site does not yet offer the handbook, and offers no language switch. Both are projections
  of what this decision puts in the tree and are not decided here.
- The browser tests of an edition (`handbook/cs/tests/`) need a browser and are not part of the
  gate; the gate proves the build, and `test/cases/2984` proves the gate by mutation.
- The status is `proposed`. The owner ordered the change; writing `accepted` is the owner's act.
