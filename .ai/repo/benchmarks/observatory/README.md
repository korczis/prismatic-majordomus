---
schema: context/v1
id: ai.repo.benchmarks.observatory
kind: context
title: Observatory evidence
description: The gates of the Observatory (G0 to G7) with their verdicts, and the browser measurements behind them, each naming its revision, command and machine.
status: active
scope: subtree
providers: ["*"]
audience: [human, agent]
composition: extend
order: 100
children:
  require_contract: false
---

# Observatory evidence

`manifest.json` holds the verdict of each gate of the Observatory (ADR 0158, I2680): `PASS`,
`FAIL`, `BLOCKED` or `NOT RUN`, each with the command that decided it, the environment it ran
in and the path of its evidence. A gate is never `PASS` without all three.

The browser measurements come from one script, `scripts/lib/ui-observatory.mjs`, run against
an origin:

```bash
node scripts/lib/ui-observatory.mjs --origin https://majordomus.dev --out <dir>
node scripts/lib/ui-observatory.mjs --origin http://127.0.0.1:<port> --out <dir> --soak-minutes 60
```

It writes `evidence.json` and two screenshots of the kiosk, at 1920x1080 and 3840x2160. The
evidence names the revision the page states, the machine, the operating system and the
browser. A frame rate is headless Chromium's on that machine: the display loop's rate, not a
panel's.

- `production/` is the published site: the revision it served, a snapshot, with the mesh
  scenes replayed from the repository's recording.
- `live/` is a server on a machine of the mesh: the newest event of the journal found in the
  display's own model, the recovery drills, and a soak with the page's memory read at the
  start and the end. Its screenshots are not committed, because a live server labels the
  machines of the mesh by their identities; the evidence records an event by its kind and
  clocks and whether the two identities were one, never by the identity.

`production/` and `live/` hold measurements, not sections of the layer, so they owe no
contract of their own (`children.require_contract: false`): this document states their format
once.

A measurement is a moment. It is replaced by running again, not edited.
