---
schema: area/v1
id: context
kind: area
title: Context and continuity
summary: 'What a worker knows when it starts, and what survives when it stops.'
status: stable
weight: 10
tags: [context, continuity, memory]
---

# Context and continuity

What the next worker — machine or person — knows at the moment it begins, and what is left
behind when the current one ends. Everything here is a question about durable state:
whether the repository can say what matters, or whether the only copy is in a conversation
that is about to be discarded.

Not in this area: what a worker does with the context once it has it, which is
[implementation](/why/areas/verification/), and whether the rules it loaded are the right
ones, which is governance.
