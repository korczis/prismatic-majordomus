+++
title = "Refuse a directory of the layer that documents nothing"
description = "A directory added under the layer without a context document fails validation by name, nothing resolves against the tree until it says what it is for, and the same hierarchy is readable over MCP and the API without running the gate."
weight = 21
[extra]
id = "document-every-directory-of-the-layer"
source = ".ai/repo/use-cases/document-every-directory-of-the-layer.md"
category = "knowledge"
maturity = "described"
+++

## Situation

Somebody adds a directory under `.ai/` — a new section, a place for data, a subdivision of an existing one — and writes no `README.md`. Nothing breaks that day. The directory contributes nothing to the chain a worker resolves, it names no format for the files it holds, and the next person to land there reads its ancestors and guesses the rest. Left alone, that is how a layer with a contract per directory becomes a layer with a contract per directory somebody remembered.

## What you run

- `context validate`: the whole tree, with `missing-contract` naming every directory that owes a document
- `context resolve <path>`: refuses while the tree is broken, rather than answering from the part that happens to be fine
- `doctor`: the same check through `majordomus.context-integrity`, which the pre-commit hook runs

## Scenario

```yaml
setup: undocumented-directory
given:
  - 'a repository with Majordomus installed, a new section that carries its contract, and one directory below it that carries none'
steps:
  - id: validate
    run: ['context', 'validate']
    note: 'the directory with no document is named, and the one that has its contract is not'
    expect:
      exit: 10
      stdout_contains: ['missing-contract', 'zones/deep']
      stdout_not_contains: ['zones — missing-contract']
  - id: resolve
    run: ['context', 'resolve', '.ai/repo/zones']
    note: 'nothing resolves against a tree that does not validate, not even the part that is correct'
    expect:
      exit: 10
      stdout_contains: ['does not validate']
  - id: doctor
    run: ['doctor']
    note: 'the same finding through the rule majordomus.context-integrity, which the pre-commit hook runs'
    expect:
      exit: 10
      stdout_contains: ['missing-contract']
then:
  - 'a directory of the layer with no context document is a named failure, not a silence'
  - 'the finding names the directory, so the fix is one file in one place'
  - 'the tree refuses to resolve until it is fixed, so nobody reads a half-documented layer'
  - 'the same tree, and what each directory owes, is readable through majordomus_directories and GET /api/v1/directories without running the gate'
```

## Outcome

The obligation to document a directory is enforced rather than remembered. The fix is one file: a `README.md` carrying `schema: context/v1`, saying what the directory is for and what belongs in it. Where the directories below a contract are instances of a kind rather than sections — a skill and its examples — the governing contract exempts them with `children.require_contract: false`, and the exemption travels with the tree it describes.
