# The product is filed under domains, and the domain lists nothing

## What it means

A domain is one Markdown file under `.ai/repo/features/domains/`: a title, a one-sentence
promise, the failure it answers and an order. A feature names exactly one domain in its own
`domain` field. Everything a surface says about a domain — which features it holds, the
interfaces they reach, the guarantees and use cases behind them, the failure modes they
answer, and its route — is derived from the features that name it.

The model refuses what would make a map built on domains lie by omission: a stable feature
that names no domain while any is declared, a domain that does not exist, and a stable
feature filed under a domain that is not shown.

## How it works

The kind `domain` is declared in `share/kinds.yaml` with its JSON Schema, and the source
class `domain` discovers its files through the version-control index. `crate::product`
files each feature under the domain it names and derives every domain's view from its stable
members; `product.domains` projects that to the command line, HTTP and MCP, and the site's
`product.json` carries it through an allow-list (ADR 0104).

## How to see it

```bash
majordomus product domains                      # every domain, with its derived members
majordomus product list --domain <id>           # the features filed under one
majordomus product validate                     # refuses an unfiled or misfiled feature
```
