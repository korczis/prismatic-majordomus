+++
title = "What the product does is a catalogue object, and every public surface of it is derived"
description = "What the product does is a catalogue object, and every public surface of it is derived"
weight = 109
[extra]
kind = "rule"
slug = "project-product-surface-derived-1"
identity = "project.product-surface-derived@1"
status = "active"
source = ".ai/repo/rules/project/product-surface-derived.v1.md"
+++
{% raw %}

## Rationale

The homepage was the last surface of this repository that stated what the product does by
hand. Everything else — the command line, the HTTP routes, the OpenAPI document, MCP, the
Cockpit, the generated reference, the registry pages of the site — had already become a
projection of one declaration per capability and one file per object of the layer. The
homepage was an inventory kept beside the registry it described.

The cost is the one this repository refuses everywhere else. A capability added to the
executable reached every derived surface the day it existed, and the homepage the day
somebody remembered. A module that gained an MCP tool did not change a card that said
"command line and HTTP". Nothing could say whether a sentence on the public page was still
true, and the gap grew in exactly the direction nobody measured.

Two things about a product page genuinely cannot be derived: which parts of the product
form one story for a visitor, and which of those stories the page shows first. Those are
editorial, they are written, and they are kept apart from everything else — because a
`surfaces: [cli, api, mcp]` written by a person is a claim, and the registry already knows
the answer.

## Required behaviour

A product feature is one Markdown file under the layer's features section, with front
matter declaring a supported `schema:` version. Adding a valid, tracked file is the whole
act of adding a feature: nothing may require a second edit in a Rust list, a template, a
navigation file, a site data file, a generated index or a schema document.

The front matter holds two things and nothing else. **References** name what the feature
is made of, each against the registry that owns it — capability modules of the executable,
public commands of the shell tool, object kinds of the layer, rules, documents, decisions,
claims, use cases, Cockpit areas, surfaces of the web topology, and the operational areas
and audiences of the why catalogue. **Editorial decisions** are the fields nothing can
infer: the headline, the summary, the order, and whether the page features it.

Every fact a surface states about a feature is derived from those references, in one place
in the executable: the interfaces it is exposed through, the tools, routes and
command-line paths behind it, the objects of its kinds counted, the class of each rule and
the status of each claim, the moments that name its mechanisms, the features that name it,
and its route. A derived key written into a source file is a violation of this rule even
when it is correct on the day it is written, and the schema refuses one.

The set of providers the tool adapts to is the set of adapters the distribution ships, and
what a repository does with each is read from its policy and its tree. Neither is a list
kept in a page, a template or a site data file.

No template, generator, navigation file or hand-written site data may name a feature, a
capability module, a public command, a provider or a count of any of them. The one
hand-written file on the public surface holds positioning sentences and button labels, is
held to a line budget, and carries no number and no capability claim.

A capability module, a public command or an object kind that no stable feature names is
reported as a gap and shown, never quietly omitted: a thing the product does that the
product page does not mention is what this rule exists to make visible.

## Failure behaviour

`majordomus product validate` exits `10` and names every finding with its file, its
front-matter key and, for an unresolved reference, the nearest candidate. `majordomus
generate --check` refuses a tree whose derived product dataset does not match the
repository. The site generator refuses a dataset that does not validate, that carries a
feature body, or that carries a machine path. `scripts/site-check` refuses a template, a
generator or a navigation file that names a feature, a module, a command, a provider or a
count by hand, and refuses a feature with no page or a page with no feature.

## Verification

`majordomus product validate`, `scripts/derive-check`, `scripts/site-check`, and
`bash test/run.sh 97_product_features`, which adds one feature file, finds it answered by
the command line, the HTTP API, MCP, the site dataset and the derived graph with nothing
else edited, then removes it and finds it gone from all of them.
{% endraw %}
