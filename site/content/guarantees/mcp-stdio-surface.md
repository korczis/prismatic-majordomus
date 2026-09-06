+++
title = "The Rust executable serves the repository's AI layer to an MCP client over stdio, read-only, with protocol frames alone on stdout"
description = "majordomus mcp, the one command of the Rust executable in apps/majordomus-cli/, is a process an MCP client spawns. It reads the .ai/ layer once, answers initialize, resources/list, resources/read, tools/list and tools/call over stdin and stdout, and ends when the client closes the pipe. Nothing but JSON-RPC frames is written to stdout; everything else goes to stderr. It writes no file, touches no git state and listens on no port."
weight = 96
[extra]
claim_id = "mcp-stdio-surface"
status = "guaranteed"
source = "docs/claims/mcp-stdio-surface.md"
+++
{% raw %}

## What it means

`majordomus mcp`, the one command of the Rust executable in `apps/majordomus-cli/`, is a process an MCP client spawns. It reads the `.ai/` layer once, answers `initialize`, `resources/list`, `resources/read`, `tools/list` and `tools/call` over stdin and stdout, and ends when the client closes the pipe. Nothing but JSON-RPC frames is written to stdout; everything else goes to stderr. It writes no file, touches no git state and listens on no port.

## How it works

`src/repository.rs` finds the root, `src/index.rs` builds the in-memory index, `src/mcp/surface.rs` projects it into resources and tools, `src/mcp/protocol.rs` speaks the protocol over that surface, and `src/mcp/stdio.rs` moves one frame per line. Logging goes through `tracing` to stderr under `MAJORDOMUS_LOG`. There is no MCP library and no async runtime: the read-only subset a server needs is small enough to keep in one file behind the index.

## How to see it

```bash
cargo build --manifest-path apps/majordomus-cli/Cargo.toml
printf '%s\n' '{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2025-06-18","capabilities":{},"clientInfo":{"name":"you","version":"0"}}}' \
              '{"jsonrpc":"2.0","id":2,"method":"resources/read","params":{"uri":"majordomus://prompt/continue"}}' \
  | apps/majordomus-cli/target/debug/majordomus mcp 2>/dev/null
```

Two lines come back, both JSON; the second carries `.ai/repo/prompts/continue.md` as read. Add `MAJORDOMUS_LOG=debug` and the diagnostics appear on stderr while stdout stays the same two lines.

## What it does not cover

One transport, stdio; no subscriptions, no list-change notifications, no prompts capability (the repository's prompt assets render `{{CONTEXT}}` from checkout-local state the shell tool owns, so they are served as resources, unrendered and labelled so). Interoperability is proven against real protocol frames, not against every client.

## Why it exists

Tools that speak MCP cannot read a shell tool. The layer is the repository's, readable without any tool, and this is the reader for clients that read through a protocol, kept subordinate to the shell tool that validates and enforces the layer. `test/cases/72_rust_mcp.sh` speaks to the built binary inside a repository the shell tool's own `init` wrote; `apps/majordomus-cli/tests/mcp_stdio.rs` holds every frame to the rules above, including that a session mutates nothing.
{% endraw %}
