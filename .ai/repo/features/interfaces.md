---
schema: feature/v1
id: interfaces
kind: feature
title: One registry over the command line, HTTP, OpenAPI, Swagger UI and MCP
short_title: Interfaces
headline: Every agent, script and person reads the same repository through the interface it already speaks, and none of the interfaces is maintained by hand.
summary: The Rust executable serves the layer over stdio MCP, MCP over HTTP, routes under /api/v1/, an OpenAPI document with a Swagger UI, and a command line, all derived from the capability registry, which names what each capability changes; one shared server per repository, and every attached client is a peer the others can see.
status: stable
weight: 20
domain: surfaces
featured: false
areas: [documentation, observability, coordination]
modules: [repository, objects, web, environment, executions, commands, editors, entity, mcp]
rules: [project.web-surface-declared-once, project.web-surface-topology, project.native-cli-documented, project.shared-server-resilience]
docs: [docs/MCP.md, docs/WEB.md, docs/CAPABILITIES.md]
adrs: [adr-0001, adr-0003, adr-0013]
claims: [mcp-stdio-surface, mcp-data-driven, mcp-shared-server, mcp-lease-resilience, mcp-peers, mcp-client-autostart, mcp-uri-resolution, openapi-inferred, web-surface-declared-once, web-namespaces-reserved, cli-documentation-executable]
use_cases: [serve-the-layer-to-ai-clients, see-what-the-repository-holds-without-reading-it, find-an-object-without-reading-everything]
cockpit: [api]
web: [api, openapi, swagger, mcp, docs, rustdoc]
related: [declare-once, coordination]
tags: [mcp, http, openapi, cli]
---

## What it does

The first `majordomus mcp` in a repository binds one shared server on the loopback
interface and logs every surface it serves: the home page, this documentation, the crate's
rustdoc, the Cockpit, the Swagger UI, the OpenAPI document, the capability routes and MCP
over HTTP. Every later client attaches to it instead of starting another, and the server
ends when its last client leaves. The client configurations at the repository root — one
per provider that reads one, as `docs/generated/providers.md` lists them — name one
launcher, so opening the repository in any of them is enough.

What the server answers is the registry: a tool, a resource, a route or a command exists
because a declaration exists, and the same declaration is what the reference and the
website render. A surface — the documentation mount, the crate's rustdoc, a generated
report, the Swagger UI — is discovered from the thing that produces it, or from the source
that says it exists, and resolved once into a topology the router, the home page, the
publication and the validator all read.

## What it does not do

Most of what is served reads. Every capability carries its effect in the registry: a read,
a change to the answering runtime's own state (the peer board, the mesh journal, the
evidence ledger), or a change to the repository. The ones that write the repository are
`continuity.publish`, `continuity.resume`, `continuity.sync`, `executions.start`,
`intents.propose`, `plan.transition`, `reasoning.record`, `recover.orphans` and
`worktree.create`;
the `initialize` instructions every client reads name them from the registry, and case
3363 refuses this page when its list and the registry's differ, because a sentence that
promises a read-only surface is exactly the sentence an agent acts on. There is no
authentication and no remote binding by default; the server is for the clients on this
machine. When it is bound beyond loopback, its reads answer whoever reaches it, and every
capability that changes something is refused to a remote caller unless a linked node
signed the input. It does not run a model and it does not route work to one.
