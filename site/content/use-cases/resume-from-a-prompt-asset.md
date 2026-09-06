+++
title = "Start a session from a rendered framing, not a pasted transcript"
description = "List the prompt assets the repository ships, render one against the durable state, and read the context it embeds."
weight = 37
[extra]
id = "resume-from-a-prompt-asset"
source = ".ai/repo/use-cases/resume-from-a-prompt-asset.md"
category = "continuity"
maturity = "described"
+++

## Situation

Every new session starts with the same paragraph somebody types from memory, and the repository state it describes is whatever that person last remembered.

## Scenario

```yaml
setup: installed
given:
  - 'Majordomus installed; the skeleton ships its prompt assets'
steps:
  - id: which
    run: ['prompt', 'list']
    note: 'the small set of versioned framings; nothing ranks them'
    expect:
      exit: 0
      stdout_contains: ['continue', 'handover', 'review']
  - id: refuse-unknown
    run: ['prompt', 'show', 'nosuch']
    note: 'an asset that does not exist is named with the command that lists them'
    expect:
      exit: 12
      stdout_contains: ['no prompt asset', 'prompt list']
  - id: the-context
    run: ['context']
    note: 'what a prompt embeds: durable state in authority order, within the budget'
    expect:
      exit: 0
      stdout_contains: ['^## GIT', '^## BUDGET']
then:
  - 'a prompt is rendered against a closed set of state tokens'
  - 'the context inside it is the same one `context` prints'
```

## Outcome

The framing is a versioned file under `.ai/repo/prompts/`; rendering it embeds the assembled context, so the session starts from durable state and a budget, not from a paste.
