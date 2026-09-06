---
schema: adr/v1
id: adr-0012
kind: adr
title: The Cockpit is a sixth projection of the registry, not an application over it
status: proposed
date: 2026-09-06
tags:
  - architecture
  - capabilities
  - rust
  - interface
related:
  - rule:project.interfaces-are-projections
  - rule:project.rust-canonical-declaration
  - rule:project.rust-benchmark-coverage
  - rule:project.no-network-no-eval
  - file:apps/majordomus-cli/src/cockpit/mod.rs
  - file:apps/majordomus-cli/src/graph.rs
  - file:apps/majordomus-cli/src/capability/builtin/graph.rs
  - file:apps/majordomus-cli/src/capability/builtin/health.rs
  - file:share/cockpit/src/cockpit.css
  - file:docs/COCKPIT.md
  - test:apps/majordomus-cli/tests/cockpit.rs
  - file:scripts/cockpit-probe
provenance:
  origin: authored
---

# 12. The Cockpit is a sixth projection of the registry, not an application over it

## Context

The executable already answers one question five ways. A capability is declared once with
`capability!`, and MCP tools and resources, HTTP routes, the OpenAPI document, the Swagger
UI shell and the command line are derived from that declaration (ADR 2, ADR 4). Every one
of them is machine-facing. A person who wants to know what this repository holds, what
applies where, what is derived from what and what is failing has a JSON document, a
Swagger form and a terminal.

The obvious way to close that gap is the wrong one. A dashboard that fetches
`/api/v1/capabilities` and lays it out in a client-side application is a second model of
the same facts: it needs its own list of pages, its own idea of what a capability is, its
own health checks, its own graph derivation, and its own answer when the two disagree.
Every one of those is a copy, and this repository's first rule about interfaces
(`project.interfaces-are-projections`) exists because copies drift on the first edit that
forgets one.

The second temptation is subtler: a view layer that reaches into the index directly
because it is in the same process. That is faster to write and it is a fourth way of
reading the repository, outside the executor, outside the cache, outside the counters, and
outside the validation every other caller passes through.

Two things were also genuinely missing rather than merely unrendered. The repository's
graphs — rule dependencies, decisions and what they put in force, the shape of the layer —
were derived by shell scripts for the website and by nothing for the executable, so the
process that holds the index could not answer "what depends on this". And "is this
healthy" was answered by three separate commands (`capabilities validate`, `bench coverage
--check`, `generate --check`) and by no single readable thing.

## Decision

- **The Cockpit is a projection.** `apps/majordomus-cli/src/cockpit/` renders
  server-rendered HTML under `/cockpit`, and every page is laid out from what a capability
  answered through `Context::execute` — the same call MCP and the HTTP routes make. A page
  is counted in the same perf counters, answered from the same cache and bound by the same
  validation as every other caller. No page reads the index directly.

- **Its routes are infrastructure, not capabilities.** `/cockpit` joins `/`,
  `/openapi.json`, `/docs` and `/mcp` in `INFRASTRUCTURE_ROUTES`. It declares no schema, no
  operation and no name of its own, and the OpenAPI document lists it as what it is.

- **Nothing in it is a list.** The navigation's catalogues are the registry's modules, the
  index's kinds and the graph derivations. The capability explorer is the registry
  filtered. The runner's form is generated from the input schema, and it posts to the
  capability's own route — there is no runner endpoint and no second validation. The
  examples on a capability's page are its own `BenchmarkCases`. The command palette reads
  `/api/v1/capabilities`, `/api/v1/graphs` and `/api/v1/objects`, and the Cockpit's own
  pages out of the navigation the server already rendered.

- **Graphs are canonical in Rust.** `graph::Graph` is a node and edge model with declared
  node and edge vocabularies, derived from the registry and the index and nothing else:
  `registry`, `layer`, `rules`, `adrs`, `use-cases`. `graph.list` and `graph.get` are
  ordinary capabilities, so the graphs reach MCP, HTTP, OpenAPI and the Cockpit at once. A
  drawing library is a consumer at the boundary; translating to Cytoscape's shape happens
  in one function in one file of JavaScript, and a future projection to Mermaid or Graphviz
  rebuilds no semantics.

- **Health delegates and never decides.** `health.report` reports one check per dimension,
  each carrying the engine that decided it and the command that reproduces it: the index's
  own diagnostics, the registry builder, the declared scope, git, the benchmark
  projection's coverage, and the committed registry manifest compared with what this
  process built. It computes no verdict of its own, and it does not re-render the
  projections on every call — that would rebuild canonical state per request, which this
  executable does not do; `generate --check` remains the complete answer and the check
  names it.

- **The browser layer is an enhancement, in the strict sense.** Every page is complete
  HTML: the graph pages carry every node and every edge as tables, the topology page says
  where the same facts are as text. Alpine adds the palette and the theme; Cytoscape,
  Three.js and p5 add one view each and are loaded only by the page that uses one. If none
  of them loads, navigation, inspection, health and invocation all still work.

- **The policy is strict and the build is CSP-clean.** Every page carries
  `default-src 'none'; script-src 'self' 'sha256-…'` — no `unsafe-inline`, no
  `unsafe-eval` — which is why Alpine's CSP build is what is vendored and why every `x-`
  attribute in the Rust names a property or a method rather than an expression. Markup is
  a tree of values with escaping by construction, not a template string.

- **Assets live in `share/cockpit/`, like every other thing this tool reads at run time.**
  The stylesheet is compiled from one source file by `scripts/cockpit-assets` and
  committed; `alpine.csp.min.js` is committed because the interaction depends on it and it
  is small; Cytoscape, Three.js and p5 are vendored at build time and are not committed,
  because they are two megabytes that nothing on the critical path needs. Node is a build
  dependency and never a runtime one.

- **The pages are benchmark targets.** The landing page, the capability table and a graph
  page are declared beside `GET /` and `GET /docs` as system targets, so the benchmark
  coverage counts them and a baseline can regress on them.

## Alternatives rejected

- **A client-side application (React, Vue, Svelte).** It would hold backend domain state in
  a second place, need its own build toolchain at runtime, and make the first paint depend
  on JavaScript. Nothing the Cockpit does needs a virtual DOM.
- **A separate server process for the UI.** A second process would need its own copy of the
  index or an HTTP hop for every fact, and the shared-server lease already gives one server
  per repository.
- **A template engine (Askama, MiniJinja).** A new dependency to gain string interpolation,
  and interpolation is exactly where escaping is forgotten. The element builder is a
  hundred lines, escapes by construction, and its tests are the security tests.
- **Fetching the vendor libraries from a CDN.** The Swagger UI shell already does this and
  it is the one part of the HTTP projection that is not available offline. Repeating that
  for the Cockpit would make a local developer tool depend on the network.
- **Re-rendering every committed projection inside `health.report`.** It answers "what is
  stale" exactly, and it rebuilds canonical state on every request, which the hot-path rule
  forbids and the test suite catches.
- **Shelling out to `majordomus doctor` for the health page.** The shell tool's doctor
  decides whether *Majordomus is wired into this repository*; the Rust engines decide
  whether *what this process serves is sound*. They are different subjects, and the Rust
  server does not dispatch shell.

## Consequences

Adding a capability adds a Cockpit page, a navigation entry, a search entry, a palette
entry, a generated form, an OpenAPI operation and a benchmark target, with no edit to the
Cockpit — and one more browser-probed route, because `scripts/cockpit-probe` derives its
routes from the running server rather than from a list.
`apps/majordomus-cli/tests/cockpit.rs` runs that claim: it adds a rule to a
disposable repository and asserts the capability reaches the listing, a page of its own,
the object explorer, the search and the listing the palette reads.

The costs are real and named. The repository now vendors third-party JavaScript, which is
new: one file committed and three built, all pinned in `package.json`, all served from this
origin, none of them required. The stylesheet is a generated artifact that needs Node to
regenerate, so `scripts/cockpit-assets --check` is a gate that a Node-less checkout cannot
run — it fails loudly rather than passing quietly. And a page is one more reader of the
capability outputs, so a change to an output shape that a projection tolerated may now be
visible on a page; that is the point, but it is a change in blast radius.

What is not decided here: whether the Cockpit ever writes. Every capability it can reach is
a query or a command over this process's own memory, and nothing in it writes to the
repository. A capability that did would need its own decision.
