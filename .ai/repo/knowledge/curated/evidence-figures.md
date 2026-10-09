---
schema: knowledge/v1
id: evidence-figures
kind: knowledge
class: lesson
title: What porting the due-diligence figure taught, and the defects it was not allowed to bring along
description: Where the evidence-figure grammar came from, which of the source report's techniques were kept, which were deliberately left behind, and the defects found in the source that the port corrects.
status: verified
epistemics: observed
date: 2026-10-09
tags:
  - ui
  - cockpit
  - figure
  - evidence
  - lesson
provenance:
  origin: authored
  derived_from:
    - file:.ai/repo/adrs/0122-a-figure-is-a-map-of-its-evidence.md
    - file:apps/majordomus-cli/src/cockpit/figure.rs
    - file:share/cockpit/flow.js
relations:
  - type: documents
    target: rule:project.figures-are-maps-of-evidence
---

# Evidence figures: what the port kept and what it fixed

The grammar of ADR 0122 comes from the investor section of a due-diligence report
generated in the owner's other repository (`prismatic-platform`, case ADNZ-633, report
v53, built by `tools/build_linked_report.py` and `tools/viz.py`). That report is the
reference for how the figure should feel. These notes record what a reader of the ADR
cannot see: which of its techniques were kept, what was left behind, and the defects the
port must not reintroduce.

## Kept

- **Five claim types, and they are not interchangeable.** The report's
  ov/od/oh/ch/nz are `declared`/`derived`/`estimated`/`missing`/`unknown` here.
  `missing` means "looked for and not found, and it says where". `unknown` means "the
  source could not be read". An unread source is never a negative. The report's
  investigation rules say a failed adapter is UNKNOWN, never merged with a confirmed
  empty result, and `figure::Claim` keeps that split.
- **Grey means context, not a verdict.** In the report, grey marked whatever was not the
  subject: a third party's land, a boundary total, an incomparable product. Here that is
  `external`, filed neutral. `historical` is the report's dim dotted "formerly" edge.
- **Line style carries provenance.** Solid is a record, dashed is an inference, dotted is
  history. An estimate is hollow: the report drew estimated bars as outlines only, and
  the figure draws an estimated box without fill.
- **A click shows evidence, and the neighbourhood is traced.** The report dimmed
  everything outside a tapped node's neighbourhood and showed its note in an info box.
  The legend's layer switches hid an edge class together with the nodes it stranded.
- **A wide invisible hit path** under each thin line (16 units), so a line can be chosen
  with a finger.
- **Subtrees open one level at a time**, with open-all and close-all.
- **A table behind every drawing.**
- **Validators that drive a browser.** `check_tree.cjs` and `check_viz.cjs` clicked
  every element and required a note. Here the `figure` check of
  `scripts/cockpit-probe --interactions` does the same in a real browser, and case 1010
  holds the served markup.

## Left behind

- **Hex colours in the figure code.** The report's chart colours drifted from its own
  CSS tokens (`#7AA2F7` against `--blue #7FB2FF`). Here every colour is a status word
  resolved by `share/design/tokens.yaml`, so there is nothing to drift.
- **One red for two claims.** The report's `ch` and `nz` badges shared a colour, so
  "not found" and "could not read" looked the same. Here `missing` is bad and `unknown`
  is neutral, with different dashes.
- **A badge and a slice that disagree.** The report's register gave one status a blue
  badge and a grey chart slice, and another a red badge and an amber slice. A claim here
  has one word, so its badge and its line cannot disagree.
- **Client-side layout.** The report laid out its network with Cytoscape in the browser.
  The figure is laid out by the server, deterministically, and ships no library.

## Traps met

- An SVG `<g>` used as a button needs `tabindex`, `role` and a key handler. `<title>`
  alone gives a tooltip, not a control.
- A `<template>` keeps the explanation inert and CSP-clean. Inline JSON in a `<script>`
  would need a nonce or a digest.
- The Cockpit's CSP forbids inline scripts, so the figure's behaviour is one module,
  `flow.js`, added by the page that draws a figure. Pages that draw none do not load it.
