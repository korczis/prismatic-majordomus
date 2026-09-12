+++
title = "Every directory of the layer carries a context document, the exemption is declared by the contract that governs the subtree, and a descendant may narrow that requirement but never weaken it"
description = "A directory that says nothing about itself is not neutral. It contributes nothing to the"
weight = 75
[extra]
claim_id = "context-coverage"
status = "guaranteed"
source = "docs/claims/context-coverage.md"
+++
{% raw %}

## What it means

A directory that says nothing about itself is not neutral. It contributes nothing to the
chain a worker resolves, it names no format for the files it holds, and whoever lands there
reads its ancestors and guesses the rest. Coverage turns the habit of writing a `README.md`
into an invariant: every directory of the layer's tree carries a context document, and a
directory that does not is the finding `missing-contract`, naming the directory and — where
an ancestor made the requirement explicit — the contract that did.

The tree is the one the resolver already reads: the manifest's directory, minus the half
the manifest itself declares untracked (`local.tracked: false`). Nothing else is skipped by
name — a subtree that owes no contract says so in the contract above it.

## How it works

The exemption is data on the contract that governs a subtree, never a list at the root, so
it moves with the tree it describes:

```yaml
scope: subtree
children:
  require_contract: false
```

The value that applies to a directory is the one from the nearest ancestor contract that
declares it; where nothing declares it, a document is owed. The field composes by narrowing
only: a descendant may raise `false` to `true` for its own subtree, and lowering an
inherited `true` is `illegal-override` — the same class a descendant earns for superseding
a `final` document. It states what descendants owe, so it is accepted on a `subtree`
document alone, and its value is `true` or `false` and nothing else.

Exempt a subtree with `require_contract: false` when the directories below it are instances
of a kind rather than sections of the layer: a skill is `SKILL.md` and its examples, and a
contract in every instance directory would repeat, once per instance, the format the section
states once.

A subtree the layer carries but does not author is released by name instead:

```yaml
scope: subtree
children:
  require_contract: true
  exempt: [.ai/repo/rules/vendor]
```

Everything at or below a named directory owes nothing. An entry must lie inside the scope of
the document that writes it and must not be that document's own directory: a contract
releases the directories it governs and no others, so the narrowing rule cannot be escaped by
exempting a subtree from the side. The vendored rule package is exempt this way — it is
installed rather than written here, and its integrity is its own manifest's business — while
every directory this repository does write still owes a contract.

## How to see it

```bash
majordomus context validate                 # the whole tree; missing-contract names the directory
majordomus context explain .ai/repo/rules   # the chain that applies, and why each document is in
majordomus doctor                           # the same check through majordomus.context-integrity
```

`test/cases/69_context_documents.sh` proves it by mutation: a directory with no document
fails, adding one passes, a directory below it fails in turn, the governing contract's
exemption reaches the whole subtree, a descendant narrows it back to `true`, a descendant
that tries to lower an inherited `true` is refused by name, and the key on a
`directory`-scoped document, or with a value that is not a boolean, is
`invalid-front-matter`. The use case
`document-every-directory-of-the-layer` runs the refusal end to end.

## What it does not cover

Nothing here judges whether a contract is any good. The tool checks that a document exists,
parses, and is not weakened by a descendant; whether its prose actually tells a worker what
belongs in the directory is a reviewer's call. Coverage also stops at the layer: `docs/` and
the source tree are described by the documents that `tracks` them, not by a contract in
every directory.

## Why it exists

Eighteen directories under `.ai/` had drifted into silence while the tree that held them
validated cleanly, because the tool only ever checked the documents that existed. An
obligation nobody enforces is a habit, and a habit is what a repository loses first.
`.ai/repo/adrs/0011-every-directory-in-the-layer-carries-a-contract.md` records the
decision and what it rejected.
{% endraw %}
