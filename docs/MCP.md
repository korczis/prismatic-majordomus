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
its stdio session (a home page listing every surface, this repository's documentation, Swagger UI, the OpenAPI document, every capability route, MCP over
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
shared server listening on http://127.0.0.1:8741 — 7 surface(s): api http://127.0.0.1:8741/api/v1, cockpit http://127.0.0.1:8741/cockpit, docs http://127.0.0.1:8741/docs, home http://127.0.0.1:8741/, mcp http://127.0.0.1:8741/mcp, openapi http://127.0.0.1:8741/openapi.json, swagger http://127.0.0.1:8741/swagger; the one server for this repository ...
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

### One repository, every server of it

A server serves a checkout. A linked worktree is a checkout of its own — it has the
manifest, so it has a root, a lease and a server of its own — and until ADR 0035 nothing
said that two such servers belonged to one repository. Now the index route (`GET /`) names
the git repository the checkout belongs to beside the checkout's own identity:
`repository_id` is the checkout's (a digest of its root, what the lease probe compares),
`git_repository_id` is the repository's (a digest of the git directory every worktree
shares; absent where git cannot be asked), `linked_worktree` says whether this is the
primary checkout, and `leaseholder` says whether the process answering is still the one
(below). `GET /api/v1/server` — the tool `majordomus_server`, the resource
`majordomus://server` — lists every checkout git registers for the repository, the primary
first, each with its lease, where its server stands and how many peers it holds, and says
where this checkout's own server stands measured against what this executable would serve:

| standing | meaning |
|---|---|
| `absent` | no lease: nothing serves this checkout |
| `starting` | a lease without an address, young enough that its owner is still binding |
| `ready` | the server the lease names answers for this checkout, from the file on disk, at this executable's version |
| `outdated` | it answers, but from another version, or from a file replaced since it started: everything it says is yesterday's |
| `stale` | the lease names a server that does not answer, or is not a lease at all; the reason says which |

The list is what a caller gets by asking nothing, and it costs a lease read and a probe per
checkout — on a machine with a hundred worktrees registered, a hundred of each. A caller
that only wants to know about the checkout it is in says so — `checkouts=this` on the query
string, in the tool's input, or `--checkouts this` on the command line — and that reading
enumerates no other checkout, reads no other lease and probes no other server. The default
is the whole list, because that is the answer this capability gave before the field existed.

The lease itself is one type, read once (`lease::LeaseDocument`, `lease::LeaseFile::read`):
the election, the read-only `serving`, the environment snapshot and the status all parse it
through the same reading, and it carries the server's `version` beside the executable it
was started from. `.ai/local/state/mcp/server.json` is still where a person reads it with
`cat`; the status is where a program does.

### The server that is no longer the one

A lease can be taken over while the process that held it is still running and still healthy
— most often because its executable was replaced by a rebuild, which the election reads as
"it is serving code that is no longer on disk". The superseded process is not killed. It
keeps its socket, its peer board and the generation of the layer it loaded, and it goes on
serving the sessions it already had until they end. That is deliberate: a client mid-answer
should not lose its server because somebody ran `cargo build`.

What it must not do is go on being *the* server. From the moment its own reader finds the
lease no longer carries its token:

| | |
|---|---|
| its index says so | `GET /` answers `leaseholder: false`; every other field — `repository`, `repository_id`, `git_repository_id` — is as true of it as of the current server, which is exactly why one field has to separate them |
| the probe refuses it | `lease::probe` asks three questions, not two: a Majordomus server, this checkout, still the leaseholder. A client holding an address from before the takeover is told there is no server there rather than served a board nobody else can see |
| it takes on nobody new | an `initialize` with no session gets `409 lease_lost`, naming the launcher as the way to the current server |
| its open sessions continue | they are its own until they end, and the process ends with them |

A server too old to answer the third question is accepted by the probe. It cannot be told
from a current one on that endpoint, and refusing it would be the worse failure: a live
server taken for dead is taken over, which is how one checkout comes to have two.

This was measured before it was written: on 2026-09-10 this repository had two servers, one
lease, and a session whose environment carried the older address read a peer board with one
peer on it while the board everybody else shared had two.

### Four readings of "ready"

This executable answers "ready" four times, and the audit that prompted ADR 0035 counted
three of them as an accident (`docs/ENTRY_AUDIT.md`, root cause 5). They are not one
question badly split: they are four questions about four subjects, and merging any two
would lose the one thing that reader needs. Each is named here with its owner, so that the
next reader does not have to rediscover which one they are looking at.

| the question | the answer | who asks it |
|---|---|---|
| Is a **surface's** producer's output on disk? | `http::Served::ready(surface_id)` — the directory a producer writes into has files in it; a route this executable answers is always ready | the home page (`GET /`), which renders a surface with no output as `not built` rather than serving a 404 |
| Can **this process** answer a request? | `health.ready` — `GET /api/v1/ready`: the registry and the index it built at start-up, and how the layer read. Local initialisation only | a hosting platform's readiness probe. It contacts nothing outside this process on purpose: a readiness check that probes a dependency fails a deployment for something that is not this process |
| Does **anything** accept a connection at the address the lease published? | `environment::ServiceAvailability` — one TCP connect with a hard budget and no name resolution (`environment::probe::reachable`) | the environment snapshot, which runs on a shell prompt (`majordomus env`, `.envrc`) and may not spend an HTTP round trip or reach DNS to say what it knows |
| Is what answers there **current**? | `ServerStanding` — `server.status`, from `lease::probe` (this checkout's identity, over HTTP) and the version and executable the lease carries | anyone who has to trust what the server says: `serve ensure`, `serve stop`, the `server` check of `health.report`, and the session briefing |

Read down the column and the ladder is plain: the third asks whether a socket is open, the
fourth whether the process behind it is this checkout's, from the file on disk, at this
build. A stale server answers the third and fails the fourth, which is exactly the class
that has cost this repository a day before now.

`health.report` carries the fourth and only the fourth. The first is per surface and
belongs on the page that renders surfaces; the second is a statement about the process
answering the report, which cannot be false where the report is being produced; and the
third cannot tell a live server from a socket somebody else holds. The check delegates to
`capability::builtin::server::standing_at` — the same reading `server.status` answers from
— so the health report and the status cannot say two different words about one lease.

### Ensuring a server, and stopping it

```text
majordomus serve status [--checkouts this|repository] [--format json]
                                            where this checkout's server stands, and every server of the repository
majordomus serve ensure [--idle S] [--wait S] [--port P]
                                            a ready server for this checkout, started if it must be
majordomus serve stop [--wait S]            end the server this checkout's lease names
```

`serve ensure` reads the lease and probes the server it names, exactly as the election
does, and converges: `ready` is printed and nothing is started; `starting` is waited for;
`absent` and `stale` start a server as a process of its own — this executable, `serve
--fallback --idle S`, its log at `.ai/local/state/mcp/server.log`, in its own process group
so that it outlives the shell that asked — and wait until it is ready; `outdated` starts one
only when the election would take the lease over (the same executable, replaced on disk)
and is otherwise reported with the remedy, because a server of another build that answers
is not this command's to end. Run twice, it starts nothing the second time; run by three
shells at once, the election lets one of the three servers bind and the others defer. The
call is bounded by `--wait`; a server that did not become ready in time is reported with
the standing it reached and exit 10.

A server `ensure` starts has no client of its own. It ends when no peer has been attached
for `--idle` seconds (fifteen minutes by default), which is what keeps ADR 0003's line —
there is no process without a client — true in time rather than at every instant: an
agent's entry is owed a server before its first attach, and a checkout nobody works in
does not keep one.

`serve stop` signals the server the lease names, when that server answers for this
checkout, and waits for the lease to go. A lease that names a server of another checkout,
or one that does not answer, is left alone and said so; nothing here kills a process that
was not asked for by name.

**Who calls `ensure`.** The provider's start event does (`session.ensure_server_on_start`
in the policy, on by default): the one moment a server nobody has started yet is owed one is
when an agent arrives, and the briefing the event writes carries one line — `Shared server:
ready http://127.0.0.1:8741 pid 123` — so that a worker knows before its first tool call
whether the board it is told to read exists. The event never builds the executable: one
that is missing or older than its sources is named in that line and left alone, because a
hook is not the place to start a compiler and a server from stale code would answer with
yesterday's tree. An MCP client's launcher (`bin/majordomus-mcp`) has always converged the
same way through the election; a shell entering the repository (`.envrc`) is told and not
served, because `project.envrc-is-an-adapter` forbids the entry hook to start anything and a
shell is not a client.

**An announcement outlives the server it was made to.** The bridge sees every frame its
client sends, so it keeps the arguments of the last announcement the server accepted and
says them again wherever the client lands next: after a re-attach, once the session is
re-opened; after a takeover, onto the board of the server the bridge's own process has
become. A worker that announced once is on the board of every server that serves it,
without being asked to announce again; the instruction to announce again after a
reconnect stays in the bootstrap for the one case a bridge cannot cover, a client whose own
process is the server and restarts.

**What the election now guards against.** An owner keeps its lease young while the layer
loads (`Lease::keep_alive`), so a cold start slower than the bind grace is never taken for
an abandoned one; a take-over removes only the file it judged, never one that arrived in the
meantime; an owner whose lease was taken over while it was binding refuses to publish and
degrades, rather than writing over the winner's address; and a server whose lease is taken
over later stops claiming it — its signal handler no longer unlinks the file, which is
somebody else's — serves the peers it has, and ends with them. The server's own reader also
forgets the HTTP sessions that stopped pinging on every path, not only while the owner
waits for peers to leave, so a dead peer never stays `attached` on the board.


## Starting it from a client

The root of this repository carries the configuration each client reads, all naming the
same launcher, so that the first client to open the repository becomes the server and the
others attach:

| client | file | what it names |
|---|---|---|
| Claude Code | [`.mcp.json`](../.mcp.json) | a stdio server, `bin/majordomus-mcp`; Claude Code asks once whether to trust a project server |
| Gemini CLI | [`.gemini/settings.json`](../.gemini/settings.json) | the same launcher under `mcpServers.majordomus` |
| Codex | [`.codex/config.toml`](../.codex/config.toml) | `[mcp_servers.majordomus]`, loaded when the project is trusted |
| bb | nothing of its own | an orchestrator: the agent it starts (Claude Code, Codex, an ACP agent) reads its own file above, so a bb thread attaches through the agent, not through bb (ADR 0024). Claude Code under bb runs with `settingSources: project`, which loads `.mcp.json`; a server loaded from a settings file gets two seconds before the first turn, so a cold checkout that has to build the executable shows it `pending` at init and connected afterwards |

The rows are the providers whose declaration names a client configuration; the whole set,
with what each reads and where it keeps its scratch checkouts, is
[`docs/generated/providers.md`](generated/providers.md), generated from the same declaration.
| anything speaking Streamable HTTP | the running server's `/mcp` | `initialize` answers with an `Mcp-Session-Id`; every later request carries it; `DELETE /mcp` ends the session; an idle session expires and the client re-initialises on the 404, as the transport prescribes |

`bin/majordomus-mcp` builds the executable when it is missing or older than its sources
(`MAJORDOMUS_BUILD_PROFILE=release` for a release build; `MAJORDOMUS_BIN` names an
executable and never builds; `MAJORDOMUS_NO_BUILD=1` refuses to build and exits 12 with the
command to run), sets `MAJORDOMUS_SHARE` to the distribution's `share/` beside it, writes
nothing to stdout, and passes every argument to `majordomus mcp`. `just mcp`, `just serve`
and `just inspect` at the root do the same for a person; `just docs-ui` opens the running
server's Swagger UI. The same document, with its tags, examples and statuses, is the API
reference on the site at `/docs/api/` and is served raw at the site's `/openapi.json`; the
`initialize` instructions, the OpenAPI `info` and `GET /` open with the one summary from
`about.rs` ([`CAPABILITIES.md`](CAPABILITIES.md)).

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
| `majordomus_peers` | `peers.list` | none | every peer, the caller's own id, each peer's announcement, and every pair of claims that meet |
| `majordomus_announce` | `peers.announce` | `intent`, `scope?` | the calling peer's record, and the peers whose claimed scope it collides with |

An announcement is one line of intent and the repository-relative paths the peer expects
to touch. The board lives in the server's memory and is gone with the process;
`peers.announce` is the one capability of kind `command`, because it changes that memory,
and it is announced to MCP clients as not read-only. Over plain HTTP there is no caller,
so `POST /api/v1/peers/announce` is refused (422) and `GET /api/v1/peers` answers without
a `caller`.

**A claim is answered, not merely recorded.** `peers.announce` compares the scope it is
given against every other announcement and returns the peers whose claims meet it, with
the pairs of paths that meet: two claims meet when they are equal or one is inside the
other (`apps` contains `apps/majordomus-cli`; `app` does not, because a claim is a path
and not a prefix of a string). `peers.list` reports the same collisions across the whole
board, each pair once. It is still not enforcement — the shell tool's `start --scope` and
`check --overlap` do that, per worktree, and they are what refuses a commit — but a
collision is now known at the moment it is created rather than discovered afterwards in
the history of a branch.

**The board reaches a worker that never asks for it.** Reading it was voluntary, and
voluntary co-operation failed: a session announced, its transport was re-established under a
new peer id, and it was invisible to eight others for three hours; two sessions built the
same subsystem because neither looked first. So `majordomus context` — the command the
bootstrap tells every worker to run before working — carries a `PEERS` section between `GIT`
and `TASK`: who else is attached, what each of them claims, and specifically which of those
claims meets the current task's scope. It reads the lease of the shared server (which is
repository-scoped, so a linked worktree finds it through `--git-common-dir` in the primary
checkout) and asks `GET /api/v1/peers`. The hint is never load-bearing: no lease, no `jq` or
`curl`, a server that does not answer within two seconds, or a board holding nobody leaves
`context` exactly as it was, because a repository with one worker in it must not grow a
section about being alone. `test/cases/106_context_peers.sh` holds all of it.

**An announcement outlives the connection that made it.** A session that reconnects used
to lose everything it had said, silently, to itself and to everyone else; the board now
keeps a departed peer's announcement and lists it with `attached: false`, so what a
session said it was working on survives a dropped socket. A peer that never announced
leaves nothing behind, the newest 32 departed peers are kept so that a server which ran
all day is not a museum, and an attached peer is never evicted to make room for one that
left. `peers.list`'s `count` is the peers actually attached; the `peers` array is longer
when the board is holding what somebody said before they went.

## Episodes

A peer is a connection. An **episode** is a sitting of work, and for a client with no
provider hooks of its own the connection is the only thing that can draw its boundary
([ADR 0043](../.ai/repo/adrs/0043-every-client-gets-an-episode-and-every-provider-capability-cites-its-evidence.md)).
Until it, drawing the boundary below the model was wired for Claude Code alone, and the
repository's claim to do so was a claim about one vendor.

| tool | capability | arguments | answers |
|---|---|---|---|
| `majordomus_session_attach` | `episodes.attach` | `external_id` | the episode this connection now holds, whether it was resumed, and the reattach grace |
| `majordomus_session_detach` | `episodes.detach` | `external_id?` | the episode as it was closed, and what the repository's end event reported |
| `majordomus_episodes` | `episodes.list` | none | every episode this server holds, open and detached, and the caller's own |

```
initialize                    → a peer attaches; no episode
episodes.attach(external_id)  → the episode opens, or this client's own is resumed
  … every message is that episode's heartbeat …
the connection goes           → detached, not closed
episodes.attach again         → resumed: the same episode, a new peer id
episodes.detach               → closed, deliberately, into a session record
no reconnect in 15 minutes    → closed by the reaper, as interrupted
the server stops              → closed, shutdown
```

**`initialize` opens nothing.** A client that opens the server to read one rule is not a
worker and leaves no record; attach is a call, made by the client that knows it wants an
episode. That is also what keeps the guarantee `test/cases/90_mcp_shared_server.sh` holds —
serving changes the repository not at all.

**The identity is the client's, never the peer id.** `external_id` is what the client
durably calls the sitting it is in — its conversation or thread id — and its whole job is to
survive a reconnect. A peer id is handed out per connection and is a different string every
time the client comes back; treating one as durable is what made a session invisible to
eight others for three hours on 2026-09-09.

**A dropped connection detaches; it does not close.** Two clocks govern a client's
disappearance and they answer two questions. `SESSION_IDLE_TIMEOUT` (90s) decides when a
socket is forgotten. `episodes::REATTACH_GRACE` (15 minutes) decides when the *work* is
over. A reader of the logs will see a connection reaped long before the episode it carried,
and that is intended.

**Nothing here writes a session record.** The board runs `majordomus capture session
--provider generic --event start|end` — the same command a provider hook's shim runs, with
the same payload shape, through the same reader — and reports what it said, verbatim, in the
episode's `repository` field. A second writer of the record the hooks already write would be
the repeated semantic definition [`CAPABILITIES.md`](CAPABILITIES.md) forbids. The
repository's own store is also what recovers an episode across a *server* restart: a killed
server writes no end event, the episode stays open in `.ai/local/state/sessions-open/`, and
the next `attach` under the same identity is `session start --if-open keep`, which keeps it.

**Raw prompt capture is not here and is declared not to be.** An MCP server is handed
`initialize`, tool calls and notifications; the person's prompt is never among them, in any
version of the protocol. `share/providers.yaml` says `prompts: none` for the generic
provider, with that reasoning in its evidence field, and there is no `connection` value under
`prompts` for anybody to reach for. What each provider *can* do, and where it was verified,
is `majordomus product providers` and `majordomus capture status`.

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
| which tools exist | the executable capabilities with an MCP tool exposure, composed under `apps/majordomus-cli/src/capability/builtin/` |

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
| `majordomus_worktrees` | `worktree.topology` | none | the `majordomus://worktrees` document: the container, the trunk, every worktree with its standing and diagnostics, every branch, the tallies |
| `majordomus_worktree_status` | `worktree.status` | `path?` | one worktree — the repository's own, or the one holding `path` — with its standing, canonical path, uncommitted work and whether it is where it belongs; a path in another repository is refused |
| `majordomus_worktree_inspect` | `worktree.inspect` | `branch` | the canonical path of a branch, whether it exists, what occupies the path, the worktree holding it |
| `majordomus_worktree_migration_plan` | `worktree.migration_plan` | none | every misplaced worktree with where it belongs, how it would move and what blocks it; the exceptions; changes nothing |

The table above is the core set and not the whole of it: the tool list is a projection of
the capability registry, it grows whenever a module is composed, and a list in prose that
claimed to be complete would be wrong the next time one is. `majordomus_capabilities` and
`docs/generated/capabilities.md` are the derived, total reference; what is written here is
what a reader needs before opening it.

### Sessions and continuity

Two of the tools answer about `.ai/local/` — the half of the layer that names this machine —
and they are the reason the server binds to the loopback interface. They are served and
never published: no generator writes them into `docs/generated/` and no site page carries
one.

| tool | capability | answers |
|---|---|---|
| `majordomus_continuity` | `continuity.state` | what a worker resuming *here* would be handed: the episode the pointer resolves to, the active task, the handover and checkpoint that resolve for this worktree and branch with their labels, the blockers |
| `majordomus_lifecycle_episodes` | `lifecycle.episodes` | every open episode of the store — not only the one the pointer follows — with its provider session, its standing (`current`, `open`, `foreign`, `stranded`) and the last ledger line stamped with it |
| `majordomus_lifecycle_recovery` | `lifecycle.recovery` | episodes that can no longer close themselves and the command that clears each, temporary files a killed close left in the tracked sessions section, the pointer's layout, and the started-against-closed arithmetic |
| `majordomus_lifecycle_runtime` | `lifecycle.runtime` | the commit this process's index was built at, against the commit the repository is on right now, and whether they agree |
| `majordomus_lifecycle_providers` | `lifecycle.providers` | per provider: the lifecycle events its adapter declares, whether it can archive prompts, and the enforcement entries this repository wires to its hook |
| `majordomus_lifecycle_closed` | `lifecycle.closed` | the tracked records a clone receives: how many, how many on this branch, and the newest twenty |

The first answers the worker's question and the rest answer the operator's, which is a
different question and not a superset: `continuity.state` follows
`state/session-current.yaml`, and that pointer is a symlink the most recent start event
re-aims. [`CONTINUITY.md`](CONTINUITY.md) has the model and the whole path.

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
failure table is the rule `project.shared-server-resilience`. `tests/server_status.rs` holds
the two-worktree case and the stale-lease case; `tests/health_server.rs` holds the `server`
check of `health.report` against a real server, a checkout nobody serves and a lease naming
an address nobody answers at, and asserts that the check and the status say one word about
one lease. `tests/hot_path.rs` sends
hundreds of frames and requires the startup counters (`majordomus_perf`) unchanged;
`majordomus bench` times every tool through a real child process
([`CAPABILITIES.md`](CAPABILITIES.md)). The claims are in [`CLAIMS.yaml`](CLAIMS.yaml)
under `mcp-`, `hot-path-no-rebuild` and `benchmark-coverage-derived`.
