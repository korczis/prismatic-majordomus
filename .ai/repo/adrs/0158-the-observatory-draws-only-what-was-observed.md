---
schema: adr/v1
id: adr-0158
kind: adr
title: The Observatory is one screen of the UI, and it draws only what was observed
status: proposed
date: 2026-10-14
tags:
  - ui
  - observatory
  - evidence
  - mesh
related:
  - file:.ai/repo/adrs/0142-no-screen-waits-for-a-source.md
  - file:.ai/repo/rules/project/ui-derived-state.v1.md
  - file:.ai/repo/rules/project/the-published-ui-is-the-trunks.v1.md
  - file:.ai/repo/rules/project/no-screen-waits-for-a-source.v1.md
  - test:test/cases/3011_the_observatory_draws_nothing_it_did_not_read.sh
  - file:docs/MAJORDOMUS_UI.md
  - file:apps/majordomus-cli/src/ui/observatory.rs
  - file:share/ui/observatory.js
provenance:
  origin: authored
---

# 158. The Observatory is one screen of the UI, and it draws only what was observed

## Context

Every screen of `majordomus ui` is a page of rows: a reading, its source, a link. That is
the right shape for a person who came to decide something and the wrong one for a display
nobody is sitting at. The owner asked, on 2026-10-14, for the second kind: a window into
how the repository and its mesh are working, good enough to leave running on a television,
and bound by one sentence above all others, "no invented operational data, ever". The
request arrived as a specification of eight scenes, a director that rotates them, a kiosk
mode, a live stream, a replay of recorded events and eight acceptance gates.

A display of that kind is where a product most often starts to lie. A picture that must
never look empty invites a particle for a peer that is not there and a pulse for work
nobody did, and a reader cannot tell the drawn fact from the drawn decoration. This
repository already forbids the same thing in rows: `project.ui-derived-state` refuses a
mocked value and a missing value drawn as zero or healthy. A picture needs the same
refusal said in its own terms, before the first scene is drawn rather than after the
eighth.

It is also where a second application starts. A kiosk, a screensaver and an interactive
view are three names for one thing seen with different amounts of chrome, and three
implementations of it would disagree within a week.

## Decision

1. **One screen.** The Observatory is a screen of `majordomus ui` like any other: a module
   under `apps/majordomus-cli/src/ui/`, an entry of `SCREENS`, sources asked through
   `source::ask_all`. It adds no capability, no transport, no store and no service. A fact
   it wants that no capability answers is a capability to add, by the canonical
   declaration, and until then a scene that says it is unavailable.

2. **One address, one preset, two redirects.** `/ui/observatory` is the screen.
   `/ui/kiosk` is the same screen, the same projection and the same script with the
   application's chrome removed; it differs by one attribute, never by a second
   implementation. `/ui/screensaver` answers a permanent redirect to `/ui/kiosk`, the name
   that says what it is, and `/ui/obsrvatory` one to `/ui/observatory`.

3. **One inventory of scenes.** The scenes are declared once, in the screen's module:
   identity, label, character, the capabilities each requires, what its marks mean, the
   screen that shows the same facts as rows, and how long it stays. The page, the script,
   the rows without script and the case read that one list.

4. **A scene is semantic or ambient, and says which.** A semantic scene binds marks to
   records: a peer, an event, an issue, a release. An ambient scene is decoration and is
   labelled as such; it may move by chance, and nothing in it stands for a fact.

5. **Nothing is drawn that was not read.** The page carries its whole projection as one
   document, `observatory/v1`: when it was observed, at which revision, every source asked
   with the state it answered in, its reason when it failed and how long it took, and
   every scene with its model. A semantic scene whose source failed, or answered with
   nothing to draw, is unavailable: it has no model, it carries the source's own reason,
   and the script does not rotate to it. It is never drawn empty, never drawn with
   stand-in marks, and never replaced by a picture of health.

6. **An event is projected by its identity, never its body.** What the Observatory takes
   from a mesh event is its stream and sequence, its kind, its node, its logical time and
   its wall time. Intents, paths and message bodies stay in the screens that are already
   decided to show them. The events are the journal's newest: `mesh.events` reads the journal
   forward from a clock, so a reader of "what is happening" asks it with `newest`, an input
   this decision added to the capability because the first page from the beginning was
   what the mesh screen had been showing as the latest.

7. **The status word is true.** The page says `LIVE` only when it was read again within a
   stated bound, `STALE` when it was not, and `SNAPSHOT` on the public site, where it reads
   nothing again and says at which revision it was taken. A scene of the mesh, in a process
   that runs none, draws the recording the repository carries and says `REPLAY` with the
   time it was recorded; nothing else stands in for it, and a scene the recording does not
   carry is unavailable.

10. **A recording names nothing by its identity** (amended 2026-10-14, I2678). The recording is
   what `observatory.capture` answers on a server that runs the mesh, committed at
   `.ai/repo/observatory/recording.json` and read by `observatory.replay`. Machines are
   numbered in the order the mesh listed them, runtimes and sessions are ordinals, and an
   event keeps its kind, its logical clock and its time; no node id, runtime id, session key,
   intent or path is carried. The owner asked for the mesh on the public site and for real
   data only; this is the form of real data that a public page may hold.

8. **It reads and never executes** (ADR 0142). A reader may choose a scene; nothing on the
   screen starts work, writes a record or changes the mesh.

9. **Every scene leads to its rows.** A scene names the screen where the same facts are a
   table, so that a mark a reader doubts can be checked against the record it stands for.

## What is delivered with this decision, and what is not

Delivered: the screen and its kiosk preset, the two redirects, the eight scene
declarations, the projection, the rows without script, a script that draws and rotates the
available scenes and polls its own page, and case 3011, which holds points 2, 3, 5 and 7
on a server and on the published snapshot.

Not delivered, and recorded as issues rather than drawn as if they were:

- a recording captured on a schedule: the replay on the public site is the last capture
  someone committed (I2678 delivered the replay itself);
- a pushed stream: since I2679 the display follows the journal from a cursor through
  `observatory.events` between readings of its page, and nothing is pushed to it;
- a physical wall display: every frame rate and screenshot is headless Chromium's on the
  machine its evidence names. The gates' verdicts and their evidence are in
  `.ai/repo/benchmarks/observatory/manifest.json` (I2680);

## Alternatives rejected

**A separate application for the kiosk.** It would have its own data model within a
release and its own defects after two. One screen with a preset cannot disagree with
itself.

**Sample data where a source is silent.** It makes a better first impression and removes
the one property the display is for. A reader who learns that one particle was invented
has no reason to believe the others.

**A new event store or stream protocol for the display.** The mesh already keeps an
ordered journal with cursors and the UI already has one way to ask a source. A display is
not a reason to own a second copy of either.

**Hiding unavailable scenes.** A scene that silently disappears reads as a product that
has fewer scenes. Naming it with its reason tells the reader what the repository would
have to run for it to appear.

## Consequences

- The public site shows fewer than eight scenes moving, and says why for each of the
  others. That is the cost of point 5 and it is accepted.
- A new scene is a declaration and a model; a scene's source is a capability, so every
  fact on the display can also be asked from the command line, the API and MCP.
- The Observatory is held by the same gates as every other screen, `ui-integrity` and
  `cockpit-projection` among them, and by case 3011 for what only a picture can get wrong.
- The remaining gates of the specification stay open as issues with their own evidence,
  and the screen is not described as complete until they are closed.
