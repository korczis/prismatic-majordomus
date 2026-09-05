# MCP surface — `majordomus mcp`

What the Rust executable under [`apps/majordomus-cli/`](../apps/majordomus-cli/) serves
to an MCP client, where it comes from, and what it refuses. Behaviour as implemented and
tested; where implementation and this document disagree, the document is wrong and
changes in the same commit as the fix. The developer-facing detail (architecture, every
option, the kind schema) is in the application's own
[`README.md`](../apps/majordomus-cli/README.md).

## What it is

A process the client starts, speaking the Model Context Protocol on its stdin and stdout,
serving the repository's AI layer read-only, and joining the repository's one shared
server: the first such process in a repository binds the loopback HTTP projection beside
its stdio session (Swagger UI, the OpenAPI document, every capability route, MCP over
HTTP) and says where; every later one attaches to it.

```bash
cargo build --manifest-path apps/majordomus-cli/Cargo.toml
apps/majordomus-cli/target/debug/majordomus mcp --inspect    # what would be served, and every diagnostic
apps/majordomus-cli/target/debug/majordomus mcp              # serve until the client goes; the first one is the server
apps/majordomus-cli/target/debug/majordomus mcp --standalone # this client alone: no port, no lease, no peers
bin/majordomus-mcp                                           # the same, built first when needed: what a client configuration names
```

The log on stderr names it the moment it is up:

```
shared server listening on http://127.0.0.1:8741 (swagger ui http://127.0.0.1:8741/docs, openapi http://127.0.0.1:8741/openapi.json, mcp over http http://127.0.0.1:8741/mcp); the one server for this repository ...
```

It is not a daemon: nothing starts it but a client, nothing keeps it alive but clients,
and it ends when its own client is gone and the last attached one has left. It keeps no
state beyond its memory, needs no database, and writes one file: the lease under
`.ai/local/state/mcp/`, the checkout-local half the layer reserves for operational state,
removed when the server stops. It is one projection of the executable's capability
registry ([`CAPABILITIES.md`](CAPABILITIES.md)): the same capabilities are the HTTP routes
and the `capabilities` commands, and every tool and resource here is derived from a
registry entry, none declared in the MCP code. The decision is
[`.ai/repo/adrs/0003-shared-mcp-server-peers-and-client-autostart.md`](../.ai/repo/adrs/0003-shared-mcp-server-peers-and-client-autostart.md).

## One server per repository

| | |
|---|---|
| election | the first process to create `.ai/local/state/mcp/server.json` (atomically) is the server; it binds, writes its URL into the file, and logs it |
| port | `--http-port` (default `8741`) on `--http-host` (default `127.0.0.1`); a taken port is replaced by a free one and both are logged, so a second repository or a stray process never stops a client from starting |
| attaching | a later `majordomus mcp` reads the lease, checks that the server answers for this root, and bridges its stdio to `/mcp`: one HTTP request per message, a ping every twenty seconds, no index and no registry of its own, so it starts in milliseconds |
| stale lease | a lease whose server does not answer for this root (the process was killed), a file that is not a lease document, an empty one, or one whose owner published no URL within fifteen seconds is taken over by the next process, and the log says which of these it was; nothing a client leaves behind can lock the others out |
| lifetime | the server serves while its own client is attached or any peer is; when the owner's client goes first, the log says `serving until the last peer leaves`; when the last peer goes, the server stops, closes the port and removes the lease |
| signals | `SIGTERM`, `SIGINT` or `SIGHUP` (a client killing its server, Ctrl-C in a terminal) removes the lease inside the handler before the process dies of the signal; `kill -9` cannot be caught, and the next process takes the stale lease over |
| takeover | a bridged peer whose server died elects again on its next message: it becomes the server itself, carrying its client's `initialize` across so that the client never notices, or attaches to whichever process won first (`re-attached to the shared server`); when it can serve neither way (its own `--strict` refuses a degraded layer) the client gets a JSON-RPC error naming why, never silence |
| options | the server's `--discovery` and `--strict` apply to every session it serves; a bridge inherits them and the log says which server it attached to |
| `--standalone` | the first version's behaviour: this client alone, no port, no lease, no peers, nothing written anywhere |
| degraded | when the lease cannot be written or replaced, or the shared server cannot start, the client is served alone exactly as `--standalone` would, and the log says `cannot use the shared server` with the path and the reason |
| `serve` | the same shared server without a stdio session of its own; when one already runs it logs the URL and exits 0 |

## Starting it from a client

The root of this repository carries the configuration each client reads, all naming the
same launcher, so that the first client to open the repository becomes the server and the
others attach:

| client | file | what it names |
|---|---|---|
| Claude Code | [`.mcp.json`](../.mcp.json) | a stdio server, `bin/majordomus-mcp`; Claude Code asks once whether to trust a project server |
| Gemini CLI | [`.gemini/settings.json`](../.gemini/settings.json) | the same launcher under `mcpServers.majordomus` |
| Codex | [`.codex/config.toml`](../.codex/config.toml) | `[mcp_servers.majordomus]`, loaded when the project is trusted |
| anything speaking Streamable HTTP | the running server's `/mcp` | `initialize` answers with an `Mcp-Session-Id`; every later request carries it; `DELETE /mcp` ends the session; an idle session expires and the client re-initialises on the 404, as the transport prescribes |

`bin/majordomus-mcp` builds the executable when it is missing or older than its sources
(`MAJORDOMUS_BUILD_PROFILE=release` for a release build; `MAJORDOMUS_BIN` names an
executable and never builds; `MAJORDOMUS_NO_BUILD=1` refuses to build and exits 12 with the
command to run), sets `MAJORDOMUS_SHARE` to the distribution's `share/` beside it, writes
nothing to stdout, and passes every argument to `majordomus mcp`. `just mcp`, `just serve`
and `just inspect` at the root do the same for a person; `just docs-ui` opens the running
server's Swagger UI.

## Peers

Every session is a peer: the server's own stdio client, every bridged `majordomus mcp`,
and every client speaking `/mcp` directly. A peer is named by what its client sent in
`initialize` (`clientInfo.name` and `version`: `claude-code`, `codex`, `gemini-cli`, or
whatever the client calls itself), numbered `p1`, `p2`, ... in attachment order, with the
transport, when it attached and when it was last seen. The `initialize` result's
`instructions` tell a client the server's URL, its own peer id and every other peer with
what it announced, before its first tool call.

| tool | capability | arguments | answers |
|---|---|---|---|
| `majordomus_peers` | `peers.list` | none | every peer, the caller's own id, and each peer's announcement |
| `majordomus_announce` | `peers.announce` | `intent`, `scope?` | the calling peer's record with its announcement |

An announcement is one line of intent and the repository-relative paths the peer expects
to touch. It is informational: other clients read it to avoid a collision; nothing here
enforces it (the shell tool's `start --scope` and `check` do that, per worktree). The
board lives in the server's memory and is gone with the process; `peers.announce` is the
one capability of kind `command`, because it changes that memory, and it is announced to
MCP clients as not read-only. Over plain HTTP there is no caller, so `POST
/api/v1/peers/announce` is refused (422) and `GET /api/v1/peers` answers without a
`caller`.

## What decides what is served

The executable names no repository file except the two conventions the layer itself
documents, `.ai/manifest.yaml` and `sources.yaml` under the `knowledge` section, and reads
how each kind is read from the tool distribution at run time. The rest is data:

| decides | read from |
|---|---|
| where the layer is | the nearest ancestor holding `.ai/manifest.yaml`; `.git` and `.majordomus/` are not markers |
| which sections exist | `sections:` in the manifest |
| which files are sources, of which kind | `.ai/repo/knowledge/sources.yaml`, one pathspec and kind per class, through the git index |
| how a kind is read and which keys it may carry | `share/kinds.yaml` and `share/schemas/<kind>.schema.json` in the distribution, plus a repository's own under `.ai/repo/knowledge/` |
| which tools exist | the executable capabilities with an MCP tool exposure, composed in `apps/majordomus-cli/src/capability/builtin.rs` |

Consequences a repository can rely on:

- a new rule, prompt, profile, milestone, issue, claim or document is served after `git add`
  and a restart, with no change to the executable;
- a new class in `sources.yaml` naming a known kind, or a new kind with its schema under
  `.ai/repo/knowledge/`, is served the same way;
- `.ai/local/` is never served, tracked or not;
- the YAML read is the subset [`SCHEMAS.md`](SCHEMAS.md) defines, so a file the shell tool
  refuses is refused here with the same line named.

## Resources

| URI | content |
|---|---|
| `majordomus://repository` | JSON: root, layer schema, sections, git state, discovery mode, kinds present, every diagnostic |
| `majordomus://scope` | JSON: the repository scope as read, its origin, and every tracked file tallied against it ([`SCOPE.md`](SCOPE.md)) |
| `majordomus://<kind>/<identity>` | the file as read, `text/markdown` or `application/yaml` |

A URI is resolved once, by one function, wherever it is asked for: `resources/read`,
the `majordomus_get` tool and `GET /api/v1/object` answer the same URI alike. A URI the
registry does not know is not found on every one of them. `majordomus://repository` is a
query (`repository.info`) with a resource exposure: read as a resource it is the report
as JSON text; read through `majordomus_get` it is the report as data (`answer`) with the
same text beside it (`content`). The registry refuses a query exposed as a resource whose
input requires anything, because a read supplies none.

Identity is the kind's identity fields joined with `@` — `majordomus.scope-integrity@1`
for a rule, `continue` for a prompt, `implementation` for a profile, `M001` for a
milestone — or, for a kind with no identity fields (policy, document), the
repository-relative path. Every listed resource carries `_meta.majordomus` with the kind,
the identity and the provenance: path, directory, the class that discovered it, the
manifest section it falls under, and its size.

## Tools

| tool | capability | arguments | answers |
|---|---|---|---|
| `majordomus_list` | `objects.list` | `kind?`, `tag?` | the objects, summarised |
| `majordomus_get` | `objects.get` | `uri` | tagged by `source`: `declarative`, a file of the layer with metadata, provenance, media type and content; or `builtin`, a query the URI projects (`majordomus://repository`) with its `answer`, the capability's provenance and the same text as `content` |
| `majordomus_search` | `objects.search` | `query`, `kind?`, `limit?` | case-insensitive substring hits with one snippet line each |
| `majordomus_repository` | `repository.info` | none | the `majordomus://repository` document |
| `majordomus_scope` | `repository.scope` | none | the `majordomus://scope` document: the declaration, its origin, the tally |
| `majordomus_scope_classify` | `repository.scope_classify` | `path` | whether a repository-relative path is in or out of the scope, the reason, and the rule that decided |
| `majordomus_capabilities` | `capabilities.list` | `kind?`, `exposure?` | every capability with its projections |
| `majordomus_capability` | `capabilities.describe` | `id` | one capability: schemas, provenance, every projection |
| `majordomus_peers` | `peers.list` | none | the clients attached to this shared server (above) |
| `majordomus_announce` | `peers.announce` | `intent`, `scope?` | records what the calling peer is working on (above) |
| `majordomus_perf` | `perf.counters` | none | this process's work counters and phase timings: what happened once at startup, what happens per call |

Every query is read-only and says so in its annotations; `majordomus_announce`, the one
command, says it is not, and it changes this process's memory and nothing else. Each tool
carries the canonical id in `_meta.majordomus.id` and its `inputSchema` and
`outputSchema` from the canonical schemas. A refused call is a result with
`isError: true`; an unknown tool, method or resource is a protocol error.

## Failure behaviour

| state | what happens |
|---|---|
| no `.ai/manifest.yaml` above the working directory | exit `12`, the start directory named |
| project data under `.majordomus/` and no manifest | exit `12`, naming `majordomus migrate` |
| manifest or `sources.yaml` malformed, unknown key, unsupported schema | exit `10`, the path and the key or line named |
| one file that cannot become an object | excluded; an error diagnostic names its path and a stable code; the index is `degraded` and still serves; `--strict` exits `10` instead |
| `git` unusable | `--discovery vcs` (the default) exits `13` naming `--discovery filesystem`; the git block of `majordomus://repository` reads `unavailable` with the reason |
| no share directory (kinds and schemas) found | exit `12`, every directory tried named, and `--share` / `MAJORDOMUS_SHARE` named as the remedy |
| a kind schema or a schema file invalid, or a repository redefining a distributed kind | exit `10`, both files named |
| client closes its pipe | the process ends with `0`, after the last attached peer has left when it was the server |
| the port is taken | a free port is bound instead; both are logged |
| the lease names a server that does not answer for this root | it is taken over; `stale lease` is logged with the URL |
| the shared server a bridge is attached to dies | the bridge takes over on its next message, or re-attaches to the process that did; the client never re-initialises |
| a bridge cannot take over (its `--strict`, a broken layer) | the client's request is answered with a JSON-RPC error (`-32603`) naming why; the stdio session stays open |
| the lease file is corrupt, empty, or has had no URL for longer than the bind grace | it is taken over; `corrupt lease`, `empty lease` or `abandoned lease` is logged with the path |
| the lease cannot be created, joined or replaced (a filesystem refusing writes under `.ai/local/`), or the shared server cannot start | the client is served alone, as `--standalone` would: `cannot use the shared server` is logged with the path and the reason, then `serving this client alone`; no port, no lease, no peers; the layer's own errors still exit as above |
| the server gets `SIGTERM`, `SIGINT` or `SIGHUP` | the lease is removed inside the handler and the process dies of the signal; its bridges elect again on their next message |
| `--http-host` is not a loopback address | served, with a warning that every host reaching that interface can read the layer, its diagnostics and its peers |
| an HTTP client leaves without `DELETE /mcp` | its session expires after ninety seconds of silence; a server whose owner has already left ends then, never later |

Two files of one kind claiming one identity are both excluded and both named, as the
rules contract requires. Nothing is repaired, defaulted or rewritten.

## Not served, on purpose

- **MCP prompts.** The repository's prompt assets render `{{CONTEXT}}` from checkout-local
  state the shell tool owns; served unrendered they would read as finished. They are
  resources (`majordomus://prompt/<name>`), not prompts.
- **The hierarchy of bootstrap files.** Root `README.md`, `AGENTS.md` and the other
  provider files are served as documents with their directory recorded; nothing merges
  or ranks them, because the repository defines no merge semantics. Recorded in
  [`.ai/repo/adrs/0001-rust-cli-and-stdio-mcp.md`](../.ai/repo/adrs/0001-rust-cli-and-stdio-mcp.md).
- **Any mutation of the repository**, subscriptions, list-change notifications, and a
  server-initiated stream on `/mcp` (this server sends nothing unasked). The HTTP
  projection of the same registry is served by the shared server and by `majordomus
  serve`; see [`CAPABILITIES.md`](CAPABILITIES.md).
- **Persistent coordination.** The peer board is one process's memory: what a peer is
  working on across sessions and machines is the shell tool's task record and scope, not
  this.

## What proves it

`test/cases/72_rust_mcp.sh` builds the executable and speaks to it over pipes inside a
repository the shell tool's `init` wrote, and reads `majordomus://repository` through the
tool and the resource read to see one answer; `76_capabilities_projections.sh` reads one
capability back through every interface; `90_mcp_shared_server.sh` starts two clients
through `bin/majordomus-mcp` in such a repository and checks that one server serves both,
that each sees the other, that the lease comes and goes with the server, and that the
three client configurations name the launcher. The crate's own suite
(`cargo test --manifest-path apps/majordomus-cli/Cargo.toml`) covers the command line,
root selection, the metadata contract, determinism, the protocol round trip, protocol-only
stdout, non-mutation, the add–remove–break sequence of external extension, and, in
`tests/mcp_shared.rs`, the shared server over real pipes and sockets: the election, the
bridge, `/mcp` sessions, the fallback port, `serve` deferring, `--standalone`, the takeover
after a kill, the re-attachment, the refusal when the taker cannot serve, a corrupt, foreign,
empty or abandoned lease being taken over, two clients starting in the same instant, an
unwritable lease directory degrading to a standalone session, `SIGTERM` removing the lease,
malformed traffic on `/mcp`, and the bridge's transparency: a bridged session and a
restarted server answer byte for byte what the first server did. The doctrine behind the
failure table is the rule `project.shared-server-resilience`. `tests/hot_path.rs` sends
hundreds of frames and requires the startup counters (`majordomus_perf`) unchanged;
`majordomus bench` times every tool through a real child process
([`CAPABILITIES.md`](CAPABILITIES.md)). The claims are in [`CLAIMS.yaml`](CLAIMS.yaml)
under `mcp-`, `hot-path-no-rebuild` and `benchmark-coverage-derived`.
