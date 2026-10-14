+++
title = "The Observatory at /ui/observatory and /ui/kiosk shows a scene only when the capabilities it requires answered with something to draw, or, for a scene of the mesh where no mesh runs, when the repository carries a recording of it, which it labels a replay; a scene without either carries no model and states the source's reason, on a server and in the published snapshot alike"
description = "The Observatory (/ui/observatory, and /ui/kiosk as its unattended preset) draws the"
weight = 198
[extra]
claim_id = "observatory-draws-nothing-it-did-not-read"
status = "guaranteed"
source = "docs/claims/observatory-draws-nothing-it-did-not-read.md"
+++
{% raw %}

## What it means

The Observatory (`/ui/observatory`, and `/ui/kiosk` as its unattended preset) draws the
repository and its mesh as rotating scenes. Every scene is declared once, with the
capabilities it requires. A scene is available only when those capabilities answered with
something to draw; otherwise it carries no model, states the source's own reason, and the
rotation does not show it. A process that runs no mesh, which is what reads the published
snapshot, has no live mesh scene to show: there a scene of the mesh draws the recording the
repository carries and says it is a replay, and without a recording it says it is
unavailable.

## How it works

`apps/majordomus-cli/src/ui/observatory.rs` holds the one inventory, `SCENES`, and projects
the answers of nine capabilities into one bounded document, `observatory/v1`, which the page
carries in its `data-observatory` attribute. `share/ui/observatory.js` draws that document
and decides nothing about it. The same projection is a table on the page, one row per
scene, for a reader without the script.

`test/cases/3011_the_observatory_draws_nothing_it_did_not_read.sh` proves it on a server
over a freshly initialised repository and on `majordomus web report ui-snapshot`:

- `/ui/observatory` and `/ui/kiosk` answer, and carry the same scenes in the same order;
- the document names every source with the state it answered in, and a failed one with a
  reason;
- an available scene has a model, an unavailable one has none and has a reason, and no
  available semantic scene requires a source that failed;
- the three scenes of the mesh are unavailable where no mesh runs and no recording is
  carried; with a recording, the recorded scene is available, says `replay` with the time it
  was recorded, and the scenes it does not carry stay unavailable; a recording that is not one
  is a failed source and is never drawn;
- `/ui/screensaver` redirects to `/ui/kiosk` and `/ui/obsrvatory` to `/ui/observatory`;
- the snapshot holds the page and its script.

## How to see it

```bash
majordomus serve                       # logs the URL of the shared server
# open <url>/ui/observatory, or <url>/ui/kiosk for the unattended display
bash test/run.sh 3011_the_observatory_draws_nothing_it_did_not_read
```

## What it does not cover

It proves which scenes are available and that an unavailable one carries no model. It does
not prove what the canvas paints: that each mark of an available scene is placed and
coloured from its model is the script's, and no test reads pixels. The unit tests of the
module hold the projection itself, including that an event's body is never projected.

It does not prove frame rate, a 4K composition or that a kiosk survives a day; those are
I2680. It does not prove a live stream: the display reads its page again (I2679).

## Why it exists

A wall display is believed from across a room by people who will never open a source line.
A picture that drew a healthy constellation where no mesh was running, or a river of
events that never happened, would be the most persuasive false statement this repository
could make. The claim makes absence a stated fact of the page, and the case holds it on the
two processes that serve the page: a server, and the reader of the published snapshot.
{% endraw %}
