---
id: serve-the-layer-to-ai-clients
kind: use-case
title: "Serve the repository's AI layer to every AI client through one shared server"
summary: 'Open the repository in Claude Code, Codex or Gemini CLI and have each of them read the same rules, prompts and knowledge over MCP from one process, seeing each other.'
category: mcp
status: active
target: guaranteed
weight: 7
actors: [operator, agent]
difficulty: basic
commands: [init, doctor]
mcp_tools: [majordomus_repository, majordomus_list, majordomus_get, majordomus_peers, majordomus_announce]
doctrines: [majordomus.ai-layout-integrity, majordomus.projection-integrity, majordomus.scope-integrity]
claims: [mcp-client-autostart, mcp-shared-server, mcp-peers, mcp-lease-resilience, mcp-stdio-surface, mcp-uri-resolution, mcp-degraded-not-silent]
responsibilities: [layer, scope, doctor]
applications: [repository-opened-in-ai-clients, several-agents-one-repository]
scenario:
  setup: installed-wired
  given:
    - 'a repository with the layer installed and its projections generated'
  steps:
    - id: nothing-to-add
      run: ['init', '--extend']
      note: 'the layer is complete; the client configurations at the root start the server'
      expect:
        exit: 0
        stdout_contains: ['nothing to add']
    - id: healthy
      run: ['doctor']
      note: 'what the server will serve is what doctor proved'
      expect:
        exit: 0
        stdout_contains: ['doctor: 0 failure']
  then:
    - 'an MCP client opened here starts the shared server through bin/majordomus-mcp and reads the same layer'
---

# Situation

Three AI clients are open in one checkout. Each reads .ai/ by hand, none knows the others exist, and the first thing two of them do is edit the same file. The layer is the contract, but nothing serves it and nothing shows who is working where.

# What you run

- `init`: writes the .ai/ layer the executable serves; the client configurations at the root (.mcp.json, .gemini/settings.json, .codex/config.toml) start bin/majordomus-mcp, which builds the Rust executable when it must
- `doctor`: proves the layer and its projections are consistent before a client reads them, and that the MCP client autostart is wired

# Outcome

The first client to open the repository is the shared server; every later one attaches to it. Each client sees the same objects as majordomus:// resources and the same tools, lists the other clients with majordomus_peers, and announces its intent and the paths it will touch with majordomus_announce. A lease a client leaves behind never locks the others out, and a client that cannot serve says so instead of serving a degraded layer silently.
