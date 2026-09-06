+++
title = "Have the session opened and closed without anybody remembering to"
description = "Wire the provider''s own session events, and the episode opens when the sitting begins, hands the worker what the last one left, records what a compaction is about to discard, and closes with a continuation record beside its envelope."
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

The provider's own `SessionStart`, `PreCompact` and `SessionEnd` events run the lifecycle. Each is idempotent, because the events are: a resume keeps the open episode, an end with nothing open writes nothing, and a compaction with no active task records nothing.

The open freezes what the context builder resolved, next to a section for the worker's own notes, and writes a briefing to standard output — which the provider adds to the context it is about to build. That is the step that makes the record readable as well as written: a continuation package nothing loads is one nobody reads. A compaction records a derived checkpoint, because the conversation is about to stop holding what it knows. An end with the task still active writes a derived handover before the envelope closes, so the next worker inherits both an index of the episode and something to act on.

`doctor` holds the repository to the wiring by driving a payload through the shim, and refuses a working context that carries a conversation. `majordomus_continuity` reads the same state back over MCP, over HTTP and in the Cockpit, with the same two tiers and the same four divergence labels — and never writes, because the lifecycle has one writer.
