---
id: project.a-visualisation-is-interactive
version: 1
kind: rule
title: A graph, a chart or a diagram is something a reader can act on, never a picture
description: Every drawing a served surface makes — a relationship graph, a chart, a diagram drawn from a Markdown body, a scene — binds a way from its marks to the records they stand for, is drawn from data the server derived and put on the page, and has a text form a reader without the script reads instead; the drawings that predate the rule are a named debt that may shrink and may not grow.
statement: A served script that draws binds a pointer or keyboard way from each mark to the record it stands for; what a mark leads to is derived by the server and carried on the page as data, never written in the script; the drawing has a text equivalent on the same page; and a drawing that binds nothing is either absent or listed in `.ai/repo/visualisation-baseline.txt`, which may not grow.
status: active
class: blocking
depends_on: [project.ui-derived-state@1, project.ui-conformance@1, project.every-link-and-control-is-tested@1, project.markdown-is-rendered-once@1]
tags: [ui, cockpit, visualisation, accessibility]

x-majordomus:
  tests: [test/cases/1102_a_visualisation_is_interactive.sh, scripts/ci/visualisation-check]
---

# Rationale

A drawing on a control surface stands for records. The nodes of a graph are issues and pull
requests, the bars of a chart are runs, the boxes of a diagram are documents and rules. A
reader looks at one because something in it is wrong or interesting, and the next thing the
reader wants is the record. A drawing that cannot be pressed sends them to find it by hand:
read the label, open another screen, search. That is the difference between a control
surface and a slide.

The surfaces had both kinds. The relationship graphs of the UI and the Cockpit, and the
train graph, take a press on a node to its record. The overview's activity field and the 3-D
topology draw the same records and bind nothing. Nothing said which kind a new drawing had to
be, so the next one would have been whichever its author found easier.

Three more things go wrong in the same place, and the rule holds them together:

- **Where a mark leads is written in the script.** A script that builds an address from a
  label is a second link resolver, and it is wrong the day a route moves.
- **The drawing is the only form.** A canvas is invisible to a screen reader, to a reader
  without script, and to the published snapshot's link check.
- **The source is lost.** A diagram drawn from a body replaces the text it was drawn from,
  and a reader who wants to quote or correct it cannot see it.

# Required behaviour

1. **A way through.** A served script that loads a drawing library binds a pointer handler
   that selects or navigates, or builds an anchor for the mark. A mark that stands for a
   record leads to that record.
2. **Derived, not written.** What a mark leads to is resolved by the server, by the
   capability and resolvers the surface already uses for its links, and put on the page as
   data (`data-graph` with each node's `href`; `data-mj-links` for a diagram drawn from a
   Markdown body). A script holds no route and builds no address from a name.
3. **A text form on the same page.** The table beside a graph, the source of a diagram: what
   a reader without the script, or without the library, reads instead. A diagram's source
   stays one press away after the figure is drawn.
4. **By keyboard too.** What a pointer can reach in a drawing, a keyboard can: an anchor, or
   a control in the tab order.
5. **No new pictures.** `.ai/repo/visualisation-baseline.txt` names the drawings that still
   bind nothing. A drawing that is not interactive and is not listed fails; a listed one that
   became interactive, or is gone, fails until the line is removed. The list is lowered in
   the commit that pays the debt.

# Enforcement

`scripts/ci/visualisation-check` is the gate, in every plan: it finds every served script
that loads a vendored library at run time — the only way a drawing library reaches a page —
and holds each to rule 1 or to the baseline. `test/cases/1102_a_visualisation_is_interactive.sh`
holds the gate itself: that it finds this repository's drawings, fails on a new picture and on
a baseline line that no longer names one, and cannot pass by finding nothing; and that a
diagram drawn from a Markdown body carries its derived links and keeps its source.
