---
schema: context/v1
id: ai.repo.why
kind: context
title: Why this tool exists
description: The operational moments this tool answers, the audiences that recognise them and the areas they fall under, each one a file that every projection is derived from.
status: active
scope: subtree
providers: ["*"]
audience: [human, agent]
composition: extend
order: 100
tracks: [site/templates/why-section.html, site/templates/why.html]
children:
  require_contract: false
---

# Why this tool exists

The catalogue of operational failure modes Majordomus is a response to. A **moment** is one
such failure mode — something a reader recognises from their own week. An **audience** is
who recognises it. An **area** is the part of operations it falls under. Each is one
Markdown file here, and every list, count, route, filter, backlink and diagnosis anywhere
in this repository is derived from these files.

None of these are model problems. They are operations problems, and operations is what
Majordomus does. A moment that would be fixed by a better model does not belong here.

```text
moments/<id>.md      one operational failure mode      schema: moment/v1
audiences/<id>.md    who recognises it                 schema: audience/v1
areas/<id>.md        the operational area              schema: area/v1
```

The three directories below hold instances of a kind, not sections of the layer, so they
owe no contract of their own (`children.require_contract: false`): the format all three
follow is stated here, once, and a copy of it in each directory is the duplication this
contract exists to prevent.

## The contract

Front matter is the machine side and the body is the human side. The front matter is not
decoration: discovery, validation, relations, filtering, ordering, navigation, search, the
command line, the HTTP API, the OpenAPI document, MCP and the diagnosis are all read from
it, and nothing derives structure from the prose. The contracts are
`share/schemas/moment.schema.json`, `audience.schema.json` and `area.schema.json`; a key
they do not declare is an error, and a `schema:` version the tool does not know is refused
rather than guessed.

The file name is the `id` is the slug is the route. `moments/two-agents-one-bug.md` is
`majordomus://moment/two-agents-one-bug` is `/why/two-agents-one-bug/`; there is no
mapping table anywhere. Four identities are reserved because the section's own routes use
them — `audiences`, `areas`, `graph` and `index` — and a moment claiming one is refused
rather than quietly replacing a page.

## Discovery

Nothing registers a moment. The source classes `moment`, `audience` and `area` in
`../knowledge/sources.yaml` discover every file here through the version-control index, the
kinds in `share/kinds.yaml` say how each is read, and the schema decides whether it is
valid. Adding the file and tracking it in git is the whole act of adding a moment.

What follows without any further step: the object is in the index and served as
`majordomus://moment/<id>`; `majordomus why` lists it; `/api/v1/why` returns it and the
OpenAPI document describes its type; the MCP tools answer it; `/why/<id>/` exists with its
audience, area, filter and search entries; its signals join the questionnaire; the graph
gains its nodes and edges; and every page it names gains a backlink to it.

## Membership and relations are declared once

A moment declares the audiences and areas it belongs to. An audience never lists its
moments and an area never lists its moments — that list is `moments.filter(m => m.audiences
contains this)`, computed where it is shown. The same holds for every reverse direction: a
claim page's "moments this answers", a capability's "moments it addresses", the
related-moment list. **A backlink is never authored.**

References are typed and validated against the thing they name:

| key | resolves against |
|---|---|
| `audiences`, `areas`, `related` | this catalogue |
| `commands` | the public commands of `share/commands.yaml` |
| `capabilities` | the capability registry of the Rust executable |
| `responsibilities` | what `README.md` declares the tool supervises |
| `claims` | `docs/CLAIMS.yaml` |
| `doctrines` | the effective rule set (`majordomus rules list`) |
| `use_cases` | `../use-cases/` |

A name that resolves to nothing fails validation with the nearest candidate offered. Before
this, a typo produced a link to a page nobody would notice was missing.

## Adding a moment

```bash
cp .ai/repo/why/moments/two-agents-one-bug.md .ai/repo/why/moments/<id>.md
$EDITOR .ai/repo/why/moments/<id>.md          # id, hook, summary, audiences, areas, signals, examples
majordomus why validate                        # schema, references, quality floors
```

That is the whole procedure. Then `just derive` regenerates the projections and
`git add` the new file with them.

## Adding an audience or an area

The same: one file under `audiences/` or `areas/`. A new audience appears in the filters,
gets its route, and is offered as a value of every moment's `audiences` — but it holds no
moments until moments name it, and the coverage floor refuses a `stable` audience that no
three stable moments name.

## What is authored and what is derived

| authored here | derived, never written here |
|---|---|
| title, hook, summary, body | the route and the API path |
| audiences, areas, tags, lifecycle | the moments of an audience or area, and their counts |
| signals and examples | backlinks from claims, commands, capabilities and rules |
| severity, frequency, weight, featured | related-by-shared-metadata, and the diagnosis score |
| explicit `related` | the reverse of `related` |

Writing a derived value into the front matter is the defect this directory exists to
prevent. There is no `route:`, no `url:`, no `moment_count:` and no `backlinks:` key, and
the schema refuses them.

## Quality floors

A `stable` moment carries a summary, at least one audience, at least one area, at least one
signal, at least three examples, and a body with the sections the validator names. A
`draft` is exempt from the content floors, is listed as a draft and is counted nowhere.
`majordomus why validate` is the whole check and CI runs it.

## Anti-patterns

```text
DO                                  DON'T
create one file here                add it to a Rust vec, a JS constant or a nav file
name a capability by its id         write out its command syntax or its endpoint URL
let the count be computed           write "34 problems" in prose anywhere
declare membership on the moment    list a moment inside its audience's file
```

Never edit a generated projection: `site/data/registry/why.json`,
`site/data/registry/why-graph.json` and everything under `site/content/why/` are outputs.
Change the file here and regenerate.

## Moments and use cases are different things

A moment is the pain, in the reader's words. A use case (`../use-cases/`) is the workflow
that answers it, with a scenario the tool executes. A capability is the mechanism. They
cross-link by id and never restate each other's text: a moment that explains how a command
works has taken over the command page's job and will go stale first.
