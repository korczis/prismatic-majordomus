+++
title = "One registry over the command line, HTTP, OpenAPI, Swagger UI and MCP"
description = "The Rust executable serves the layer read-only over stdio MCP, MCP over HTTP, routes under /api/v1/, an OpenAPI document with a Swagger UI, and a command line, all derived from the capability registry; one shared server per repository, and every attached client is a peer the others can see."
weight = 20
[extra]
id = "interfaces"
status = "stable"
source = ".ai/repo/features/interfaces.md"
+++
{% raw %}

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

Almost nothing here writes to the repository: all but two of the capabilities over MCP and
HTTP are reads, and most of the commands that change anything change only this process's
memory. The exceptions are named rather than implied — `plan.transition` and
`recover.orphans` — and the `initialize` instructions every client reads name them too,
counted off the registry rather than stated by hand, because a sentence that promises a
read-only surface is exactly the sentence an agent acts on. There is no
authentication and no remote binding by default; the server is for the clients on this
machine. It does not run a model and it does not route work to one.
{% endraw %}
