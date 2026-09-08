---
schema: adr/v1
id: adr-0023
kind: adr
title: Product features are objects of the layer, and the landing page is a projection of them
status: accepted
date: 2026-09-07
tags:
  - product
  - site
  - projections
  - architecture
related:
  - rule:project.product-surface-derived
  - rule:project.interfaces-are-projections
  - rule:project.why-catalogue-is-canonical
  - rule:project.no-counts-in-prose
  - file:apps/majordomus-cli/src/product.rs
  - file:apps/majordomus-cli/src/capability/builtin/product.rs
  - file:.ai/repo/features/README.md
  - file:docs/PRODUCT.md
  - file:site/templates/index.html
  - test:test/cases/97_product_features.sh
provenance:
  origin: authored
---

# 23. Product features are objects of the layer, and the landing page is a projection of them

## Context

The website's homepage was the one surface of this repository that still stated what the
product does by hand. Its sections were written into a template and a marketing file:
which capabilities to mention, which interfaces they were available on, what to say about
the Cockpit, which commands to show. Every other surface — the command line, the HTTP
routes, the OpenAPI document, MCP, the Cockpit, the generated reference, the registry
pages of the site itself — had already become a projection of one declaration per
capability (ADR 2, ADR 4, ADR 5) and one file per object of the layer (ADR 7, ADR 8,
ADR 18). The homepage was the last inventory kept beside the registry it described.

The cost was the one this repository refuses everywhere else. A capability added to the
executable appeared on the API reference, in the Cockpit and in the registry pages the day
it existed, and on the homepage the day somebody remembered. A module that gained an MCP
tool did not change a card that said "command line and HTTP". A moment added to the why
catalogue was on the homepage only because that one section had already been made a
projection. Nothing could say whether a sentence on the homepage was still true, and the
gap between the product and its public face grew in exactly the direction nobody
measured.

Two things about the homepage cannot be derived, and the tension between them and the
rest is what needed a decision. Which parts of the product form one story for a visitor —
that worktrees and scope claims and peer announcements are one chapter called
coordination — is an editorial grouping no registry holds. And which chapters the homepage
shows, in which order, under which headline, is a presentation decision. Everything else
the page says about a chapter — which interfaces expose it, how many capabilities, tools
and routes stand behind it, which objects of the layer it comprises, which operational
moments it answers, what is guaranteed and what is only advisory — is a fact some registry
already owns.

## Decision

**A product feature is one declarative object of the layer.** The kind `feature` joins
`share/kinds.yaml` with its JSON Schema beside the others; the source class `feature` in
the repository's `sources.yaml` discovers every file under `.ai/repo/features/`; the
manifest names the section. This needed no code for discovery, parsing, validation,
identity, the index or the MCP resource `majordomus://feature/<id>`: it is the mechanism
every kind already goes through.

**A feature file holds references and editorial decisions, and nothing derivable.** The
references name what the feature is made of, each against the registry that owns it:
capability modules of the executable, public commands of the shell tool, object kinds of
the layer, rules, documents, decisions, claims, use cases, areas of the Cockpit, surfaces
of the web topology. The editorial fields are the ones nothing can infer: the headline, the
summary, the order, whether the homepage features it, and the operational areas and
audiences of the why catalogue it serves — the catalogue's own taxonomies, reused rather
than declared a second time. The schema refuses a derived key: there is no `surfaces`, no
`route`, no count and no list of moments in a source file.

**Every fact a surface says about a feature is derived, in one place.**
`apps/majordomus-cli/src/product.rs` resolves every reference against the registry, the
index, the catalogue and the topology, and derives what nobody authored: the interfaces a
feature is exposed through (`cli` when a module has a command-line path or the feature
names a shell command, `api` when a module has an HTTP route, `mcp` when a module has a
tool or a resource or the feature names a kind, `cockpit` when it names an area or a
module, `docs` when it names a document), the tools, routes and paths behind it, the
objects of its kinds counted, the rules with their class and whether the tool enforces
them, the claims with their status, the moments that name any of its mechanisms, and the
features that name it. A reference that resolves to nothing is an error naming the file,
the key and the nearest candidate; a page that linked to nothing is what that refusal
prevents.

**The model is a capability module, projected everywhere.** `product.features`,
`product.feature`, `product.matrix`, `product.providers` and `product.validate` are
declared once with `capability!`, so the command line, the HTTP routes, the OpenAPI
operations, the MCP tools and resources, the Cockpit's pages and the generated reference
are derivations of one declaration, per ADR 2. The matrix of features against interfaces,
and of every module, command and kind against the features that name it, is one of those
answers; a module no feature names is a warning the validation reports and the matrix
shows, never a row that is quietly absent.

**The providers are discovered, not listed.** The set of providers the tool has an adapter
for is the set of templates the distribution ships under `share/providers/`; what this
repository does with each — the bootstraps its policy renders, the client configuration at
its root, the hooks its policy wires — is read from the policy and the tree. What a
vendor's tool reads its project-scoped MCP configuration from is a contract this repository
cannot discover and states once, in the executable, beside the other protocol constants.

**The site is a reader.** `majordomus generate site` writes
`site/data/registry/product.json` and `product-graph.json` from the same capabilities,
through an allow-list of the fields a public page may carry. The homepage renders its
chapters from the featured features in weight order, its matrix, its providers, its
telemetry and its graph from that dataset, and names no feature, module, command, kind,
provider or count of its own; `scripts/site-check` refuses a template, a generator or a
navigation file that does. The one hand-written file, `site/data/marketing.toml`, holds
the positioning sentences and button labels, and is held to a line budget and to carrying
no number and no capability claim.

## Alternatives rejected

**A `features.toml` under the site's data directory, one entry per feature with its
surfaces written in.** The fastest thing to build and the second inventory this repository
refuses for rules, kinds, capabilities and moments. It would be correct on the day it was
written and wrong on the first module that gained a route, and nothing would notice.

**Deriving the chapters entirely, one per capability module.** A module is an
implementation boundary, not a story a visitor reads: worktrees, peer announcements and
scope claims are three modules and one chapter, and the shell tool's commands are no module
at all. A page that showed one card per module would be complete, true and unreadable. The
grouping is editorial, and pretending otherwise would have hidden the decision inside a
derivation.

**Extending `docs/RESPONSIBILITIES.yaml` into the product model.** It is the closest thing
the repository had — a hand-kept cross-reference joined to the README's own table by row
title — and it is the shell tool's alone. Making it carry modules, kinds, decisions and
surfaces would have coupled the product's public face to the prose of one table and left
the executable's half described nowhere.

**Putting the editorial fields on the Rust `module!` declarations.** A feature spans modules,
shell commands and kinds; no single declaration is the place to say what a visitor should
read first about all three, and a marketing headline inside a capability descriptor would
have reached the OpenAPI document and MCP, where it means nothing.

**Reading the surfaces from the feature file as a convenience.** It is the whole defect in
one field: a `surfaces: [cli, api, mcp]` written by a person is a claim, and the registry
already knows the answer.

## Consequences

Adding a feature is adding one file and running `just derive`. It then appears in the
command line, the HTTP API, the OpenAPI document, MCP, the derived graph, the Cockpit, the
`/features/` pages, the matrix and — when it is featured — the homepage, with nothing else
edited, which `test/cases/97_product_features.sh` proves by doing it and undoing it.

Adding a capability to a module a feature already names changes the feature's derived
surfaces, counts and links the next time `generate` runs, and `generate --check` refuses a
tree that did not run it: the homepage cannot describe an executable this repository does
not have. Adding a module, a public command or a kind that no feature names is reported as
a gap by `product validate` and shown on the matrix; assigning it to a feature is one
reference in one file, and that assignment is the editorial act the model asks for and
nothing more.

The cost is a kind triple to keep in step and a stricter build: a typo in a feature's
references now fails `product validate` and the site generation instead of rendering a
slightly wrong card. The message names the file, the key and the correction, which is what
makes that a repair rather than an investigation. The screenshots the Cockpit chapter shows,
when it shows any, remain captured evidence rather than a derivation, and are held only to
naming routes the topology has.
