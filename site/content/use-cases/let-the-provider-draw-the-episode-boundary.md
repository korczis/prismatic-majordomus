+++
title = "Have the session opened and closed without anybody remembering to"
description = "Wire the provider''s own session events, and the episode opens when the sitting begins, closes when it ends, and leaves behind the context the worker was given at the open."
weight = 32
[extra]
id = "let-the-provider-draw-the-episode-boundary"
source = ".ai/repo/use-cases/let-the-provider-draw-the-episode-boundary.md"
category = "continuity"
maturity = "described"
+++

## Situation

The session record is only as good as the discipline that opens it. A worker that is asked to run `session start` runs it when it remembers, which is not the sitting that ended in a crash, a compaction, or somebody closing the window — and those are the ones somebody later needs to read about. Meanwhile the layer names a store for the working context of an episode and nothing has ever written a file into it, so a reader cannot tell an empty store from an unimplemented one.

## Scenario

```yaml
setup: provider-reachable
given:
  - 'Majordomus installed, no session open, no provider hook wired'
steps:
  - id: unwired
    run: ['capture', 'status']
    note: 'a repository that wires nothing says so; it is never presented as an episode boundary that happens to be quiet'
    expect:
      exit: 0
      stdout_contains: ['claude-code:session +unconfigured']
  - id: install
    run: ['capture', 'install']
    note: 'one shim per event, and the entries that make the provider run them'
    expect:
      exit: 0
      stdout_contains: ['majordomus-session-start', 'majordomus-session-end']
      files_exist: ['.claude/hooks/majordomus-session-start', '.claude/hooks/majordomus-session-end']
  - id: proven-by-running-it
    run: ['capture', 'status']
    note: 'verified means a synthetic payload went through the end shim and reached the command, with the mutation left out'
    expect:
      exit: 0
      stdout_contains: ['claude-code:session +verified']
  - id: open
    run: ['session', 'start']
    note: 'the open freezes the context the builder resolved into the store the layer names'
    expect:
      exit: 0
      stdout_contains: ['working context: \.ai/local/session-contexts/']
  - id: where-it-is
    run: ['session', 'context']
    note: 'the path, not the document: local evidence is never poured into a terminal where a context can pick it up'
    expect:
      exit: 0
      stdout_contains: ['^\.ai/local/session-contexts/2[0-9]+T[0-9]+Z--s-']
  - id: close
    run: ['session', 'close']
    note: 'the shared record; the working context learns the outcome in the same document it was opened with'
    expect:
      exit: 0
      stdout_contains: ['^\.ai/repo/sessions/']
  - id: an-end-with-nothing-open
    run: ['session', 'close', '--if-none', 'ignore']
    note: 'what the provider''s end event passes: the episode may have been closed by hand, or never opened, and neither is a failure'
    expect:
      exit: 0
then:
  - 'the episode boundary exists without a worker choosing to draw it'
  - 'what the worker was told at the open is evidence rather than recollection'
  - 'the working context stays under the ignored half of the layer and never becomes a transcript'
```

## Outcome

The provider's own `SessionStart` and `SessionEnd` events open and close the episode. Both are idempotent, because the events are: a resume keeps the open episode and an end with nothing open writes nothing. The open freezes what the context builder resolved, next to a section for the worker's own notes, and the close appends the outcome and the record it wrote. `doctor` holds the repository to the wiring by driving a payload through the shim, and refuses a working context that carries a conversation.
