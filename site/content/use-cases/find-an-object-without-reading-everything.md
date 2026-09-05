+++
title = "Find an object of the layer without reading everything"
description = "Search durable records literally across kinds from the shell, and ask the shared server the same question over MCP with majordomus_search and majordomus_get."
weight = 23
[extra]
id = "find-an-object-without-reading-everything"
source = ".ai/repo/use-cases/find-an-object-without-reading-everything.md"
category = "mcp"
maturity = "guaranteed"
+++

## Situation

A client wants the rule about scope, or the decision about the parser, and has the whole layer in front of it. Reading every file to find one is what makes agents slow and expensive; asking a person is what makes them wrong.

## What you run

- `search <text>`: durable records that contain the text, across kinds
- `knowledge nodes --kind <k>`: the canonical objects of one kind, with identity
- `prompt list`: the prompt assets, also served as resources
- over MCP: `majordomus_search`, `majordomus_get` by `majordomus://` URI, `majordomus_list` by kind

## Outcome

One question, one answer, from the shell or from the server, without reading the layer end to end. The URI a client gets back resolves the same way everywhere.
