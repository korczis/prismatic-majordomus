# The Observatory draws nothing it did not read

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
the answers of the capabilities it names in `SOURCES` into one bounded document, `observatory/v1`, which the page
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

`test/cases/3026_the_observatory_answers_a_click.sh` proves what a reader who points at a
mark is told, on a server over a freshly initialised repository:

- every source says how its reading stands: current, stale or pending;
- every painter of the director, drawing the server's own model, registers each mark it
  drew with an identity that is a record of that model, a kind, a state and a truth class
  (observed or derived): the detail a click opens is made of those words;
- the scripts fetch only the page and the journal's events, and send nothing that would
  change anything.

## How to see it

```bash
majordomus serve                       # logs the URL of the shared server
# open <url>/ui/observatory, or <url>/ui/kiosk for the unattended display
# point at any mark, or walk them with the arrow keys and Enter; Esc closes the detail
bash test/run.sh 3011_the_observatory_draws_nothing_it_did_not_read
bash test/run.sh 3026_the_observatory_answers_a_click
```

## What it does not cover

It proves which scenes are available and that an unavailable one carries no model. It does
not prove what the canvas paints: that each mark of an available scene is placed and
coloured from its model is the script's, and no test reads pixels. Case 3026 reads the marks
the director's own painters register, not those of the p5 scenes of ADR 0160, which need the
vendored p5 to draw at all. The unit tests of the
module hold the projection itself, including that an event's body is never projected.

It does not prove frame rate, a 4K composition or that a kiosk keeps running; those are
measured by `scripts/lib/ui-observatory.mjs` and recorded with their machine in
`.ai/repo/benchmarks/observatory/`. The cursor the display follows between readings (`observatory.events`) is proven by the
mesh cooperation test, not by this case.

## Why it exists

A wall display is believed from across a room by people who will never open a source line.
A picture that drew a healthy constellation where no mesh was running, or a river of
events that never happened, would be the most persuasive false statement this repository
could make. The claim makes absence a stated fact of the page, and the case holds it on the
two processes that serve the page: a server, and the reader of the published snapshot.
