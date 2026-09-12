+++
title = "Close an episode whose client never came back"
description = "A killed provider sends no end event. Close the episode it left open by evidence, exactly once, without claiming work it did not do."
weight = 7
[extra]
id = "recover-an-episode-nobody-closed"
source = ".ai/repo/use-cases/recover-an-episode-nobody-closed.md"
category = "continuity"
maturity = "described"
+++

## Situation

A provider crashes, is killed, or loses its connection. The episode it opened is still in `state/sessions-open/`, because the end event that would have closed it never arrived. Nothing owns that file: `session start` with the same key is refused for ever, `continuity` reports an episode nobody is in, and the work the next worker actually does is attributed to no episode at all.

Deleting the file would be the obvious move and the wrong one. The episode is a record of a stretch of work, and the ledger lines it stamped are the only account of what that stretch contained.

## What you run

- `recover episodes --check`: the candidates, each with the last moment it is known to have been alive, and what would happen to it
- `recover episodes`: closes the ones whose evidence says the client is gone, and nothing else

Every step here reads the store through `recover` itself rather than through `session status`, and that is deliberate: `session status` prints the episode's start head, which is a different commit in every repository the demonstration runs in, so a scenario that asserted on it would make the published catalogue differ from the checkout that generated it.

## Scenario

```yaml
setup: session-stranded
given:
  - 'two episodes open here: one whose client is gone, and one this process is inside'
steps:
  - id: the-plan
    run: ['recover', 'episodes', '--check']
    note: 'every candidate is measured and the measurement is printed, including the ones nothing happens to'
    expect:
      exit: 0
      stdout_contains: ['last seen 2020-01-01T00:00:00Z', 'stranded:', 'live: this process is inside it', 'check: 1 action']
  - id: dry-run-wrote-nothing
    run: ['recover', 'episodes', '--check']
    note: 'the check is a dry run: the second one plans exactly what the first one did'
    expect:
      exit: 0
      stdout_contains: ['stranded:', 'check: 1 action']
  - id: recover
    run: ['recover', 'episodes']
    note: 'the stranded episode is closed into a record; the live one is untouched'
    expect:
      exit: 0
      stdout_contains: ['.ai/repo/sessions/', 'recovered: 1 action']
  - id: idempotent
    run: ['recover', 'episodes']
    note: 'a second run has nothing to do: one episode, one record'
    expect:
      exit: 0
      stdout_contains: ['nothing to recover']
  - id: still-live
    run: ['recover', 'episodes', '--check']
    note: 'the episode somebody is working in survived the sweep and is still listed as live'
    expect:
      exit: 0
      stdout_contains: ['live: this process is inside it', 'nothing to recover']
then:
  - 'an episode under the staleness threshold is never closed, and neither is the one this process resolves to'
  - 'an episode whose last sign of life cannot be read as a timestamp is skipped and counted, never closed'
  - 'the recovered record claims no commits and no changed files: the tree at recovery time belongs to whoever is working now'
  - 'the reason is a ledger event, session.recovered, naming the recovered episode rather than the recoverer'
```

## Outcome

The store holds one record per episode and no episode that nobody is in. The record says, in its own body, that it was written by recovery and on what evidence — so a reader months later can tell a closed episode from a recovered one without going to the ledger, and neither of them from a fabricated one, because recovery writes nothing it cannot prove.
