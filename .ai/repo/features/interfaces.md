---
schema: feature/v1
id: interfaces
kind: feature
title: One registry over the command line, HTTP, OpenAPI, Swagger UI and MCP
short_title: Interfaces
headline: Every agent, script and person reads the same repository through the interface it already speaks, and none of the interfaces is maintained by hand.
summary: The Rust executable serves the layer read-only over stdio MCP, MCP over HTTP, routes under /api/v1/, an OpenAPI document with a Swagger UI, and a command line, all derived from the capability registry; one shared server per repository, and every attached client is a peer the others can see.
status: stable
weight: 20
featured: false
areas: [documentation, observability, coordination]
modules: [repository, objects, web]
rules: [project.web-surface-declared-once, project.web-surface-topology, project.native-cli-documented, project.shared-server-resilience]
docs: [docs/MCP.md, docs/WEB.md, docs/CAPABILITIES.md]
adrs: [adr-0001, adr-0003, adr-0013]
claims: [mcp-stdio-surface, mcp-data-driven, mcp-shared-server, mcp-lease-resilience, mcp-peers, mcp-client-autostart, mcp-uri-resolution, openapi-inferred, web-surface-declared-once, web-namespaces-reserved, cli-documentation-executable]
use_cases: [serve-the-layer-to-ai-clients, see-what-the-repository-holds-without-reading-it, find-an-object-without-reading-everything]
cockpit: [api]
web: [api, openapi, swagger, mcp, docs]
related: [declare-once, coordination]
tags: [mcp, http, openapi, cli]
---

## What it does

The first `majordomus mcp` in a repository binds one shared server on the loopback
interface and logs every surface it serves: the home page, this documentation, the Cockpit,
the Swagger UI, the OpenAPI document, the capability routes and MCP over HTTP. Every later
client attaches to it instead of starting another, and the server ends when its last client
leaves. The client configurations at the repository root — one per provider that reads
one, as `docs/generated/providers.md` lists them — name one launcher, so opening the
repository in any of them is enough.

What the server answers is the registry: a tool, a resource, a route or a command exists
because a declaration exists, and the same declaration is what the reference and the
website render. A surface — the documentation mount, a generated report, the Swagger UI —
is discovered from the thing that produces it and resolved once into a topology the router,
the home page, the publication and the validator all read.

## What it does not do

Nothing here writes to the repository: every capability over MCP and HTTP is a read, and
the one command that changes anything changes this process's memory. There is no
authentication and no remote binding by default; the server is for the clients on this
machine. It does not run a model and it does not route work to one.
