+++
title = "Decide what belongs in the AI context, as data"
description = "Read the repository’s declared scope policy, see which paths are in and out of the index, and let doctor and the knowledge index apply it rather than a hardcoded list."
weight = 22
[extra]
id = "classify-what-belongs-in-the-context"
source = ".ai/repo/use-cases/classify-what-belongs-in-the-context.md"
category = "knowledge"
maturity = "guaranteed"
+++

## Situation

An AI client indexes what it finds: build outputs, the local state directory, a vendored binary, a data dump. The context fills with noise and, sometimes, with a secret. Every tool has its own ignore list, and none of them is the repository’s own statement of what matters.

## What you run

- `doctor`: the scope policy exists and parses
- `knowledge sources`, `knowledge nodes --scope shared`: what the index discovers once the policy is applied
- over MCP: `majordomus_scope` reads the policy, `majordomus_scope_classify` answers for one path

## Outcome

What belongs in the AI context is declared once in the layer, checked, and applied by the shell tool and the server alike. A client that honours it indexes the repository, not the machine.
