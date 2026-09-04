# The context that applies to a path is composed from the layer's scoped documents, root to target, in one deterministic order with provenance, and a tree that does not validate resolves nothing

## What it means

The AI layer is hierarchical. A directory under `.ai/` may carry a context document: a
Markdown file whose front matter declares `schema: context/v1` and `kind: context`, an
`id` that is its identity whatever the file is called, where it applies (`scope`:
its directory, its subtree, or the directories it lists), which providers and audience it
addresses, and how it composes with what applies above it (`extend`, `replace` naming
what it supersedes, or `final`, which no descendant may supersede). The effective context
for a path is every applicable document from the tree root down to the target directory,
least specific first, and each entry says why it is there.

## How it works

`lib/context_docs.sh` walks the tree once, in C-collation order, skipping the local half
and the vendored rule package, and admits a file by its contract, never by its name;
names the manifest lists under `context.documents` must carry the contract wherever they
appear. Each document is checked against the allowed keys, the required keys, the value
vocabularies and the providers the policy projects; then across documents for a unique
id, references that resolve within the ancestor chain, no superseded `final` document
and no cycle. Resolution orders the applicable documents by depth, then `order`, then
path, drops what a `replace` below supersedes, filters by `--provider` and `--audience`,
and prints a fingerprint of the ordered ids, paths and content hashes. A path outside the
tree receives the root chain plus every document whose `tracks` pathspecs cover it.

## How to see it

```bash
majordomus context list                                  # every document, in resolution order
majordomus context explain .ai/repo/rules/project        # the chain, why each applies, what was left out
majordomus context resolve lib/rules.sh --provider claude-code --json
majordomus context validate                              # exit 10 with every problem named, or 0
```

## What it does not cover

The tool composes an ordered list of documents; it never merges their Markdown, ranks
their prose, or decides that one sentence contradicts another. Whether a document is
right is the reader's judgement; whether it is where it says it is, and applies to what
it says, is the tool's.

## Why it exists

The always-loaded root file is where every rule ends up when nothing says where a rule
belongs, and that file was measured oscillating between nothing and about a thousand
lines. A contract that lets context live beside the thing it describes, and a resolver
that proves the chain from root to target is one deterministic list, is what keeps the
root thin without losing the rest. `test/cases/69_context_documents.sh` breaks the tree
one fact at a time and proves each break is refused by name.
