---
id: find-an-object-without-reading-everything
kind: use-case
title: 'Find an object of the layer without reading everything'
summary: 'Search durable records literally across kinds from the shell, and ask the shared server the same question over MCP with majordomus_search and majordomus_get.'
category: mcp
status: active
target: guaranteed
weight: 82
actors: [agent]
difficulty: basic
commands: [search, knowledge, prompt]
mcp_tools: [majordomus_search, majordomus_get, majordomus_list]
doctrines: [majordomus.ai-layout-integrity, majordomus.prompt-integrity]
claims: [record-search, mcp-stdio-surface, mcp-uri-resolution, prompt-assets]
responsibilities: [layer, state]
applications: [repository-opened-in-ai-clients]
scenario:
  setup: active-task-records
  given:
    - 'an active task that has already produced a checkpoint, a decision and an open question'
  steps:
    - id: search
      run: ['search', 'parser']
      note: 'durable records matched literally, across kinds, without an index'
      expect:
        exit: 0
        stdout_contains: ['^decision ', 'parser', 'match']
    - id: nodes
      run: ['knowledge', 'nodes', '--kind', 'rule']
      note: 'one node per canonical object of one kind, with its identity'
      expect:
        exit: 0
        stdout_contains: ['^rule ']
    - id: prompts
      run: ['prompt', 'list']
      note: 'the repository’s own prompt assets, the ones an MCP client can also read as resources'
      expect:
        exit: 0
        stdout_contains: ['^continue ', '^debug ']
  then:
    - 'the shell and the MCP server answer from the same files'
    - 'a majordomus:// URI resolves the same way through the resource read, the tool and the HTTP route'
    - 'no index has to exist for the literal search to work'
---

# Situation

A client wants the rule about scope, or the decision about the parser, and has the whole layer in front of it. Reading every file to find one is what makes agents slow and expensive; asking a person is what makes them wrong.

# What you run

- `search <text>`: durable records that contain the text, across kinds
- `knowledge nodes --kind <k>`: the canonical objects of one kind, with identity
- `prompt list`: the prompt assets, also served as resources
- over MCP: `majordomus_search`, `majordomus_get` by `majordomus://` URI, `majordomus_list` by kind

# Outcome

One question, one answer, from the shell or from the server, without reading the layer end to end. The URI a client gets back resolves the same way everywhere.
