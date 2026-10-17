---
schema: feature/v1
id: observatory
kind: feature
title: "The Observatory: the repository and its mesh, drawn for a wall"
short_title: Observatory
headline: One screen of the UI rotates eight scenes on a display nobody is sitting at, and every mark on it is a value a capability answered; a scene with nothing true to draw says why and is never drawn empty.
summary: At /ui/observatory, and at /ui/kiosk filling the window unattended, the shared server draws the machines of the mesh and their runtimes, the newest events of its journal, the milestones of the plan, the dimensions of health, the published releases and the sessions cooperating, from the same capabilities the other screens read. The published site carries it as a snapshot that says which commit it was read at.
status: stable
weight: 94
domain: surfaces
featured: true
areas: [observability]
modules: [observatory, mesh, plan, distribution, release, health, economics, repository, served]
kinds: [deployment]
rules: [project.ui-derived-state, project.interfaces-are-projections]
claims: [observatory-draws-nothing-it-did-not-read]
docs: [docs/MAJORDOMUS_UI.md]
adrs: [adr-0158]
web: [ui]
related: [ui, cockpit]
tags: [ui, interface, mesh]
---

## What it does

The other screens of `majordomus ui` are rows for a person who came to decide something.
The Observatory is the same facts as a picture, for a hallway display. It rotates eight
scenes: Mesh Cosmos, Event River, Intent Nebula, Token Spectrum, System Pulse, Delivery
Tree, Cooperation Choreography and Ambient Intelligence. Each scene names the capabilities
it requires, what its marks mean, and the screen where the same facts are rows.

A mark is a fact or it is decoration. A machine, a runtime, an event, a milestone, a health
dimension, a release and a session are drawn from what `mesh.peers`, `mesh.events`,
`plan.roadmap`, `health.report`, `distribution.releases` and `mesh.state` answered. The
drifting background and the slow motion are decoration, and the display says so.

A scene whose source did not answer, or answers that no mesh runs in this process, is
unavailable with the source's own reason. The rotation skips it. An event is drawn by its
kind, its stream and its clock; its body stays in the capability that owns it.

The status word is true: `LIVE` while the page was read again within forty seconds, `STALE`
when the server stopped answering, `SNAPSHOT` on the published site, with the commit it was
read at. `/ui/kiosk` is the same screen, projection and script filling the window.

## What it does not do

It adds no capability, no store and no transport: the page carries one bounded projection,
and the script asks the server for nothing but that page. It starts no work and writes no
record.

On the published site the process that takes the snapshot runs no mesh, so the three scenes
of the mesh replay the recording the repository carries, labelled `REPLAY` with the time it
was recorded; the recording is as old as the last capture someone committed. The display
follows the journal from a cursor between readings of its page (`observatory.events`). Its gates are recorded with their evidence in `.ai/repo/benchmarks/observatory/manifest.json`: screenshots at 1920x1080 and 3840x2160 of the published site, 60 frames per second in headless Chromium on an Apple M5 Pro, no WCAG 2.2 AA violation, reduced motion honoured, an hour-long soak with a stable page, and recovery drills. No physical wall display has been measured.
