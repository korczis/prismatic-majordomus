+++
title = "Serve the repository's AI layer to every AI client through one shared server"
description = "Open the repository in Claude Code, Codex or Gemini CLI and have each of them read the same rules, prompts and knowledge over MCP from one process, seeing each other."
weight = 7
[extra]
id = "serve-the-layer-to-ai-clients"
source = ".ai/repo/use-cases/serve-the-layer-to-ai-clients.md"
category = "mcp"
maturity = "guaranteed"
+++

## Situation

Three AI clients are open in one checkout. Each reads .ai/ by hand, none knows the others exist, and the first thing two of them do is edit the same file. The layer is the contract, but nothing serves it and nothing shows who is working where.

## What you run

- `init`: writes the .ai/ layer the executable serves; the client configurations at the root (.mcp.json, .gemini/settings.json, .codex/config.toml) start bin/majordomus-mcp, which builds the Rust executable when it must
- `doctor`: proves the layer and its projections are consistent before a client reads them, and that the MCP client autostart is wired

## Outcome

The first client to open the repository is the shared server; every later one attaches to it. Each client sees the same objects as majordomus:// resources and the same tools, lists the other clients with majordomus_peers, and announces its intent and the paths it will touch with majordomus_announce. A lease a client leaves behind never locks the others out, and a client that cannot serve says so instead of serving a degraded layer silently.
