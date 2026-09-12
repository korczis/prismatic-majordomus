+++
title = "Which interfaces a feature is exposed through, what stands behind it and what it answers are derived from the registries that own those facts, never authored, and a source file that writes one down is refused by its schema"
description = "A feature file holds two things: **references** naming what the feature is made of, and"
weight = 164
[extra]
claim_id = "product-surfaces-derived"
status = "guaranteed"
source = "docs/claims/product-surfaces-derived.md"
+++
{% raw %}

## What it means

A feature file holds two things: **references** naming what the feature is made of, and
**editorial decisions** nothing can infer. Everything a surface states about the feature is
derived from those references — the interfaces it is exposed through, the tools, routes and
command-line paths behind it, how many objects of its kinds the layer holds, the class of
each rule and the status of each claim, the operational moments it answers, the features that
name it, and its own route.

A source file cannot write any of that down. The schema sets `additionalProperties: false`,
so `surfaces: [cli, api, mcp]` or `route: /features/<id>/` in a feature is a validation error
rather than a claim a page believes.

## How it works

`apps/majordomus-cli/src/product.rs` derives the interfaces from the registry rather than
from the file: `cli` when a module the feature names has a command-line path or the feature
names a shell command, `api` when a module has an HTTP route, `mcp` when a module has a tool
or a resource or the feature names a kind, `cockpit` when it names an area or a module with
one, `docs` when it names a document. A feature therefore cannot claim a surface the registry
does not have, and gains one the moment the registry does.

The editorial fields are the ones nothing can infer: the headline, the summary, the order,
whether the homepage features it, and the operational areas and audiences of the Why
catalogue it serves — that catalogue's own taxonomies, reused rather than declared a second
time.

## How to see it

```bash
majordomus product show worktrees --format json | jq '.surfaces, .counts'
grep -c surfaces .ai/repo/features/worktrees.md   # 0: the file declares none
majordomus product validate                       # refuses a derived key with its schema message
cargo test -p majordomus-cli --test product
```

## What it does not cover

Which features form one story for a visitor, and which of those the homepage shows first, are
editorial and are written. Deriving the chapters — one per capability module, say — was
rejected: a module is an implementation boundary, not a story, and the shell tool's commands
are no module at all.

The screenshots a Cockpit chapter shows, when it shows any, remain captured evidence rather
than a derivation, and are held only to naming routes the topology has.

## Why it exists

A `surfaces: [cli, api, mcp]` written by a person is a claim, and the registry already knows
the answer. That one field is the whole defect the model was built to remove: correct on the
day it is written, wrong on the first module that gains a route, and nothing notices.
{% endraw %}
