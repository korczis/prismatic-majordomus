+++
title = "MCP, HTTP, OpenAPI, Swagger UI, the capabilities commands and the generated reference are derived from the registry, and a change to one definition reaches every one of them"
description = "No interface of the Rust executable declares anything of its own. The MCP tools and resources, the HTTP routes and the OpenAPI operations, the Swagger UI page, majordomus capabilities list and describe, and docs/generated/capabilities.md are built from the registry when asked. Change a capability's description or input type in its descriptor, or add a declarative object to the layer, and every interface shows the change on the next start; nothing else is edited."
weight = 100
[extra]
claim_id = "interfaces-are-projections"
status = "guaranteed"
source = "docs/claims/interfaces-are-projections.md"
+++
{% raw %}

## What it means

No interface of the Rust executable declares anything of its own. The MCP tools and resources, the HTTP routes and the OpenAPI operations, the Swagger UI page, `majordomus capabilities list` and `describe`, and `docs/generated/capabilities.md` are built from the registry when asked. Change a capability's description or input type in its descriptor, or add a declarative object to the layer, and every interface shows the change on the next start; nothing else is edited.

## How it works

`mcp/surface.rs` maps registry entries with an MCP exposure to tools and resources and calls the registry by canonical id; `http/router.rs` maps entries with an HTTP exposure to routes and binds the input from the query or the body; `http/openapi.rs` renders the same entries as an OpenAPI 3.1 document whose `operationId` is the canonical id; `http/swagger.rs` serves a page that loads `/openapi.json`; the `capabilities` commands dispatch through the registry's CLI exposure; `generate.rs` writes the reference from the same entries.

## How to see it

```bash
apps/majordomus-cli/target/debug/majordomus capabilities describe objects.get --format json   # exposure: mcp tool, http route
printf '{"jsonrpc":"2.0","id":1,"method":"tools/list"}\n' | apps/majordomus-cli/target/debug/majordomus mcp 2>/dev/null   # the same id under _meta
apps/majordomus-cli/target/debug/majordomus serve --port 0   # GET /openapi.json: operationId objects.get, x-majordomus-mcp majordomus_get
```

## What it does not cover

The command line's other commands (`schema`, `validate`, `mcp`, `serve`, `generate`) are views of the registry or servers of it, not capabilities; the clap tree is hand-written and only the introspection commands dispatch through the registry. Swagger UI's assets are the pinned distribution on unpkg; the page embeds no specification but does need the network for the viewer itself.

## Why it exists

The invariant the operator asked for, and the rule `project.interfaces-are-projections`: a capability defined once, every interface derived. `apps/majordomus-cli/tests/projections.rs` proves every declared projection present, none orphan, and a changed input type and description reaching the MCP schema, the OpenAPI operation and the reference; `tests/http_serve.rs` proves MCP and HTTP answer the same handler with the same JSON; `test/cases/76_capabilities_projections.sh` reads one capability back through every interface from a repository `init` wrote.
{% endraw %}
