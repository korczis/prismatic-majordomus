---
schema: feature/v1
id: context
kind: feature
title: Every directory of the layer carries its contract, and a worker reads what applies
short_title: Context
headline: Context is a tree, not one file: the documents that apply to a path are resolved from the root down, the nearest one adds and never silently replaces, and a worker loads what its task needs and nothing more.
summary: Every README under .ai/ is a context document with an identity, a scope, the providers and audience it addresses and how it composes with its ancestors; the repository scope declares what a worker reads and what it never reads; and the briefing a worker gets is assembled from durable state within a line budget with every exclusion named.
status: stable
weight: 140
featured: false
areas: [context]
modules: [directories, repository]
commands: [context]
kinds: [context, scope]
rules: [majordomus.context-integrity, majordomus.minimum-sufficient-context, project.context-locality, project.scope-is-declared, majordomus.layout-integrity, majordomus.ai-layout-integrity]
docs: [docs/CONTEXT.md, docs/SCOPE.md]
adrs: [adr-0011]
claims: [context-documents, context-coverage, context-impact, context-selection-budget, minimum-context, scope-declared, ai-layer-manifest, local-state-ignored, no-counts-in-context]
use_cases: [read-only-the-context-that-fits, document-every-directory-of-the-layer, trace-a-change-to-the-context-it-affects, classify-what-belongs-in-the-context]
cockpit: [directories]
related: [continuity, policy]
tags: [context, scope]
---

## What it does

`majordomus context resolve <path>` prints the documents that apply to a path, least
specific first; `context explain` says why each is in or out; `context validate` refuses a
tree with a missing contract, a duplicate identity, a broken or cyclic override, or an
override of a document marked final; `context affected` names the documents a change set
touches. The repository scope is declared once, out over in, and the executable discovers,
indexes and serves nothing outside it.

## What it does not do

A provider's own nested-file loading is an optimisation the tool does not replace; the
resolution is what applies. The checkout-local half of the layer is never context and never
published.
