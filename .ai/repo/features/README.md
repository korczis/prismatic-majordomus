---
schema: context/v1
id: ai.repo.features
kind: context
title: Product features
description: What the product does for a person, one feature per file, each made of things the other registries already own; every surface that presents a feature is derived from these files.
status: active
scope: subtree
providers: ["*"]
audience: [human, agent]
composition: extend
order: 100
tracks: [apps/majordomus-cli/src/product.rs, apps/majordomus-cli/src/capability/builtin/product.rs, site/templates/index.html, site/templates/feature.html, site/templates/features-section.html]
---

# Product features

A **feature** is one thing the product does for a person, stated once, in one Markdown
file here. The homepage, the `/features/` pages, the capability matrix, the Cockpit, the
command line, the HTTP API and MCP present the same feature from the same file, and none
of them keeps a list of features of its own.

```text
<id>.md      one product feature      schema: feature/v1
```

## Facts are derived, decisions are written

The front matter is two things kept apart on purpose.

**References** name what the feature is made of, each against the registry that owns it:
`modules` (capability modules of the Rust executable), `commands` (public commands of the
shell tool), `kinds` (object kinds of the layer), `rules`, `docs`, `adrs`, `claims`,
`use_cases`, `cockpit` (areas of the Cockpit) and `web` (surfaces of the topology). A name
that resolves to nothing fails `majordomus product validate` with the nearest candidate
offered; a page that linked to nothing is what that refusal prevents.

**Everything a surface says about the feature is derived from those references** and is
never written here: which interfaces expose it (command line, HTTP, MCP, the Cockpit, the
documentation), the tools, routes and command-line paths behind it, how many objects of its
kinds the layer holds, which operational moments it answers, what is guaranteed and what is
only advisory, and its route. The schema refuses a derived key such as `surfaces:` or
`route:`.

**Editorial decisions** are the fields nothing can infer, and they are the only ones a
person writes for presentation: `headline` (the promise), `summary`, `status`, `weight`
(the order), `domain` (its one place in the product's top-level model), `areas` and
`audiences` (the why catalogue's own taxonomies, reused rather than declared again).

## Domains

Every feature names exactly one **domain** in `domain:` — one of the few things the
product controls, each a file under `domains/` here (kind `domain`, schema `domain/v1`):

```text
domains/<id>.md   one product domain   schema: domain/v1
```

A domain states its title, a one-sentence `headline` (what the product does about it), a
one-sentence `problem` (the failure it answers) and a `weight` (the order of the homepage
map and of every list of domains). It never lists its features: membership is the
feature's `domain`, and everything a domain shows — members, interfaces, claims, use cases,
rules, moments — is derived. `majordomus product domains` answers it; a stable feature that
names no domain, or one that does not exist, fails `majordomus product validate`
(ADR 0104). The domain is not an area: a feature still names the why catalogue's areas it
serves, which can be several.

## Discovery

Nothing registers a feature. The source class `feature` in `../knowledge/sources.yaml`
discovers every file here through the version-control index, the kind `feature` in
`share/kinds.yaml` says how it is read, and its schema
(`share/schemas/majordomus/feature/feature.v1.schema.json`) decides whether it is valid.
Adding the file and tracking it in git is the whole act.

What follows with no further step: `majordomus product list` shows it; `/api/v1/product/features`
returns it; the MCP tools answer it and `majordomus://feature/<id>` serves it; `/features/<id>/`
exists, and its domain's place on the homepage map lists it; the capability matrix
gains its row; and every module, command and kind it names gains a link back.

## The floors a stable feature is held to

A `stable` feature names at least one mechanism (a module, a command or a kind) and at least
one document; its body carries `## What it does` and `## What it does not do`. A draft is exempt and listed as a draft. `majordomus product
validate` reports every floor a record misses; a module of the executable, a public command
or a kind of the layer that no feature names is reported too, because a thing the product
does that the product page does not mention is the gap this section exists to close.

## Adding a feature

```bash
cp .ai/repo/features/worktrees.md .ai/repo/features/<id>.md
$EDITOR .ai/repo/features/<id>.md       # id, headline, summary, the references, the body
majordomus product validate             # schema, references with the nearest candidate, floors
just derive                             # the site dataset and the pages follow
```
