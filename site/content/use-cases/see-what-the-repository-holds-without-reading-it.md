+++
title = "See what the repository holds, and run one of its capabilities, without reading a file"
description = "Open the shared server in a browser and get the whole layer as pages: what exists, where each thing came from, what depends on what, what is failing, and a form that runs any capability."
weight = 7
[extra]
id = "see-what-the-repository-holds-without-reading-it"
source = ".ai/repo/use-cases/see-what-the-repository-holds-without-reading-it.md"
category = "mcp"
maturity = "described"
+++

## Situation

Somebody needs to know what this repository actually holds. Not the code — the governance:
which rules are in force, what a decision put in force, which capabilities exist and where
each came from, what is generated and what was written, what is stale, what is failing.

Today the answers are spread across a JSON document, a Swagger form, three commands with
different exit codes, and six hundred files. A reviewer reads files. A newcomer reads
files. An operator who wants to try one capability writes a curl by hand from a schema.

The temptation is to build a dashboard: a second application that fetches the API and lays
it out, with its own list of pages, its own idea of what a capability is, and its own
health checks. That is a second model of the same facts, and it drifts on the first edit
that forgets it.

## What you run

- `knowledge sources`: the source classes the repository declares — the same classes that
  become the object kinds the Cockpit's navigation offers, so what it can show is what the
  repository says exists
- `doctor`: proves the layer and its projections before anything serves them; the Cockpit's
  own health page is the executable's side of the same question, and it names the engine
  behind every verdict rather than reaching one of its own

The server itself is started by the AI client's autostart or by `majordomus serve`, and it
logs the URL. Everything below is on that one port: no second process, no second command.

## Scenario

```yaml
setup: installed-wired
given:
  - 'a repository with the layer installed, its projections generated and the client autostart wired'
steps:
  - id: what-the-server-will-serve
    run: ['knowledge', 'sources']
    note: 'the source classes the Cockpit lists as object kinds are the ones the repository declares'
    expect:
      exit: 0
      stdout_contains: ['^policy +shared +policy', '^knowledge sources: [0-9]+ file']
  - id: sound-before-it-is-served
    run: ['doctor']
    note: 'the Cockpit renders what the layer holds; a layer that does not pass here is what its health page would report'
    expect:
      exit: 0
      stdout_contains: ['doctor: 0 failure']
then:
  - 'the shared server the client autostart binds serves the Cockpit at /cockpit beside Swagger UI at /swagger'
  - 'every page is rendered from a capability, so a capability added to the registry has a page with no edit to the Cockpit'
  - 'the health page reports one check per dimension, each naming the engine that decided it and the command that reproduces it'
```

## Outcome

The Cockpit is a projection of the capability registry, not an application over it. Every
page is laid out from what a capability answered through the same executor MCP and the HTTP
routes call, so a page is counted in the same performance counters, answered from the same
cache and bound by the same validation as any other caller.

What that buys, concretely: the navigation's catalogues are the registry's modules, the
index's kinds and the graph derivations; the capability explorer is the registry filtered;
the form that runs a capability is generated from its input schema and calls its real
route; the examples on its page are its own benchmark cases. Adding a capability adds its
page, its navigation entry, its search entry, its palette entry, a working form and a
benchmark target, with nothing edited in the Cockpit.

The graphs answer the questions a list cannot: what a rule depends on, what a decision put
in force, which module feeds which interface, what shape `.ai/` is. They are derived in the
executable from the registry and the index, served as ordinary capabilities, and drawn by a
library that is a consumer of the same nodes and edges — so a reader without JavaScript
gets every node and every edge as a table, and loses nothing.
