+++
title = "Keep the prompts that started the work, below the model rather than around it"
description = "Wire the provider hook, prove it by running a payload through it, and see that a prompt the tool could not parse is reported rather than silently dropped."
weight = 25
[extra]
id = "capture-the-prompts-that-started-the-work"
source = ".ai/repo/use-cases/capture-the-prompts-that-started-the-work.md"
category = "continuity"
maturity = "guaranteed"
+++

## Situation

The prompt that started the work is the one thing nobody keeps. It lives in a provider transcript that the next session cannot read, so six weeks later the repository holds the change and no record of what was asked for.

## Outcome

The prompts are captured below the model, by the provider hook, into files the repository ignores and nothing loads into a context. Whether the wiring is real is answered by running a payload through it rather than by a claim, and a payload that cannot be read is reported instead of dropped.

Each prompt is kept twice under one stem, because the two readers are different. The `.json` record is what the provider sent — its own spans, still escaped, pretty printed one member per line — and it is what everything else is derived from. The `.md` beside it is that record rendered for a person: the fields as front matter and again as a table, then the prompt itself under `## PROMPT`, decoded and fenced. An archive nobody opens is evidence of nothing, and a rendering nothing can be rebuilt from is not evidence at all, so the pair is enforced rather than left to habit. The asymmetry is what makes that safe: `capture render` rebuilds any rendering from its record, and no command can go the other way.
