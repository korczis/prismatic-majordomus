# Every client attached to the shared server is a peer named by its own initialize, listed by majordomus_peers, and majordomus_announce tells the others what it is working on and which paths it expects to touch

## What it means

When two or three AI clients are open in one checkout, each of them can ask the shared server who else is there. A peer is a session: the server's own stdio client, every bridged `majordomus mcp`, and every client speaking `/mcp` directly. It is named by what its client sent in `initialize` (`clientInfo.name` and `version`), numbered `p1`, `p2`, ... in attachment order, with its transport, when it attached and when it was last seen. A client can announce one line of intent and the repository-relative paths it expects to touch; the others see it in `majordomus_peers` and in the `instructions` they get from `initialize`, which name the server's URL, the caller's own id and every other peer before the first tool call.

## How it works

`src/peers.rs` is the board: attach, identify, touch, announce, detach, list, in one process's memory. `src/capability/builtin.rs` declares `peers.list` (a query) and `peers.announce` (the one capability of kind `command`, because it changes that memory) as typed descriptors with explicit exposures, and the registry projects them like every other: tools `majordomus_peers` and `majordomus_announce`, `GET /api/v1/peers`, `POST /api/v1/peers/announce`, the OpenAPI operations, the reference. A handler learns who is calling through the context's `caller`, set per session by `Surface::for_peer`; over plain HTTP there is no caller, so `POST /api/v1/peers/announce` is refused and `GET /api/v1/peers` answers without one. The kind decides the rest: a command is bound to `POST` and announced to MCP clients as not read-only, both checked by the registry.

## How to see it

```bash
just build
# with a server running (an MCP client open, or `just serve` in another terminal):
curl -s http://127.0.0.1:8741/api/v1/peers | jq '.peers[] | {id, client, transport, announcement}'
# through MCP, in a client: call majordomus_announce with {"intent": "refactor the parser", "scope": ["lib/parse"]},
# then majordomus_peers from another client: the announcement is on the record of the first
apps/majordomus-cli/target/debug/majordomus capabilities describe peers.announce   # kind: command, http: POST /api/v1/peers/announce
```

## What it does not cover

Announcements are informational: nothing here refuses a write outside an announced scope. The durable, enforced form of "who works where" is the shell tool's task record and `start --scope` / `check`, per worktree; the board is the live, in-memory complement for clients sharing one checkout, and it is gone with the server. Nothing persists, nothing routes by it, and no role, tier or persona is attached to a peer.

## Why it exists

The operator asked that the shared server let the different clients coordinate out of the box. The smallest thing that does that is a list of who is attached with what they said they are doing, derived from the handshake every client already performs, exposed through the registry so that MCP, HTTP, OpenAPI and the reference agree. `apps/majordomus-cli/tests/mcp_shared.rs` has two clients announce and see each other over real pipes; `tests/shared_units.rs` covers the board; `test/cases/90_mcp_shared_server.sh` does it through `bin/majordomus-mcp` in a repository the shell tool's `init` wrote.
