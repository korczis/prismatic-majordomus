+++
title = "Trace a change to the context documents it affects"
description = "List the scoped context documents, resolve which apply to a path and why, and find out which documents a change set touches before it lands."
weight = 19
[extra]
id = "trace-a-change-to-the-context-it-affects"
source = ".ai/repo/use-cases/trace-a-change-to-the-context-it-affects.md"
category = "knowledge"
maturity = "guaranteed"
+++

## Situation

Context is not one file. A directory carries its own document, the root carries another, and a change deep in the tree may fall under a document nobody remembers writing. When a document changes, which paths and which projections does it affect? Guessing is how a stale rule survives a review.

## What you run

- `context list`: every scoped document with its scope and composition
- `context resolve <path>` and `context explain <path>`: what applies to a path, in order, and why
- `context affected`: the documents a change set touches, from git
- `context check-sync`: validate the tree, then the projections against their stamps, then the affected set

## Outcome

Every path has one deterministic answer to “what governs me”, every change has one answer to “what did I touch”, and a projection rendered from a document that has since changed is reported, not trusted.
