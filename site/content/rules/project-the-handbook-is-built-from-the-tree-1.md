+++
title = "The handbook is built from the tree it describes, and a chapter names what it rests on"
description = "The handbook is built from the tree it describes, and a chapter names what it rests on"
weight = 160
[extra]
kind = "rule"
slug = "project-the-handbook-is-built-from-the-tree-1"
identity = "project.the-handbook-is-built-from-the-tree@1"
status = "active"
source = ".ai/repo/rules/project/the-handbook-is-built-from-the-tree.v1.md"
+++
{% raw %}

## Rationale

A handbook explains the product in a reader's words, and a reader's words go stale without
anybody deciding they should. Until 2026-10-12 this one was rebuilt on one machine from a
script that read `origin/master`; it had a check that refused a chapter whose subject had left
the repository, and nothing ran that check when the repository changed. A working mechanism
with no starter is indistinguishable, from outside, from a wired one (ADR 0149).

The remedy is the one this repository applies to everything it publishes: the thing is built
from the tree, the build refuses what is no longer true, and the build runs on every change.

## Required behaviour

- `handbook/build.py` reads the work tree it is in: the files git tracks,
  `docs/generated/graph.json`, the registry, the CI model and the history. It fetches nothing
  and names no path outside the tree.
- An edition is `handbook/<language>/` with an `edition.toml` declaring that language
  (`project.english-only` v3).
- Each chapter of an edition is listed in the edition's `chapter_sources.py` with what it rests
  on, as `kind:label` references into the graph. A chapter that is about the handbook itself is
  named in the edition's own short list of exceptions, in the build.
- Every number a caption states is a placeholder the build fills from the derived data.
- The built page lands under `target/handbook/` and is not tracked.

## Failure behaviour

`python3 handbook/build.py --check` exits 10 and names what came apart: the chapter and the
reference the graph no longer carries, the chapter that cites nothing, the placeholder with no
figure, or the data file whose keys changed. The gate `handbook-check` fails the change.

## Verification

`test/cases/2984_a_handbook_edition_is_built_from_the_tree.sh` builds the edition from this
tree, then proves each refusal by mutation in a copy: an anchor that does not exist, a chapter
with its anchors removed, an unknown placeholder. It also proves the three conditions
`project.english-only` v3 puts on an edition's declaration.
{% endraw %}
