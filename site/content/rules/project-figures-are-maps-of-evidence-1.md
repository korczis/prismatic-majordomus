+++
title = "A figure is a map of its evidence, and every element on it explains itself"
description = "A figure is a map of its evidence, and every element on it explains itself"
weight = 100
[extra]
kind = "rule"
slug = "project-figures-are-maps-of-evidence-1"
identity = "project.figures-are-maps-of-evidence@1"
status = "active"
source = ".ai/repo/rules/project/figures-are-maps-of-evidence.v1.md"
+++
{% raw %}

## Rationale

A picture is read before the table under it, and a reader acts on what the picture
says. When a derived line looks like a declared one, or a box cannot be asked where its
number came from, the drawing claims more than the data does. ADR 0122 records the
decision and where the grammar came from: the ownership flow of a due-diligence report,
in which every box and arrow said what it was, how sure the report was of it, and which
document it came from.

## Required behaviour

1. **The question is named.** A figure has a head that states the question it answers,
   and a caption that says what on it is recorded and what is inferred.
2. **Every element has a claim.** Each box and line is `declared`, `derived`,
   `estimated`, `external`, `historical`, `missing` or `unknown`. The claim is a status
   word filed in `share/design/tokens.yaml` and a dash drawn by the server. Colour is
   never the only carrier.
3. **Unknown is not a negative.** An element whose source could not be read is
   `unknown`. It is never drawn as declared, as missing, or left out.
4. **Every element explains itself.** Each is a control reachable by keyboard
   (`tabindex="0"`, `role="button"`, `aria-pressed`). Choosing it shows its explanation:
   title, claim, note and the address of its source when it has one.
5. **Detail lives in subtrees.** A box that stands for several things carries their
   count. They open under the figure as a native `<details>` tree, one level at a time.
6. **The legend is exact.** It lists the claims the figure draws and no other, each with
   its meaning.
7. **The data is under the drawing.** Every fact the drawing shows is also in a table
   that needs no script.

## Failure behaviour

The component makes 1, 2, 5, 6 and 7 true by construction: a page that builds a `Flow`
cannot omit them. The unit tests in `apps/majordomus-cli/src/cockpit/figure.rs` fail the
build when the component stops doing so.
`test/cases/1010_a_figure_is_a_map_of_its_evidence.sh` serves a real repository's
Cockpit and requires, on a rendered entity page, a figure whose every drawn element has
an explanation, whose legend names only drawn claims, and whose script and stylesheet
are served. A figure drawn outside the component is a review finding until a gate can
see it. That gap is open, and this rule says so rather than claiming it closed.

## Verification

`cargo test --lib cockpit::figure` and `bash test/run.sh 1010_a_figure_is_a_map_of_its_evidence`.
`scripts/cockpit-probe --interactions` (the `figure` check of
`scripts/lib/cockpit-probe.mjs`) finds the entity page whose figure draws the most elements
and works it in a real browser. It chooses every element, by pointer and by keyboard, and
requires its explanation in the information box. It requires the subtree controls to open
and close every subtree, a legend switch to take its claim out of the drawing and bring it
back, and the fragment to restore the choice. The page sweep of the same probe holds every
page free of horizontal overflow at every declared width.
{% endraw %}
