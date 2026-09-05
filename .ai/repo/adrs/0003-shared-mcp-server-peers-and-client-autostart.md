# 3. One shared MCP server per repository, peers, and clients that start it themselves

Status: accepted, 2026-09-05. Extends ADR 1 and ADR 2.

## Context

ADR 1 made `majordomus mcp` a process per client: stdio only, no port, dead with its
pipe. ADR 2 added `serve`, a second command with the same registry on a loopback socket,
Swagger UI included. The operator then asked for four things, in this order: Swagger UI
should come up with `majordomus mcp` by default and its URL should be in the log; an MCP
client (Claude Code, Codex, Gemini CLI, ...) opened in the repository should build, start
and use the server without a hand step; exactly one server should run per repository, and
it should let the different clients coordinate out of the box; and a `justfile` should
route what a person runs to the Rust executable wherever it can do the job.

Two or three MCP clients open in one checkout is now the ordinary case, not the
exception. Each spawns its own stdio process, so "one server" cannot mean "one process
per client": it means the first process serves and the rest attach to it. The
repository's "Intentionally Absent" list refuses daemons, servers, queues and registries
of workers; the decision below has to answer it line by line, as ADR 1 did.

## Decision

- **One shared server per repository, owned by whoever started it.** The first
  `majordomus mcp` (or `serve`) in a repository binds the loopback HTTP projection
  (Swagger UI at `/docs`, `/openapi.json`, every capability route, and MCP over HTTP at
  `/mcp`) beside its own stdio session, and logs the URL. It ends when its own client is
  gone and the last attached peer has left. There is no process without a client.
- **A lease, not a daemon.** The server is found through one file,
  `.ai/local/state/mcp/server.json`, in the checkout-local half the layer already
  reserves for operational state: created atomically by the process that wins, carrying
  the URL once bound, removed when the server stops. A lease whose server does not answer
  for this root is stale and is taken over. Nothing else is written anywhere; `--inspect`
  and `--standalone` write nothing at all.
- **Every later `majordomus mcp` is a bridge.** It carries no index and no registry: it
  forwards its client's frames to the server's `/mcp` and pings so that its session is
  kept. When its server dies it elects again, and either takes over (carrying its client's
  `initialize` across, so the client never notices) or attaches to whoever did.
- **MCP over HTTP is the Streamable HTTP transport's request half**, at `/mcp`, with the
  `Mcp-Session-Id` header, `DELETE` to end a session, and an idle expiry. No
  server-initiated stream: this server sends no notifications, so a stream would carry
  nothing. A client that speaks the transport itself is a peer like any bridge.
- **Peers are registry capabilities.** `peers.list` and `peers.announce` are typed
  descriptors in `builtin.rs` with explicit exposures (tools `majordomus_peers` and
  `majordomus_announce`, `GET /api/v1/peers`, `POST /api/v1/peers/announce`), projected
  like every other capability; nothing in the MCP or HTTP code knows about peers. A peer is
  a session, named by what the client said in `initialize`; an announcement is one line of
  intent and the paths the peer expects to touch, informational, in the server's memory,
  gone with the process. The `initialize` instructions name the server's URL, the caller's
  own peer id and every other peer, so a client learns about the others before its first
  tool call.
- **A third capability kind, `command`.** ADR 2's two kinds were query (executed,
  read-only) and resource (read). `peers.announce` is executed and changes this process's
  memory, so it is a `command`: bound to `POST` and announced to MCP clients as not
  read-only, both derived from the kind by the registry's checks rather than declared per
  projection. No kind writes to the repository.
- **Clients start it.** `bin/majordomus-mcp` builds the executable when it is missing or
  older than its sources (never when `MAJORDOMUS_BIN` or `MAJORDOMUS_NO_BUILD` says so),
  sets the share directory and `exec`s `majordomus mcp`, writing nothing to stdout. The
  root carries `.mcp.json` (Claude Code), `.gemini/settings.json` (Gemini CLI) and
  `.codex/config.toml` (Codex), each naming that launcher; whichever client opens first
  becomes the server and the others attach.
- **A `justfile` at the root** routes every recipe the Rust executable can serve to it
  (`mcp`, `serve`, `inspect`, `capabilities`, `describe`, `validate`, `generate`,
  `generate-check`, the benchmarks, the coverage) and keeps the lifecycle recipes on the
  shell tool. It is thin by construction: every recipe names the script or command that
  owns the behaviour.

## Against the "Intentionally Absent" list, point by point

| the list refuses | this decision |
|---|---|
| a daemon, server, background monitor | the shared server is a client's child that lingers only while another client is attached, and ends with the last one; nothing starts it but a client, nothing keeps it alive but clients, and `--standalone` removes it entirely |
| a database, queue, vector store | one JSON lease file under the local half, removed on exit; the peer board is memory of one process |
| a registry or catalogue of named workers | the board lists live sessions by the name the client itself sent, routes nothing by it, assigns no role, tier or persona, persists nothing, and cannot be edited; it is the same kind of fact as the overlap report: who is here now |
| a finding without a reproduce command | a stale lease, a taken port, a takeover and a failed takeover are each logged with the URL or the reason; `GET /api/v1/peers` and `majordomus_peers` reproduce the board |
| a self-report trusted without an independent check | `tests/mcp_shared.rs` spawns the processes and speaks to them over real pipes and sockets, kills the server and watches the bridge take over; `test/cases/90_mcp_shared_server.sh` does it through the launcher in a repository `init` wrote |
| a number written where a command could compute it | every count (peers, sessions, routes) is computed; the benchmarks report and assert nothing |

## Invariants

1. At most one server per repository root answers for that root; a second process finds
   it through the lease or takes over a stale one, never binds beside it.
2. A client's stdio session survives its server's death when any process can serve the
   layer; when none can, the client gets a JSON-RPC error naming why, never silence.
3. The owner's stdio session pays nothing for the HTTP side: the workers are separate
   threads over an immutable registry; `benches/shared.rs` measures the same call with the
   server bound and idle against no server at all.
4. Peers are projections of the registry like everything else; the kind decides the HTTP
   method and the MCP annotations.
5. Nothing under the repository's tracked tree is written by any mode; the lease lives
   where the layer's contract already puts operational state.

## Alternatives rejected

- **A daemon or a system service.** Refused by the list, and unnecessary: a server that
  ends with its last client answers the operator's "always exactly one".
- **One server per client on distinct ports.** Every client would have its own Swagger UI
  and none would see the others; "one server" was the requirement.
- **A Unix socket or a file under `/tmp`** as the rendezvous. The layer already has a
  checkout-local half for exactly this class of state, per worktree, and a person can read
  the lease with `cat`.
- **A boolean `read_only` on the descriptor** instead of a kind: the registry's shape
  checks, the HTTP method and the annotations would each have had to consult it
  separately; a kind carries the semantics once.
- **Keeping the bridge's own index** so that `--strict` and `--discovery` apply per
  client: the bridge would then start as slowly as a server for nothing; the server's
  options apply to the session, the log says so, and a peer that cannot serve the layer
  under its own options says so when it is asked to take over.
- **An MCP SDK or an HTTP client crate** for the bridge: one loopback request per message
  is thirty lines over `TcpStream`, and the server always answers with `Content-Length`.
- **Making the shell tool dispatch `mcp`, `serve`, `capabilities` and `generate`** in this
  change: on `master` a dispatched command is a public command with a registry entry, a
  fixture and coverage (`share/commands.yaml`); that is a change to the shell tool's
  surface and lands as its own step after this branch is rebased onto it. Until then
  `bin/majordomus-mcp` and the `justfile` are the entry points.

## Consequences

The lease file, its schema, the `/mcp` endpoint, the session header, the peer capabilities
and the `command` kind become compatibility surfaces. A long-lived shared server keeps the
index it built at start; a client that attaches later sees the layer as it was then, which
is the restart-based rediscovery ADR 2 already made the contract, and the server ends when
the last client leaves. The documents that said "one process per client, no port" now say
"one server per repository, owned by its clients", and `docs/MCP.md` carries the
lifecycle.

## Testing strategy

Black-box first: `tests/mcp_shared.rs` spawns the binary as owner, bridge and direct HTTP
client, over real pipes and a real socket, and covers the lease, the fallback port,
`serve` deferring, `--standalone`, the takeover after a kill, the re-attachment when another
process won the lease, and the refusal when the taker cannot serve the layer;
`tests/shared_units.rs` covers the board, the lease, the endpoint's sessions and expiry,
the bridge's client and the stdio loop in process; `benches/shared.rs` measures the paths
that the operator asked to be optimised; `test/cases/90_mcp_shared_server.sh` runs the
launcher and two clients in a repository the shell tool's `init` wrote and checks the
client configurations. The gates of ADR 2 are unchanged.
