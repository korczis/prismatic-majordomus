+++
title = "Product"
description = "the product features as objects of the layer: what a feature file may hold, what every surface derives from it, the public projection boundary and its allow-list, what the homepage and the `/features/` pages render, and what is refused"
weight = 36
[extra]
source = "docs/PRODUCT.md"
+++

{% raw %}

The product's public face — the homepage, the `/features/` pages, the capability matrix —
states what Majordomus does. Every fact on it is derived from the repository it describes:
which capabilities exist, which interfaces expose them, how many objects of each kind the
layer holds, which operational moments a feature answers, what is guaranteed and what is
only advisory. What a person writes is which parts of the product form one chapter, and in
what order. Behaviour as implemented and tested; where this document and the executable
disagree, the document is wrong and changes in the same commit. The decision is
[ADR 23](../.ai/repo/adrs/0023-product-features-are-objects-of-the-layer-and-the-landing-page-is-a-projection.md);
the rule is `project.product-surface-derived`; the directory's own contract is
[`.ai/repo/features/README.md`](../.ai/repo/features/README.md).

## The kind

```text
.ai/repo/features/<id>.md    one product feature    schema: feature/v1
```

The file name is the `id` is the slug is the route: `features/worktrees.md` is
`majordomus product show worktrees`, `majordomus://feature/worktrees`,
`GET /api/v1/product/feature?id=worktrees` and `/features/worktrees/`. Nothing registers a
feature. The source class `feature` in `.ai/repo/knowledge/sources.yaml` discovers every
file under that directory through the version-control index, the kind `feature` in
`share/kinds.yaml` says how it is read, and
`share/schemas/majordomus/feature/feature.v1.schema.json` decides whether it is valid.

## What is authored and what is derived

A feature file holds **references** and **editorial decisions**, and nothing else. The
schema sets `additionalProperties: false`, so a derived key such as `surfaces:`, `route:`
or a count is refused at validation rather than silently believed.

<div class="overflow-x-auto" tabindex="0">

| In the file | Derived from it |
|---|---|
| `modules` — capability modules of the executable | the capabilities, MCP tools, HTTP routes and command-line paths behind the feature |
| `commands` — public commands of the shell tool | the command-line surface, and the stage and read-only flag of each |
| `kinds` — object kinds of the layer | how many objects of each kind this repository holds |
| `rules`, `claims` | the class of each rule, whether the tool enforces it, the status of each claim |
| `docs`, `adrs`, `use_cases` | titles, statuses and the routes the site gives them |
| `cockpit`, `web` | the Cockpit areas and the mounts of the web topology |
| `areas`, `audiences` — the why catalogue's own taxonomies | the operational moments that name any of the feature's mechanisms |
| `headline`, `summary`, `short_title`, `status`, `weight`, `featured`, `tags`, `related` | nothing: these are the editorial decisions |

</div>


The interfaces a feature is exposed through are derived and never declared: `cli` when a
module it names has a command-line path or it names a shell command, `api` when a module
has an HTTP route, `mcp` when a module has a tool or a resource or the feature names a
kind, `cockpit` when it names an area or a module with one, `docs` when it names a
document. A feature therefore cannot claim a surface the registry does not have.

## One model, every interface

`apps/majordomus-cli/src/product.rs` resolves every reference against the index, the
capability registry, the command registry, the why catalogue and the web topology, and is
the only place any of this is decided.
`apps/majordomus-cli/src/capability/builtin/product.rs` declares five capabilities over it
with `capability!`, so the command line, the HTTP routes, the OpenAPI operations, the MCP
tools and resources and the generated reference are projections of one declaration
([ADR 2](../.ai/repo/adrs/0002-canonical-capability-registry.md), `docs/CAPABILITIES.md`).

<div class="overflow-x-auto" tabindex="0">

| Answer | Command line | HTTP | MCP |
|---|---|---|---|
| every feature | `majordomus product list` | `GET /api/v1/product/features` | `majordomus_features`, `majordomus://product` |
| one feature | `majordomus product show <id>` | `GET /api/v1/product/feature` | `majordomus_feature`, `majordomus://feature/<id>` |
| the matrix | `majordomus product matrix` | `GET /api/v1/product/matrix` | `majordomus_product_matrix` |
| the providers | `majordomus product providers` | `GET /api/v1/product/providers` | `majordomus_providers` |
| the refusals | `majordomus product validate` | `GET /api/v1/product/validate` | `majordomus_product_validate` |

</div>


## The public projection boundary

`majordomus generate site` writes `site/data/registry/product.json` and
`product-graph.json`. The dataset is not the model serialised: `PUBLIC_FEATURE_FIELDS` in
`apps/majordomus-cli/src/site.rs` is an allow-list, and each field is copied by name. A
field added to the model does not reach a published page until somebody adds it to that
list, which is the point — accidental exclusion is not a boundary, and a field nobody
allowed is not published.

What that keeps out is asserted, not assumed: the feature's Markdown body, absolute paths,
anything outside the repository. The test is `apps/majordomus-cli/tests/product.rs`, which
builds the dataset over a fixture and refuses a key nobody allow-listed, a body, a fixture
root path and a temporary directory; `scripts/generate-site-data` refuses the artifact
again before it renders anything from it.

## What the site does with it

`scripts/generate-site-data` validates the dataset — the schema is
`majordomus-site-product/v1`, published as
`share/schemas/generated/site-product.schema.json` — and writes one page per non-draft
feature under `site/content/features/`, with the feature's own Markdown as the body. The
templates read the dataset and name nothing:

<div class="overflow-x-auto" tabindex="0">

| Route | Template | What it renders |
|---|---|---|
| `/` | `site/templates/index.html` | the chapters (features that declare themselves featured, in weight order), the interfaces, the matrix, the graph, the providers, the kinds and the doctrine |
| `/features/` | `features-section.html` | every non-draft feature, the interfaces, the graph, the providers |
| `/features/<id>/` | `feature.html` | the feature's own prose, then everything derived from its references |
| `/features/matrix/` | `features-matrix.html` | features against interfaces, and modules, commands and kinds against the features that name them |

</div>


`site/data/marketing.toml` is the one hand-written file: positioning sentences and button
labels, held to a line budget, carrying no number and no capability claim. Routes that
moved are declared once in `site/data/nav.toml` under `[[redirects]]` and become Zola
aliases on the page they point at.

## What is refused

- A reference that resolves to nothing. The message names the file, the key and the
  nearest candidate, which makes it a repair rather than an investigation.
- A derived key in a feature file — `surfaces`, `route`, a count, a list of moments.
- A `featured` feature that is not `stable`; a `stable` feature that names no mechanism, or
  no document, or whose body lacks `## What it does` and `## What it does not do`.
- A stale dataset: `majordomus generate --check` fails a tree whose `product.json` does not
  match the repository, so the homepage cannot describe an executable this repository does
  not have.
- A template, a generator or a navigation file that names a feature, a module, a command,
  a provider or a count by hand. `scripts/site-check` decides that.

A capability module, a public command or an object kind that no stable feature names is
**not** refused. It is reported as a gap by `majordomus product validate` and shown on the
matrix, because a thing the product does that the product page does not mention is exactly
what this model exists to make visible. Closing it is one reference in one file.

## Adding a feature

```bash
cp .ai/repo/features/worktrees.md .ai/repo/features/<id>.md
$EDITOR .ai/repo/features/<id>.md       # id, headline, summary, the references, the body
git add .ai/repo/features/<id>.md       # discovery reads the version-control index
majordomus product validate             # schema, references, floors
just derive                             # the dataset, the pages and the generated docs follow
```

`test/cases/97_product_features.sh` does exactly that and undoes it, proving the feature is
answered by the command line, HTTP, MCP, the site dataset and the graph while it exists,
and by none of them once it is gone.
{% endraw %}
