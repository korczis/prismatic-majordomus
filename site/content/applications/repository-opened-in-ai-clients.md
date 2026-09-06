+++
title = "A repository opened in AI clients, not only by people"
description = "Claude Code, Codex, Gemini CLI or another MCP client is a regular reader of the repository, and what it reads should be the layer, served, rather than files it happens to find."
weight = 5
[extra]
id = "repository-opened-in-ai-clients"
source = ".ai/repo/applications/repository-opened-in-ai-clients.md"
+++

## Context

The repository already has a .ai/ layer. The clients that read it differ in how they load nested instruction files, and each one reading by hand means each one reading differently. Serving the layer over MCP from one process makes the contract the same for all of them, and makes the clients visible to each other.
