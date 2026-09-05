+++
title = "See which rules apply here and what enforces them"
description = "List the effective rule set, read one rule as the tool reads it, and see which of them the tool enforces and from where."
weight = 32
[extra]
id = "read-the-rules-the-tool-applies"
source = ".ai/repo/use-cases/read-the-rules-the-tool-applies.md"
category = "policy"
maturity = "guaranteed"
+++

## Situation

A reviewer asks which rules hold in this repository and whether anything actually checks them. The answer is scattered across a wiki, a CI file and somebody's memory.

## Outcome

`rules list` is the effective set, `rules show` is one rule as the tool reads it, and `doctrine list` is the subset the tool enforces with the command that runs each one.
