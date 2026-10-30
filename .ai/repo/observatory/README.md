---
schema: context/v1
id: ai.repo.observatory
kind: context
title: Observatory recording
description: The recording of the mesh scenes the published Observatory replays where no mesh runs, captured from a running server and committed.
status: active
scope: subtree
providers: ["*"]
audience: [human, agent]
composition: extend
order: 100
---

# Observatory recording

`recording.json` is what the Observatory (`/ui/observatory`, `/ui/kiosk`) draws for its
three scenes of the mesh in a process that runs no mesh, which is what reads the published
snapshot. It is drawn as a replay, with the time it was recorded, and never as live
(ADR 0158).

It is the answer of `observatory.capture` on a server that runs the mesh, saved as it came:

```bash
curl -s <server>/api/v1/observatory/capture | jq -S . > .ai/repo/observatory/recording.json
```

That answer is already fit for publication: machines are `machine 1`, `machine 2` in the
order the mesh listed them, runtimes and sessions are ordinals, and an event keeps only its
kind, its logical clock and its time. Its `manifest` names the capabilities read, the
window of events held and what was removed and why. Never edit the file by hand and never add a scene to
it: `observatory.replay` refuses a recording that carries a scene other than `mesh`,
`activity` and `coop`, and a file it cannot read is an error, not an empty replay.

A recording is a moment. It is replaced by capturing again, not updated.
