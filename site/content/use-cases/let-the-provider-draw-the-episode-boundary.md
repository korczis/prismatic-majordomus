+++
title = "Have the session opened and closed without anybody remembering to"
description = "Wire the provider''s own session events, and the episode opens when the sitting begins, closes when it ends, and leaves behind the context the worker was given at the open."
weight = 31
[extra]
id = "let-the-provider-draw-the-episode-boundary"
source = ".ai/repo/use-cases/let-the-provider-draw-the-episode-boundary.md"
category = "continuity"
maturity = "guaranteed"
+++

## Situation

The session record is only as good as the discipline that opens it. A worker that is asked to run `session start` runs it when it remembers, which is not the sitting that ended in a crash, a compaction, or somebody closing the window — and those are the ones somebody later needs to read about. Meanwhile the layer names a store for the working context of an episode and nothing has ever written a file into it, so a reader cannot tell an empty store from an unimplemented one.

## Outcome

The provider's own `SessionStart` and `SessionEnd` events open and close the episode. Both are idempotent, because the events are: a resume keeps the open episode and an end with nothing open writes nothing. The open freezes what the context builder resolved, next to a section for the worker's own notes, and the close appends the outcome and the record it wrote. `doctor` holds the repository to the wiring by driving a payload through the shim, and refuses a working context that carries a conversation.
