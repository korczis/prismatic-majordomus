# A product feature is discovered, never registered

## What it means

One Markdown file under `.ai/repo/features/` with schema-valid front matter is the whole act
of adding a feature to this product. From that moment `majordomus product list` shows it,
`GET /api/v1/product/features` returns it, the OpenAPI document describes its type,
`majordomus://feature/<id>` serves it over MCP, the `product` graph carries its nodes and
edges, the capability matrix gains its row, the website gives it `/features/<id>/`, and — if
the file declares itself featured — the homepage carries its chapter.

Nothing else changes. There is no registry to edit, no navigation file, no template list, no
site data file, no Rust and no schema.

## How it works

The kind `feature` is declared in `share/kinds.yaml` with its JSON Schema beside every other
kind's, and the source class `feature` in the repository's `sources.yaml` discovers every
file under the section through the version-control index. Discovery, front-matter parsing,
schema validation, identity, the index and the MCP resource are the mechanism every kind of
the layer already goes through; the product model adds no discovery of its own.

`apps/majordomus-cli/src/product.rs` resolves each file's references against the index, the
capability registry, the command registry, the Why catalogue and the web topology once, when
a capability context is composed, and every projection reads that one value.

## How to see it

```bash
majordomus product list                         # every feature, from the files alone
majordomus product show worktrees               # one feature, with every relation derived
curl -s localhost:8741/api/v1/product/features | jq '.features | length'
bash test/run.sh 97_product_features            # add one file, find it everywhere; remove it, find it gone
```

`test/cases/97_product_features.sh` adds exactly one file — asserting that the staged change
is that one path and no other — then finds it in the index, the command line, the JSON
answer, the matrix, MCP and the site projection. It then removes the file and finds it gone
from all of them, which is what a hidden registry anywhere would fail.

## What it does not cover

A feature that does not validate is not served anywhere: the site generator refuses to render
a product model with an error in it rather than publishing a broken page. And nothing here
decides whether a feature is worth writing, or which features form one story for a visitor —
that grouping is the editorial decision the model asks a person for.

## Why it exists

The homepage was the last surface of this repository that stated what the product does by
hand. Every other surface had already become a projection of one declaration per capability
and one file per object of the layer, so a capability added to the executable appeared on the
API reference and in the Cockpit the day it existed, and on the homepage the day somebody
remembered. Nothing could say whether a sentence on the public page was still true.

The cost of the old shape was paid on every addition, and worse, on every change: a module
that gained an MCP tool did not change a card that said "command line and HTTP". The case is
what holds the new shape — a single added file has to reach every projection, and a single
removed file has to leave all of them.
