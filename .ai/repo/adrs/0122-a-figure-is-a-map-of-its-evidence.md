---
schema: adr/v1
id: adr-0122
kind: adr
title: A figure is a map of its evidence
status: accepted
date: 2026-10-09
tags:
  - architecture
  - cockpit
  - ui
  - evidence
  - design
related:
  - file:.ai/repo/adrs/0012-the-cockpit-is-a-projection-not-an-application.md
  - file:.ai/repo/adrs/0089-an-interface-shows-only-what-is-derived-and-unknown-is-never-healthy.md
  - rule:project.figures-are-maps-of-evidence
  - rule:project.ui-derived-state
  - file:apps/majordomus-cli/src/cockpit/figure.rs
  - file:apps/majordomus-cli/src/cockpit/pages.rs
  - file:share/cockpit/flow.js
  - file:share/design/tokens.yaml
  - file:docs/COCKPIT.md
provenance:
  origin: authored
---

# 122. A figure is a map of its evidence

Accepted by the repository owner on 2026-10-09, in the session that implemented it, in so many
words ("accept it"), after the implementation, its tests and its pull request
(#833) were reported to them.

## Context

The Cockpit draws pictures in two ways. The graph viewer hands a capability's nodes and
edges to Cytoscape. The topology and activity views draw on a canvas. Both are
enhancements over tables, which ADR 12 requires. Neither says, on the drawing, how sure
it is of what it draws. A derived backlink and a declared reference are the same grey
line. A box tells the reader nothing when touched. Everything the drawing knows about
an element is in the table under it, and the table is a different place from the thing
the reader is looking at.

A due-diligence report built in the owner's other repository solved this for a person
who has to act on a figure. In its investor section, every box and arrow of the
ownership flow is a control. Choosing one fills a box under the drawing with what is
known of it, the type of the claim (verified, derived, estimated, missing, unreadable)
and the source document. A box with more behind it carries a `+` that opens a subtree,
one level at a time, and one button opens or closes all of them. Line style carries
provenance: a solid line is a registry entry, a dashed one is an inference, a dotted one
is history. The legend says so, and the caption says what on the figure is fact. Every
chart has the same data in a table under it. The owner asked for this form, its
ergonomics and its look to become the way this product draws, and to be used here
first.

ADR 89 already forbids drawing unknown as healthy. It does not say what a drawing owes
its reader, and nothing stopped a figure from showing a fact with no way to ask where it
came from.

## Decision

A figure in a Majordomus interface is a map of its evidence. It is rendered by the
component `cockpit::figure::Flow` and obeys the rule `project.figures-are-maps-of-evidence`:

1. **The head names the question** the figure answers. The caption says what on it is
   recorded and what is inferred.
2. **Every element carries a claim**, `figure::Claim`. The claim is one of `declared`,
   `derived`, `estimated`, `external`, `historical`, `missing` or `unknown`, and each is
   a status word filed in `share/design/tokens.yaml`. The colour therefore comes from the
   design declaration, as a badge's does. The claim is also a dash the server draws:
   solid for declared and external, dashed for derived, dash-dot for estimated, dotted for
   historical and missing. A derived edge never looks like a declared one, in colour or
   without it. The legend lists exactly the claims drawn, with what each means.
   `unknown` means the source could not be read. It is never drawn as clean or empty.
3. **Choosing an element explains it.** Every box and line is a keyboard-reachable
   control (`tabindex`, `role=button`, `aria-pressed`) that answers a pointer, a finger,
   Enter and Space. Its explanation is server-rendered into an inert `<template>` beside
   it. The chosen element and what it is joined to stay lit and the rest steps back.
   The choice is the URL fragment, and Escape or the empty drawing clears it.
4. **Detail lives in subtrees, not in the drawing.** A box stands for a group, carries
   the count of its members, and its members are a native `<details>` tree under the
   figure that opens one level at a time. The drawing stays readable however large the
   data. One control opens or closes every subtree.
5. **The legend is also a filter.** Each claim is a switch that takes the elements making
   it out of the drawing, and a box left with no line goes with them, so a reader can
   look at the record with the inferences removed.
6. **The drawing is an enhancement.** The same facts are in a table under it, open by
   default. A reader with no script and no SVG has all of them. A page that has a richer
   table hands it to the figure instead of the generated one.
7. **The server lays it out.** Columns left to right, each centred on the tallest,
   orthogonal lines from the side that faces their target. The SVG is a function of the
   answer alone, byte for byte, and no layout library ships to the browser.
   `share/cockpit/flow.js` adds only what a script must: choosing, tracing, filtering,
   opening and addressing. It computes no fact.

The first page drawn this way is an entity's relations: what names it on the left, the
entity in the middle with its own evidence as its subtree, what it names on the right.
One box stands for each relation name in each direction.

The component's types are public (`majordomus_cli::cockpit::figure`) so that every
interface that draws this product — the Cockpit now, a successor UI or the static site
later — renders one grammar rather than a second copy of it.

## What this does not change

The Cytoscape graph viewer stays for graphs of hundreds of nodes, where columns do not
fit. A figure is for a question with a subject and its neighbourhood. The topology and
activity canvases are measurements over time, not maps of claims. ADR 12 holds: pages
stay server-rendered and complete without a script. The columns are built in Rust from a
capability's answer. They are not a layout description language kept as data, which the
`capability-graph` milestone's non-scope rules out.

## Alternatives rejected

- **Tooltips.** A tooltip needs a hovering pointer, which a finger does not have, and it
  disappears as the reader moves toward the link inside it. The information box stays.
- **Cytoscape for every figure.** It ships 400 KB for a dozen boxes, lays out
  differently on every load, and its canvas is opaque to the accessibility tree.
- **Colour alone for the claim.** Fails without colour vision, in print and in a
  screenshot pasted into a chat. The word and the dash carry the claim; the colour
  repeats it.
- **Porting the report's markup as it is.** The report uses hex colours that drift from
  its own tokens, draws "missing" and "unreadable" in one red, and gives one register
  status a blue badge and a grey slice. The port keeps the grammar and takes the colours
  from the design declaration, one word per claim.

## Consequences

- `tokens.yaml` files three more words: `derived` (info), `estimated` (warn) and
  `historical` (neutral). `missing`, `external` and `unknown` were already filed.
- Every new figure in an interface is a `Flow`, or it explains in its own ADR why it is
  not one. The rule's gates are the component's unit tests, case 1010 over a served
  fixture, and the `figure` check of `scripts/cockpit-probe --interactions`, which chooses
  every element of a rendered figure in a browser and requires an explanation for each.
- The entity page gains a script module. Without it, the page is the page it was, plus
  the drawing.
