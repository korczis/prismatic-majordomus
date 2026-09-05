---
id: repository-opened-in-ai-clients
kind: application
title: 'A repository opened in AI clients, not only by people'
summary: 'Claude Code, Codex, Gemini CLI or another MCP client is a regular reader of the repository, and what it reads should be the layer, served, rather than files it happens to find.'
weight: 5
status: active
fits_when:
  - 'More than one AI client opens the repository, in one checkout or across sessions, and they should read one layer'
  - "The layer's objects are worth exposing as resources and tools rather than as files a client may or may not open"
  - 'You want a client to be able to say what it is working on and see what the others said'
does_not_fit_when:
  - 'No AI client ever opens the repository; the shell tool alone supervises the work'
  - 'You want the server to route work or refuse writes outside an announced scope; announcements are informational and the task record is the enforced form'
use_cases: [serve-the-layer-to-ai-clients, extend-what-the-executable-serves]
doctrines: [majordomus.ai-layout-integrity, majordomus.projection-integrity]
responsibilities: [layer, projection]
---

# Context

The repository already has a .ai/ layer. The clients that read it differ in how they load nested instruction files, and each one reading by hand means each one reading differently. Serving the layer over MCP from one process makes the contract the same for all of them, and makes the clients visible to each other.
