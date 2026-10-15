+++
title = "A graph, a chart or a diagram is something a reader can act on, never a picture"
description = "A graph, a chart or a diagram is something a reader can act on, never a picture"
weight = 77
[extra]
kind = "rule"
slug = "project-a-visualisation-is-interactive-1"
identity = "project.a-visualisation-is-interactive@1"
status = "active"
source = ".ai/repo/rules/project/a-visualisation-is-interactive.v1.md"
+++
{% raw %}

## Rationale

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

## Required behaviour

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

## Enforcement

`scripts/ci/visualisation-check` is the gate, in every plan: it finds every served script
that loads a vendored library at run time — the only way a drawing library reaches a page —
and holds each to rule 1 or to the baseline. `test/cases/1102_a_visualisation_is_interactive.sh`
holds the gate itself: that it finds this repository's drawings, fails on a new picture and on
a baseline line that no longer names one, and cannot pass by finding nothing; and that a
diagram drawn from a Markdown body carries its derived links and keeps its source.
{% endraw %}
