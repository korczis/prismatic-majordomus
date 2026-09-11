# A client with no provider hooks gets an execution episode from the MCP connection, resumes its own after a crash, and never has its prompt claimed to be captured

## What it means

Majordomus draws the episode boundary below the model, so that the episode which mattered — the one that ended in a crash, a compaction, or somebody closing the window — is recorded whether or not a model remembered to record it. Until ADR 0043 that was wired for one provider: Claude Code fires `SessionStart` and `SessionEnd`, a shim runs `capture session`, and the repository gets an episode. Every other client got nothing at all.

A client with no hooks still has a boundary, and the shared server is standing on it. The client speaks `initialize`, calls `majordomus_session_attach` with its own durable name for the sitting, works, and goes. The episode opens on the attach, every message it sends is that episode's heartbeat, losing the connection detaches rather than closes it, and a client that comes back under the same name is given *its own* episode again — under whatever peer id it now has. A deliberate `majordomus_session_detach` closes it into a session record; a client that simply disappears has its episode closed as interrupted once the reattach grace has passed, and the difference between those two records is the one thing about an ended episode that changes what somebody does next.

## How it works

`src/episodes.rs` is the board: attach, touch, detach, close, reap, close_all, keyed by the **external identity the client supplies** and never by a peer id. `src/capability/builtin/episodes.rs` declares `episodes.attach`, `episodes.detach` and `episodes.list` as typed descriptors, and the registry projects them like every other: the tools `majordomus_session_attach`, `majordomus_session_detach` and `majordomus_episodes`, `POST /api/v1/episodes/attach`, `POST /api/v1/episodes/detach`, `GET /api/v1/episodes`, the OpenAPI operations, the reference.

Nothing there writes a session record. The board runs `majordomus capture session --provider generic --event start|end` — the same command a provider hook's shim runs, with the same payload shape, through the same reader in `lib/capture.sh` — and reports what it said, verbatim, in the episode's `repository` field. `lib/capture.sh` admits a provider whose declared episode source is `connection` without an adapter line, because a provider that fires no event has no vendor payload shape to adapt to.

The three connection events are already there and are joined rather than reinvented: `mcp/protocol.rs` touches the episode on every message beside the peer, `http/mcp.rs` detaches it on `DELETE` and on a reaped session and closes every one on shutdown, `commands/mcp.rs` detaches it when a stdio client goes.

## How to see it

```bash
just build
# a client with no hooks at all: three frames on stdin, a complete episode on the other side
printf '%s\n' \
  '{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2025-06-18","capabilities":{},"clientInfo":{"name":"anything","version":"0"}}}' \
  '{"jsonrpc":"2.0","method":"notifications/initialized"}' \
  '{"jsonrpc":"2.0","id":2,"method":"tools/call","params":{"name":"majordomus_session_attach","arguments":{"external_id":"my-thread"}}}' \
  '{"jsonrpc":"2.0","id":3,"method":"tools/call","params":{"name":"majordomus_session_detach","arguments":{}}}' \
  | bin/majordomus-mcp --http-port 0 | jq -r '.result.structuredContent.episode.repository // empty'
# session … opened at …
# episode closed (closed, reason detach) into .ai/repo/sessions/…
majordomus capture status        # generic:session  connection  verified
```

## What it does not cover

**Raw prompt capture, and the declaration says so.** An MCP server is handed `initialize`, tool calls and notifications; the person's prompt is never among them, in any version of the protocol. `share/providers.yaml` records `generic.lifecycle.prompts: none` with that reasoning in its evidence field, and there is deliberately no `connection` value under `prompts` for anybody to reach for. Prompt capture stays provider-specific.

**A compaction.** A connection cannot see one — nothing in the protocol announces that a client is about to discard its context — so a connection episode has a start and an end and no third event. A provider that fires `PreCompact` keeps that advantage.

**An episode nobody asked for.** `initialize` opens nothing. A client that opens the server to read one rule is a peer and leaves no record, which is also what keeps `test/cases/90_mcp_shared_server.sh`'s guarantee true: serving changes the repository not at all.

## Why it exists

The repository's claim to draw the boundary below the model was, as wired, a claim about one vendor. The connection is an event source every MCP client already produces, the peer board already tracked attach, activity and detach, and `capture session` already opened and closed episodes for a provider that names its session; the three had to be joined rather than invented. `test/cases/200_generic_mcp_episode.sh` drives the whole of it through `bin/majordomus-mcp` with real JSON-RPC frames — initialize opening nothing, attach reaching the repository's store, a `kill -9` leaving the episode open to be resumed, a reconnect recovering it rather than opening a second, and a lost connection detaching rather than closing — because a lifecycle that only works when a test calls the library proves nothing about the client somebody actually closed.
