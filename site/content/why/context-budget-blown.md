+++
title = "The instruction file that grew into a rulebook"
description = "Everything important gets appended to the always-loaded file, so every session pays for every rule and reads none of them carefully."
weight = 120
[extra]
id = "context-budget-blown"
status = "stable"
source = ".ai/repo/why/moments/context-budget-blown.md"
+++
{% raw %}

## The moment

The file every session loads before it does anything is now longer than most of the source
files it describes. It is the first thing every worker reads and the last thing anybody
edits deliberately.

## Why it happens

Appending is the only safe-feeling operation. A rule that matters gets added to the file
everyone loads, because that is the only way to be sure it is seen; nothing is ever removed,
because removal looks like weakening a control. The file therefore grows monotonically and
its signal-to-noise ratio falls monotonically with it.

## Why a better model does not fix it

A larger context window makes the file cheaper to load and no more likely to be applied. A
rule buried at line 800 among 400 others competes for attention with everything around it,
and the competition is decided by salience rather than by relevance to the current path.

## What it costs

Tokens on every session, which is measurable, and attention on every session, which is not.
The second cost is the real one: past a certain length, an instruction file stops being a
contract and becomes background texture.

## What Majordomus does

The bootstrap is generated, not written, and it is small on purpose: it says how to find the
policy and the rules, never what they are. The policy sets a hard line budget for the
always-loaded projection and a separate budget for what `majordomus context` prints; both
are failing checks, so growth is caught at the moment it happens rather than at the audit.

Everything else lives where it is governed, in the layer's scoped context documents, and is
composed for the path a worker is about to touch. `majordomus context` assembles within its
budget and names every section it dropped, so a truncated briefing is visible rather than
silent.

## Before and after

```text
before   CLAUDE.md   1,100 lines, loaded by every session

after    $ majordomus doctor
         OK budget  AGENTS.md — 46 lines, budget 150
         OK context builder — 18 lines, budget 300
```

## How to verify it

Append lines to the always-loaded projection past the budget and run `doctor`: the budget
check fails with the line count and the limit. `majordomus context` prints its own size and
what it omitted.

## What it does not do

It does not decide which rules matter, and it does not summarise a rule to make it fit. A
rule that does not fit the budget belongs in a scoped document, not in a shorter paraphrase.
{% endraw %}
