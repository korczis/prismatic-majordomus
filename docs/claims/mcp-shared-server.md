# One shared server serves a repository: the first majordomus mcp binds loopback HTTP with Swagger UI and MCP over HTTP beside its stdio session and logs the URL, every later one attaches to it, and it ends when the last client leaves

## What it means

Open the repository in one MCP client and `majordomus mcp` is the server: its stdio serves that client, and beside it a loopback socket serves Swagger UI at `/docs`, the OpenAPI document, every capability route and MCP over HTTP at `/mcp`; the URL is in the log the moment it is bound. Open the repository in a second client and its `majordomus mcp` does not start another server: it finds the first through the lease, checks that it answers for this root, and forwards its client's frames to it. Close the clients in any order and the server lingers exactly as long as one is attached; when the last leaves it closes the port and removes the lease. Kill the server and a bridged client takes its place, or attaches to whichever process took the lease first, and its own client never re-initialises. `--standalone` serves one client alone with no port and no lease.

## How it works

`src/lease.rs` creates `.ai/local/state/mcp/server.json` atomically (the process that wins is the server), publishes the URL into it once the port is bound, probes a URL it finds against `GET /` and the repository root, and takes over a lease whose server does not answer. `src/shared.rs` binds (`--http-port`, default 8741, a taken port replaced by a free one), starts the worker threads over the immutable registry, and waits for the attached sessions to leave before stopping. `src/http/mcp.rs` is MCP over HTTP: `initialize` answers with an `Mcp-Session-Id`, every later request carries it, `DELETE` ends the session, and an idle one expires. `src/mcp/bridge.rs` is the other side, one HTTP request per stdio message and a ping every twenty seconds; `src/commands/mcp.rs` holds the session that is either answered locally or bridged, and elects again when the bridge loses its server.

## How to see it

```bash
just build
apps/majordomus-cli/target/debug/majordomus mcp < /dev/null      # the log: shared server listening on http://127.0.0.1:8741 (swagger ui .../docs, ...)
# in one terminal, keep a client attached:
mkfifo /tmp/in; apps/majordomus-cli/target/debug/majordomus mcp < /tmp/in & exec 3>/tmp/in
cat .ai/local/state/mcp/server.json                              # the lease: url, root, pid
# in another: a second client bridges instead of binding
printf '{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2025-06-18","capabilities":{},"clientInfo":{"name":"second","version":"0"}}}\n' | apps/majordomus-cli/target/debug/majordomus mcp
# stderr: a shared server for this repository is already running at http://127.0.0.1:8741 ...; bridging this stdio session to it
exec 3>&-                                                        # the first client goes: the server stops, the lease is gone
```

## What it does not cover

The server keeps the index it built at start; a client that attaches later sees the layer as it was then, and rediscovery is a restart (the server ends when the last client leaves). The server's `--discovery` and `--strict` apply to every session it serves. There is no server-initiated stream on `/mcp`, no notification, no subscription. The lease is per checkout: two worktrees have two servers.

## Why it exists

The operator asked for Swagger UI beside `majordomus mcp` by default, and for exactly one server per repository so that several clients (Claude, Codex, Gemini) share it and can coordinate. A daemon is refused by the design; a server that is a client's child and ends with the last client answers both asks. The decision and its answer to the "Intentionally Absent" list are `.ai/repo/adrs/0003-shared-mcp-server-peers-and-client-autostart.md`. `apps/majordomus-cli/tests/mcp_shared.rs` spawns the processes and speaks to them over real pipes and sockets, kills the server and watches the bridge take over; `test/cases/90_mcp_shared_server.sh` runs two clients through `bin/majordomus-mcp` in a repository the shell tool's `init` wrote.
