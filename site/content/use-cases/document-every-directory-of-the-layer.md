+++
title = "Refuse a directory of the layer that documents nothing"
description = "A directory added under the layer without a context document fails validation by name, nothing resolves against the tree until it says what it is for, and the same hierarchy is readable over MCP and the API without running the gate."
weight = 20
[extra]
id = "document-every-directory-of-the-layer"
source = ".ai/repo/use-cases/document-every-directory-of-the-layer.md"
category = "knowledge"
maturity = "guaranteed"
+++

## Situation

Somebody adds a directory under `.ai/` — a new section, a place for data, a subdivision of an existing one — and writes no `README.md`. Nothing breaks that day. The directory contributes nothing to the chain a worker resolves, it names no format for the files it holds, and the next person to land there reads its ancestors and guesses the rest. Left alone, that is how a layer with a contract per directory becomes a layer with a contract per directory somebody remembered.

## What you run

- `context validate`: the whole tree, with `missing-contract` naming every directory that owes a document
- `context resolve <path>`: refuses while the tree is broken, rather than answering from the part that happens to be fine
- `doctor`: the same check through `majordomus.context-integrity`, which the pre-commit hook runs

## Outcome

The obligation to document a directory is enforced rather than remembered. The fix is one file: a `README.md` carrying `schema: context/v1`, saying what the directory is for and what belongs in it. Where the directories below a contract are instances of a kind rather than sections — a skill and its examples — the governing contract exempts them with `children.require_contract: false`, and the exemption travels with the tree it describes.
