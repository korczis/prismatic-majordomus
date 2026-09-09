+++
title = "An operational moment is one file under the layer, discovered rather than registered, and answered by the command line, the HTTP API, the OpenAPI document, MCP, the derived graph and the website with nothing else changed"
description = "One Markdown file under .ai/repo/why/moments/ with schema-valid front matter is the whole"
weight = 143
[extra]
claim_id = "why-catalogue-discovered"
status = "guaranteed"
source = "docs/claims/why-catalogue-discovered.md"
+++
{% raw %}

## What it means

One Markdown file under `.ai/repo/why/moments/` with schema-valid front matter is the whole
act of adding an operational failure mode to this repository. From that moment `majordomus
why` lists it, `GET /api/v1/why` returns it, the OpenAPI document describes its type,
`majordomus://moment/<id>` serves it over MCP, the `why` graph carries its nodes and edges,
and the website gives it `/why/<id>/` with its audience page, its area page, every filter it
belongs to and its signals in the questionnaire.

Nothing else changes. There is no registry to edit, no navigation file, no template list, no
Rust and no schema.

## How it works

The three kinds — `moment`, `audience` and `area` — are declared in `share/kinds.yaml` with
their JSON Schemas beside every other kind's, and three source classes in the repository's
`sources.yaml` discover them through the version-control index. Discovery, front-matter
parsing, schema validation and the MCP resource are the mechanism every kind of the layer
already goes through; the catalogue adds no discovery of its own.

`apps/majordomus-cli/src/why.rs` projects the validated metadata into typed records once,
when a capability context is composed, and every projection reads that one value.

## How to see it

```bash
majordomus why list                             # every moment, from the files alone
majordomus why show two-agents-one-bug          # one moment, with every relation derived
curl -s localhost:8741/api/v1/why | jq '.moments | length'
bash test/run.sh 98_why_catalogue               # add one file, find it everywhere; remove it, find it gone
```

`test/cases/98_why_catalogue.sh` adds exactly one file — asserting that the staged change is
that one path and no other — then finds it in the domain index, the command line, the JSON
answer, MCP and the site projection. It then removes the file and finds it gone from all of
them, which is what a hidden registry anywhere would fail.

## What it does not cover

A moment that does not validate is not served anywhere, which is the point: the site
generator refuses to render a catalogue with an error in it rather than publishing a broken
page. And nothing here decides whether a moment is worth writing.

## Why it exists

The catalogue began as prose under `site/content-src/why/`, where a moment existed for the
website and for nothing else: the command line could not list one, no schema said what a
moment was, and a page that named a command nobody had written was a link that went nowhere
until a reader found it. Making the moment an object of the layer put it behind the same
discovery, schema and index every other kind already had, and left the website as one more
reader of it rather than its home.

The cost of the old shape was paid on every addition: a moment meant a page, an entry in
whatever list the navigation used, and a hope that the things it named still existed. The
case is what holds the new shape — a single added file has to reach every projection, and a
single removed file has to leave all of them.
{% endraw %}
