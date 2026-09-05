---
id: resume-from-a-prompt-asset
kind: use-case
title: 'Start a session from a rendered framing, not a pasted transcript'
summary: 'List the prompt assets the repository ships, render one against the durable state, and read the context it embeds.'
category: continuity
status: active
target: guaranteed
actors: [agent]
difficulty: basic
commands: [prompt, context]
doctrines: [majordomus.prompt-integrity, majordomus.context-budget]
claims: [prompt-asset, prompt-assets, context-assembly, minimum-context]
responsibilities: [policy, profiles]
applications: [long-running-work, several-agents-one-repository]
scenario:
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
---

# Situation

Every new session starts with the same paragraph somebody types from memory, and the repository state it describes is whatever that person last remembered.

# Outcome

The framing is a versioned file under `.ai/repo/prompts/`; rendering it embeds the assembled context, so the session starts from durable state and a budget, not from a paste.
