---
schema: feature/v1
id: cockpit
kind: feature
title: The Cockpit: the registry rendered for a person
short_title: Cockpit
headline: The same server that answers agents renders every capability, object, graph, worktree and health check as pages, with a runner generated from each capability's own schema; nothing on it is a second list.
summary: At /cockpit the shared server lays out what a capability answered — the registry explorer, every object of the layer, the derived graphs, the directory contracts, the continuity of this checkout, the worktree topology, one health report naming the engine behind every check — and every page is complete HTML before any script runs.
status: stable
weight: 100
featured: true
areas: [observability]
modules: [health]
rules: [project.interfaces-are-projections]
docs: [docs/COCKPIT.md]
adrs: [adr-0012, adr-0020]
cockpit: [overview, capabilities, objects, directories, graphs, continuity, worktrees, health, artifacts, api]
web: [cockpit]
related: [interfaces, declare-once]
tags: [cockpit, interface]
---

## What it does

Every page asks a capability through the one executor MCP and the HTTP routes use, so the
Cockpit cannot answer a question differently from the API. The sidebar's catalogues are the
registry's modules, the index's kinds and the graph derivations; a capability's page carries
its schemas, its projections, its provenance and a form generated from its input schema that
calls its real route. A capability, a kind or a graph added to the backend has a page, a
navigation entry and a search entry the first time it exists, with no line of the Cockpit
written for it.

The browser layer is an enhancement in the strict sense: a command palette, the runner, a
graph drawing and two optional views, under a strict content-security policy with no inline
script and no remote origin. With JavaScript off every page still shows everything it knows.

## What it does not do

It holds no model, no catalogue and no verdict of its own, and it writes nothing: every
capability it can reach is a read, or the one command that changes this process's memory.
It is not the published website; that is a static projection of the same registry with no
server behind it.
