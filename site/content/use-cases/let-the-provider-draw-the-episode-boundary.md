+++
title = "Have the session opened and closed without anybody remembering to"
description = "Wire the provider''s own session events, and the episode opens when the sitting begins, hands the worker what the last one left, records what a compaction is about to discard, and closes with a continuation record beside its envelope."
weight = 38
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
  - 'work about to begin, which is what the compaction and end events have to describe'
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
      stdout_contains: ['majordomus-session-start', 'majordomus-session-end', 'majordomus-session-compact']
      files_exist: ['.claude/hooks/majordomus-session-start', '.claude/hooks/majordomus-session-end', '.claude/hooks/majordomus-session-compact']
  - id: proven-by-running-it
    run: ['capture', 'status']
    note: 'verified means a synthetic payload went through the end shim and reached the command, with the mutation left out'
    expect:
      exit: 0
      stdout_contains: ['claude-code:session +verified']
  - id: open
    run: ['session', 'start']
    note: 'the open freezes the context the builder resolved into the store the layer names — as a snapshot of what this worker was told, not as its working context'
    expect:
      exit: 0
      stdout_contains: ['working context: \.ai/local/session-contexts/']
  - id: what-is-true-now
    run: ['session', 'context']
    note: 'the document, composed on this read: the episode, git now against git at the open, and what the episode has recorded since. It used to answer with the snapshot path, which is a file written before the worker had done anything'
    expect:
      exit: 0
      stdout_contains: ['^# Working context of session s-', '## Repository now', '## Recorded in this episode']
  - id: where-the-snapshot-is
    run: ['session', 'context', '--path']
    note: 'the frozen opening snapshot is still addressable by name, for a caller that wants the file rather than the answer'
    expect:
      exit: 0
      stdout_contains: ['^\.ai/local/session-contexts/2[0-9]+T[0-9]+Z--s-']
  - id: work-to-describe
    run: ['start', 'Prove the lifecycle runs itself', '--scope', 'lib']
    note: 'a checkpoint is a progress note inside a task, so the two events below have something to be about'
    expect:
      exit: 0
  - id: what-a-compaction-would-record
    run: ['checkpoint', '--derive']
    note: 'what the compaction event runs: a progress note composed from git and the ledger, because at that moment nobody is being asked anything'
    expect:
      exit: 0
      stdout_contains: ['^\.ai/local/state/checkpoints/']
  - id: what-an-end-would-write
    run: ['handover', '--derive']
    note: 'and what the end event writes when the task is still active: the sections the policy requires, derived from records that already exist, with no model called'
    expect:
      exit: 0
      stdout_contains: ['^\.ai/local/state/handovers/']
  - id: what-the-next-worker-is-handed
    run: ['handover', '--resolve']
    note: 'the record the start event quotes into the next episode, with the label that says how far to trust it'
    expect:
      exit: 0
      stdout_contains: ['Match: same_worktree_same_branch', 'Git state: exact']
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
  - 'the continuation record exists whether or not anybody was willing to type one'
  - 'the next episode is handed that record, with the label that says how far to trust it'
  - 'the working context stays under the ignored half of the layer and never becomes a transcript'
```

## Outcome

The provider's own `SessionStart`, `PreCompact` and `SessionEnd` events run the lifecycle. Each is idempotent, because the events are: a resume keeps the open episode, an end with nothing open writes nothing, and a compaction with no active task records nothing.

The open freezes what the context builder resolved, next to a section for the worker's own notes, and writes a briefing to standard output — which the provider adds to the context it is about to build. That is the step that makes the record readable as well as written: a continuation package nothing loads is one nobody reads. A compaction records a derived checkpoint, because the conversation is about to stop holding what it knows. An end with the task still active writes a derived handover before the envelope closes, so the next worker inherits both an index of the episode and something to act on.

`doctor` holds the repository to the wiring by driving a payload through the shim, and refuses a working context that carries a conversation. `majordomus_continuity` reads the same state back over MCP, over HTTP and in the Cockpit, with the same two tiers and the same four divergence labels — and never writes, because the lifecycle has one writer.
