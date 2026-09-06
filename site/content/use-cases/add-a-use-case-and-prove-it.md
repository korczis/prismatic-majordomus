+++
title = "Add a use case and let the tool prove it"
description = "Scaffold a draft for an uncovered command, validate it, run its scenario against the real tool, and see the coverage move."
weight = 24
[extra]
id = "add-a-use-case-and-prove-it"
source = ".ai/repo/use-cases/add-a-use-case-and-prove-it.md"
category = "extension"
maturity = "guaranteed"
+++

## Situation

A command gained a behaviour and nobody wrote down what a person does with it. The documentation will drift the day it is written by hand, and a paragraph proves nothing.

## Outcome

`usecase scaffold` writes a draft with the facts the tool already has, `usecase run` executes its scenario against the real tool and records the evidence, and `usecase coverage` says what is still missing. The draft counts for nothing until it is made active.
