# Every directory of the layer carries a context document, the exemption is declared by the contract that governs the subtree, and a descendant may narrow that requirement but never weaken it

## What it means

A directory that says nothing about itself is not neutral. It contributes nothing to the
chain a worker resolves, it names no format for the files it holds, and whoever lands there
reads its ancestors and guesses the rest. Coverage turns the habit of writing a `README.md`
into an invariant: every directory of the layer's tree carries a context document, and a
directory that does not is the finding `missing-contract`, naming the directory and — where
an ancestor made the requirement explicit — the contract that did.

The tree is the one the resolver already reads: the manifest's directory, minus `local/`,
minus the vendored rule package, whose integrity is its own manifest's business rather than
a reader's.

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

Exempt a subtree when the directories below it are instances of a kind rather than sections
of the layer: a skill is `SKILL.md` and its examples, and a contract in every instance
directory would repeat, once per instance, the format the section states once.

## How it is proved

`test/cases/69_context_documents.sh` builds a subtree of its own and mutates it: a directory
with no document fails, adding one passes, a directory below it fails in turn, the governing
contract's exemption reaches the whole subtree, a descendant narrows it back to `true`, a
descendant that tries to lower an inherited `true` is refused by name, and the key on a
`directory`-scoped document, or with a value that is not a boolean, is refused as
`invalid-front-matter`.

`majordomus context validate` reports it for the whole tree; `doctor` dispatches the same
check through the rule `majordomus.context-integrity`, which the pre-commit hook runs.

## What it does not claim

Nothing here judges whether a contract is any good. The tool checks that a document exists,
parses, and is not weakened by a descendant; whether its prose actually tells a worker what
belongs in the directory is a reviewer's call, and the decision that introduced coverage
(`.ai/repo/adrs/0011-every-directory-in-the-layer-carries-a-contract.md`) says so in as many
words.
