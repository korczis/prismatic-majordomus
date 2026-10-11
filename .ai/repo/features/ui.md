---
schema: feature/v1
id: ui
kind: feature
title: "majordomus ui: the delivery, for a person"
short_title: UI
headline: The shared server's primary surface answers how the delivery stands — health, releases, the roadmap, the work and its pull requests, the mesh, the evidence and the rules — and every value on it names the capability it was read from and every other surface that serves it.
summary: At /ui the shared server lays out the delivery in the project's own words, composed only of what capabilities answered, asked together so the first screen paints in the time of its slowest source; a panel whose source is slow arrives on its own route, and the Cockpit beneath it is one click away for the source of any value.
status: draft
weight: 95
domain: surfaces
areas: [observability]
modules: [repository, health, release, distribution, plan, intents, mesh, integration, deploy, gates, evidence, rules, capabilities]
rules: [project.ui-derived-state, project.interfaces-are-projections, project.web-surface-declared-once]
docs: [docs/MAJORDOMUS_UI.md]
adrs: [adr-0135]
web: [ui]
related: [cockpit, interfaces]
tags: [ui, interface]
---

## What it does

`majordomus ui` is the surface a person is sent to first. The Cockpit renders the registry
(every capability, object, graph and route). The UI renders the delivery: whether the
process is healthy, which version was released and which the commits imply next, how many
issues are ready or blocked, which milestones are still open, which machines are on the mesh
and how alive each runtime is, and what the integration lane would merge, repair or wait for.

Every tile and panel cites its capability, the pointer it read, the route that serves the
same answer and how long it took, and links to that capability's page in the Cockpit. A
source that cannot answer reads "not answered", and a value it does not carry reads "none" or
"unknown", never zero or healthy. The sources of a screen are asked at once. A source too
slow for the first paint, such as the pull-request queue, is a panel with a route of its own,
pulled in by the one script once the screen has painted and linked for a reader without it.

## What it does not do

It is not a second application. It has no datastore, no status vocabulary and no route list
of its own beyond the screens its dispatcher answers. It runs no server of its own either:
the shared server that serves the API, MCP and the Cockpit serves it too, under the same
content-security policy. It does not replace the Cockpit, which stays the technical layer
(ADR 0135), and it writes nothing: an action on it is a capability with its own route.
