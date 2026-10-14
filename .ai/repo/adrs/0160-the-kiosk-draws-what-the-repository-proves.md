---
schema: adr/v1
id: adr-0160
kind: adr
title: The kiosk draws what the repository proves, from the sources that already answer it, and draws it with p5 only where the build has it
status: proposed
date: 2026-10-14
tags:
  - ui
  - observatory
  - evidence
  - p5
related:
  - file:.ai/repo/adrs/0158-the-observatory-draws-only-what-was-observed.md
  - file:.ai/repo/adrs/0135-majordomus-ui-is-the-primary-surface.md
  - file:.ai/repo/adrs/0012-the-cockpit-is-a-projection-not-an-application.md
  - file:.ai/repo/adrs/0142-no-screen-waits-for-a-source.md
  - file:.ai/repo/rules/project/ui-derived-state.v1.md
  - test:test/cases/3014_the_kiosk_draws_what_the_repository_proves.sh
  - file:apps/majordomus-cli/src/ui/observatory.rs
  - file:share/ui/observatory-kiosk.js
provenance:
  origin: authored
---

# 160. The kiosk draws what the repository proves, from the sources that already answer it, and draws it with p5 only where the build has it

## Context

The Observatory (ADR 0158) has eight scenes and, of those, four read the mesh: on the public
site, and on any machine that runs no mesh, they are unavailable and say why. A display that
is left running on a wall then shows a handful of scenes and a long list of reasons. The
owner asked for more screens at `/ui/kiosk/`, all of them derived and inferred, drawn with
p5.js, dense and beautiful, in one look.

The repository already answers, without a mesh, the questions a wall of this kind is for:
which of its rules have been proved and which only named a proof, which of the claims its
documents make have a test that ran, what its public surface measures, what may run at the
same time in the plan, and whether the published installation is whole. Those are
capabilities every surface already calls (`rules.report`, `evidence.report`,
`quality.report`, `plan.waves`, `distribution.status`). They are also the facts a reader
most needs to see and most easily mistakes: a rule drawn as passing because nothing said
otherwise is the failure ADR 0158 exists to prevent, in a prettier medium.

p5 is already a decision of this repository: a pinned, vendored library that a page loads
only when it draws one (ADR 0012, ADR 0135). The Observatory has not used it; it draws on a
plain canvas, and the Cockpit and UI assets have one way each of carrying a library that is
not committed.

## Decision

1. **Five more scenes, each a function of one capability's answer.** They are added to the
   one inventory of ADR 0158, point 3, and to nothing else: no capability, no transport, no
   store.

   | scene | reads | a mark is |
   |---|---|---|
   | `proof`, Proof Lattice | `rules.report` | one rule, coloured by the state word the proof report decided |
   | `claims`, Claim Ledger | `evidence.report` | one kind of claim, a bar split by what the evidence says of its claims |
   | `surface`, Surface Atlas | `quality.report` | a count the quality report measured: operations per surface, documented, exampled and tested |
   | `waves`, Wave Front | `plan.waves` | one wave of the plan, a column split by its issues' recorded status |
   | `matrix`, Release Matrix | `distribution.status` | one check of the published installation, in the word it decided |

2. **Counted, never estimated.** A number on these scenes is a count a source reported, or
   a sum of such counts that the model states. A scene whose source holds nothing to draw is
   unavailable with the source's reason (ADR 0158, point 5), and an unknown or unproven word
   is drawn as that word, in the muted tone, not as passing. Where a source reports more
   items than a display can hold, the scene groups rather than truncates, and says how many
   it drew.

3. **p5 where the build has it, a plain statement where it does not.** The scenes are drawn
   with p5 in instance mode onto a surface of their own, which the director composes onto
   its stage: one clock, one rotation, one fade and one set of status words serve every
   scene, and the first eight scenes are not rewritten. p5 is the pinned library
   `scripts/cockpit-assets` copies into `share/ui/vendor/`, not committed, loaded lazily from
   the address the page names. A checkout that was built without it draws these scenes as
   "p5 is not built here" on the stage and keeps the same facts in the table beside it. Nothing
   is drawn in its place.

4. **One look.** The scenes take their colours from the stylesheet's tokens through the
   director's own helpers and the state words' tones from one table, so that the same word is
   the same colour on every scene and in the Cockpit.

5. **Motion is decoration and is said to be.** A cell that breathes does so as a function of
   its name, not of its state; an arc ends where its count says and only the glint on it
   moves. Reduced motion draws every scene still.

## Consequences

- The public site shows thirteen scenes of which nine have something true to draw on a CI
  runner, instead of five, and the five new ones are the ones a reader most wants to doubt.
- Each new scene reads a capability that can also be asked from the command line, the API and
  MCP, so every mark can be checked there.
- p5 is about a megabyte and is loaded only by this screen, only when a p5 scene is shown. A
  display that never rotates to one never loads it.
- A new capability that reports a count is a new scene by declaration and a model. A scene
  that wants a number no capability reports waits for the capability, not for a figure.
- The `rules.report` and `evidence.report` sources take about two seconds to compute and are
  asked on every page build; they are read as the latest reading (ADR 0142), so the screen
  waits for neither.

## Alternatives rejected

**Redrawing the first eight scenes with p5.** It would give the display one renderer and
cost a rewrite of scenes that are held by case 3011 and by a person's attention for a
different issue, in a file another worker is changing.

**A second page for the new scenes.** A kiosk, a screensaver and a screen are one display
with different chrome (ADR 0158); two pages would disagree within a week.

**Committing p5.** It is a megabyte nothing on the critical path needs, and the repository's
rule for such a library is that it is built, not committed.

## Verification

Case 3014 (`the_kiosk_draws_what_the_repository_proves`) proves on the built executable that
the five scenes are in the inventory with their sources, that each one's model is the count
its source answered (the oracle counts the same capability's answer again), that a scene with
a failed source says why, that the page names the scenes script and the drawing library, and
that the kiosk preset still differs from the screen by one attribute. The Rust unit tests
hold the models on constructed answers, including the empty ones. What only a browser can
show, that the scenes are drawn at all, is the evidence of the screenshots I2698 records.
