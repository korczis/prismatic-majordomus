---
id: open-and-close-a-session
kind: use-case
title: 'Record what one worker did in one sitting'
summary: 'Open a session, do the work under whatever tasks it touches, and close it into an envelope that references what it produced and copies nothing.'
category: continuity
status: active
target: guaranteed
actors: [agent, operator]
difficulty: basic
commands: [session, history]
doctrines: [majordomus.ledger-integrity]
claims: [ledger-integrity, event-vocabulary, history-ledger-read]
responsibilities: [state, watch]
applications: [several-agents-one-repository, long-running-work]
scenario:
  setup: installed
  given:
    - 'Majordomus installed, no session open'
  steps:
    - id: open
      run: ['session', 'start', '--worker', 'some-provider/some-model']
      note: 'one execution episode of one worker begins'
      expect:
        exit: 0
        stdout_contains: ['^session s-[0-9]+-[0-9a-f]+ opened at']
    - id: refuse-a-second
      run: ['session', 'start']
      note: 'one session per checkout at a time'
      expect:
        exit: 15
        stdout_contains: ['is open here']
    - id: close
      run: ['session', 'close']
      note: 'the envelope: identity, a temporal boundary, references to what the episode produced'
      expect:
        exit: 0
        stdout_contains: ['^\.ai/local/state/sessions/']
    - id: ledger
      run: ['history']
      note: 'the ledger carries both ends of the episode'
      expect:
        exit: 0
        stdout_contains: ['session.started', 'session.closed']
  then:
    - 'a closed session is an envelope, never a transcript'
    - 'which work happened together, and in what order, is answerable'
---

# Situation

A worker touches two tasks in one sitting and makes two decisions an hour apart. Nothing records that one worker made both, so later they look unrelated, and the only witness is a conversation nobody can search.

# Outcome

The session is a record with a start and an end; while it is open it is the one execution episode of this checkout, and once closed it references the tasks, records and decisions it produced and copies none of them.
