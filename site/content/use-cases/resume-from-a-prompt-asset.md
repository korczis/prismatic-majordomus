+++
title = "Start a session from a rendered framing, not a pasted transcript"
description = "List the prompt assets the repository ships, render one against the durable state, and read the context it embeds."
weight = 34
[extra]
id = "resume-from-a-prompt-asset"
source = ".ai/repo/use-cases/resume-from-a-prompt-asset.md"
category = "continuity"
maturity = "verified"
+++

## Situation

Every new session starts with the same paragraph somebody types from memory, and the repository state it describes is whatever that person last remembered.

## Outcome

The framing is a versioned file under `.ai/repo/prompts/`; rendering it embeds the assembled context, so the session starts from durable state and a budget, not from a paste.
