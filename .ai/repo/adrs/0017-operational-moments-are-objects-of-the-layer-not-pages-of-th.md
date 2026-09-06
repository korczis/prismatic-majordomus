---
schema: adr/v1
id: adr-0017
kind: adr
title: Operational moments are objects of the layer, not pages of the site
status: accepted
date: 2026-09-06
tags:
  - why
  - catalogue
  - projections
related:
  - rule:project.why-catalogue-is-canonical
  - rule:project.interfaces-are-projections
  - file:apps/majordomus-cli/src/why.rs
  - file:.ai/repo/why/README.md
  - test:test/cases/98_why_catalogue.sh
provenance:
  origin: authored
---

# 17. Operational moments are objects of the layer, not pages of the site

## Context

The `/why/` section stated the ten operational failure modes the tool answers. Each was a
hand-written page under `site/content-src/why/`, with TOML front matter naming the
commands, responsibilities and claims it rested on, and a block of `awk`, `sed` and `jq`
inside the site generator that parsed that front matter into `site/data/generated/why.json`
for the templates.

It was better than prose — the links were checked, and a page naming a claim that did not
exist failed the build. It was still the wrong shape, for four reasons.

The parser was a second reading of a document format the repository already knows how to
read. The executable indexes and validates every other declarative object of the layer
against a JSON Schema; these ten were parsed by a shell function that understood a subset
of TOML and nothing about schemas.

The catalogue was invisible outside the site. `majordomus` could not list a moment, the
HTTP API did not serve one, MCP did not expose one, and no graph knew they existed —
although the moments are the most direct statement the repository makes about what the tool
is for.

The taxonomy was implicit. There were no audiences and no operational areas, so a reader
could not ask "what does this look like for me" or "how much of my problem is
coordination", and the section could not answer either without somebody writing a second
list.

And the shape did not scale. The eleventh entry would have cost a page, an entry in the
generator's parser, a template change and a backlink somewhere else — so the tenth was
where it stopped.

## Decision

An operational moment is a declarative object of the layer, discovered and validated by the
mechanism every other kind already uses, and every projection of it is derived.

**Three kinds, declared as data.** `moment`, `audience` and `area` join `share/kinds.yaml`
with their JSON Schemas beside the others, and three source classes in the repository's
`sources.yaml` discover them under `.ai/repo/why/`. This required no Rust: discovery,
front-matter parsing, schema validation, identity, the index and the MCP resource
`majordomus://moment/<id>` all follow from those declarations.

**One domain model.** `apps/majordomus-cli/src/why.rs` projects the validated metadata into
typed records, resolves every reference, derives every relation nobody authored, and
answers queries. It is built once when the capability context is composed and shared;
nothing rebuilds it per request.

**Six capabilities, projected everywhere.** `why.list`, `why.moment`, `why.audiences`,
`why.areas`, `why.diagnose` and `why.validate` are declared once with `capability!`, so the
command line, the HTTP routes, the OpenAPI operations with their schemas and examples, and
the MCP tools and resources are derivations of one declaration, per ADR 0002.

**The site is a reader.** `majordomus generate site` writes
`site/data/registry/why.json` and `why-graph.json` from the same capabilities; the site
generator writes one page per record from that dataset and takes each body from the file
the record names. `site/content-src/why/` is gone.

**Authored and derived are kept apart.** The schemas refuse a derived key: there is no
`route`, no `url`, no count and no backlink in a source file. An audience does not list its
moments, and a moment does not carry the moments that name it.

## Alternatives rejected

**Keep the pages and add the metadata to their front matter.** The site would stay the only
reader, the parser would grow, and the moments would remain invisible to every other
surface. It also keeps the catalogue inside `site/`, which is the projection layer.

**A new subsystem for the catalogue — its own loader, its own validator, its own registry.**
This is what the task appeared to need and what the repository already had a general answer
for. Every one of those pieces exists as a data-driven mechanism; a parallel one would have
been a second way to declare a kind.

**Make the diagnosis a `POST` operation with a list input.** `POST` in this repository means
a capability with an effect on the process (ADR 0002's `Command` kind). A diagnosis reads
and changes nothing, so it is a `Query` bound to `GET`, and its selection is a
comma-separated parameter — one schema that binds identically on the query string, over MCP
and on the command line.

**Reuse the `taxonomy` kind for the areas.** Its identities live in one flat namespace with
the use-case categories, and two catalogues would silently collide on a shared id, which the
index resolves by excluding both. A distinct kind costs one schema and cannot collide.

## Consequences

Adding an operational moment is adding one file. It then appears in the command line, the
HTTP API, the OpenAPI document, MCP, the derived graph, the site index, its audience and
area pages, every filter, the search and the questionnaire, with nothing else edited —
which `test/cases/98_why_catalogue.sh` proves by doing it and then undoing it.

The catalogue is a machine-readable map from a reader's symptom to a mechanism: signal →
moment → area and audience → capability, command, claim, rule and use case. The diagnosis
is a traversal of that map with counting, not a model, and every recommendation carries the
moments that produced it.

Two references gained typed targets on the way: `share/commands.yaml` became an object of
the index so a moment's `commands` resolves against the command registry, and a moment's
responsibilities stopped being authored — every claim already declares the responsibility
it belongs to, so the relation is derived through the claims.

The cost is one more kind triple to keep in step, and a site dataset that is now written by
the executable rather than by the site generator, which puts it in the derivation graph's
first stage. Its payload depends on the catalogue's own sources and on nothing else, so
both `generate` passes of `scripts/derive` produce the same bytes.
