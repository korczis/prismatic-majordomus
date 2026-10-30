+++
title = "MCP surface"
description = "the read-only MCP surface of the Rust executable: what it serves, what decides that, how it fails, what it refuses to serve"
weight = 62
[extra]
source = "docs/MCP.md"
+++

{% raw %}

What the Rust executable under [`apps/majordomus-cli/`](https://github.com/korczis/prismatic-majordomus/tree/@source-ref@/apps/majordomus-cli) serves
to an MCP client, where it comes from, and what it refuses. Behaviour as implemented and
tested; where implementation and this document disagree, the document is wrong and
changes in the same commit as the fix. The developer-facing detail (architecture, every
option, the kind schema) is in the application's own
[`README.md`](https://github.com/korczis/prismatic-majordomus/blob/@source-ref@/apps/majordomus-cli/README.md).

## What it is

A process the client starts, speaking the Model Context Protocol on its stdin and stdout,
serving the repository's AI layer (reads, and the named commands that change state), and joining the repository's one shared
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
shared server listening on http://127.0.0.1:8741 — 7 surface(s): api http://127.0.0.1:8741/api/v1, cockpit http://127.0.0.1:8741/cockpit, docs http://127.0.0.1:8741/docs, home http://127.0.0.1:8741/, mcp http://127.0.0.1:8741/mcp, openapi http://127.0.0.1:8741/openapi.json, swagger http://127.0.0.1:8741/swagger; the one server of this checkout ...
```

It is not a daemon: nothing starts it but a client, nothing keeps it alive but clients,
and it ends when its own client is gone and the last attached one has left. It keeps no
state beyond its memory, needs no database, and writes one file: the lease under
`.ai/local/state/mcp/`, the checkout-local half the layer reserves for operational state,
removed when the server stops. It is one projection of the executable's capability
registry ([`CAPABILITIES.md`](@/docs/capabilities.md)): the same capabilities are the HTTP routes
and the `capabilities` commands, and every tool and resource here is derived from a
registry entry, none declared in the MCP code. The decision is
[`.ai/repo/adrs/0003-shared-mcp-server-peers-and-client-autostart.md`](https://github.com/korczis/prismatic-majordomus/blob/@source-ref@/.ai/repo/adrs/0003-shared-mcp-server-peers-and-client-autostart.md).

## One server per checkout

<div class="overflow-x-auto" tabindex="0">

| | |
|---|---|
| election | the first process to create `.ai/local/state/mcp/server.json` (atomically) is the server; it binds, writes its URL into the file, and logs it |
| port | `--http-port` on `--http-host` (default: the interface `MAJORDOMUS_HTTP_HOST` names on this machine, and `127.0.0.1` when it names none — [below](#the-machine-names-the-interface)). Without `--http-port` the server asks for the port its checkout derives, the same on every start: `8741` for a repository's primary checkout, and for a linked worktree a port in `8742`–`9740` derived from the worktree's identity (`cli::checkout_port`), so two checkouts of one repository ask for different ports and a bookmarked address or a Streamable HTTP client configuration survives a restart (I2702). `serve`, `serve ensure` and `env enter` derive it the same way, and `server.status` reports it as the desired port. A taken port is replaced by a free one and both are logged, so a second repository or a stray process never stops a client from starting |
| attaching | a later `majordomus mcp` reads the lease, checks that the server answers for this root, and bridges its stdio to `/mcp`: one HTTP request per message, a ping every twenty seconds, no index and no registry of its own, so it starts in milliseconds |
| stale lease | a lease whose server does not answer for this root (the process was killed), a file that is not a lease document, an empty one, or one whose owner published no URL within the **bind grace** is taken over by the next process, and the log says which of these it was; nothing a client leaves behind can lock the others out |
| lifetime | the server serves while its own client is attached or any peer is; when the owner's client goes first, the log says `serving until the last peer leaves`; when the last peer goes, the server stops, closes the port and removes the lease |
| freshness | the server follows the repository it serves: before a request is answered it compares a fingerprint of the git control files (`HEAD`, the staging index, `packed-refs`, the reflog, the merge and rebase markers — one `stat` each) with the one its current reading was built at, and the request that finds them different rebuilds the layer. A commit made while the server runs is visible through the API, through MCP and in the Cockpit with no restart; a client attached before the commit keeps its session and its place on the peer board. No poll, no thread, no watcher. `project.the-server-sees-the-current-tree` |
| signals | `SIGTERM`, `SIGINT` or `SIGHUP` (`serve stop`, a client killing its server, Ctrl-C in a terminal) asks the server to stop in order: the handler only records the request, and the server closes its sessions and episodes, closes its listeners and releases the lease — only while the file is still its own — before it ends. A `serve` returns 0; a `majordomus mcp` serving its own client then dies of the signal, because its client's stdio session was cut. A second signal, or an ordered stop that has not finished within eight seconds, ends the process at once; `kill -9` cannot be caught, and the next process takes the stale lease over |
| takeover | a bridged peer whose server died elects again on its next message: it becomes the server itself, carrying its client's `initialize` across so that the client never notices, or attaches to whichever process won first (`re-attached to the shared server`); when it can serve neither way (its own `--strict` refuses a degraded layer) the client gets a JSON-RPC error naming why, never silence. The message that met the failure is sent again only when the server provably did not run it — the connection was never made, or the server answered 404 (session lost) or 409 (lease lost) — or when it is idempotent: every method but `tools/call`, and a tool whose declared execution hints say so (every read). A call that was sent and never answered (a timeout, a dropped connection, a 5xx) is not sent again: the client gets `-32603` saying `outcome unknown` with the request's id, so a writing tool never runs twice |
| refusal | a bridged peer whose server is serving and will not take a request — one sent before `initialize` is answered 400 `session_required`, a message over the 1 MiB body limit 413 — relays that refusal to its client as a JSON-RPC error (`-32600`) naming the status, and elects nobody; a message over the limit is answered `payload too large` without being sent. An election is for a server that cannot be reached, that took a request and never answered, that lost the session and will not re-open it (404), or that says it lost the lease (409 `lease_lost`) |
| bounds | a message is at most 1 MiB on either transport — a stdin line over it is answered with a parse error and the session goes on, a request body over it is answered 413; a reply the bridge, a rendezvous registration or a mesh link reads is at most 32 MiB; a request naming an `MCP-Protocol-Version` this server does not speak is answered 400; a session id is 128 bits from the operating system's random source, and a session is stamped again when its answer is written, so a long request never finds its session reaped behind it (I2147) |
| options | the server's `--discovery` and `--strict` apply to every session it serves; a bridge inherits them and the log says which server it attached to |
| `--standalone` | the first version's behaviour: this client alone, no port, no lease, no peers, nothing written anywhere |
| degraded | when the lease cannot be written or replaced, or the shared server cannot start, the client is served alone exactly as `--standalone` would, and the log says `cannot use the shared server` with the path and the reason |
| `serve` | the same shared server without a stdio session of its own; when one already runs it logs the URL and exits 0 |

</div>


### One repository, every server of it

A server serves a checkout. A linked worktree is a checkout of its own — it has the
manifest, so it has a root, a lease and a server of its own — and until ADR 0035 nothing
said that two such servers belonged to one repository. Now the index route (`GET /`) names
the git repository the checkout belongs to beside the checkout's own identity:
`commit` is the commit the executable answering was built from, in full or `unknown`, and
`dirty` whether its tree carried uncommitted changes (`null` when the build did not know) —
the same two fields `GET /api/v1/live` and `GET /api/v1/ready` answer; `stale` is `null`
while the executable the process was loaded from is still the file on disk, and says why
once a rebuild has replaced or removed it — the process is then serving code that no longer
exists, and `GET /api/v1/ready` answers `ready: false` with the same reason (I1502);
`repository_id` is the checkout's (a digest of its root, what the lease probe compares),
`git_repository_id` is the repository's (a digest of the git directory every worktree
shares; absent where git cannot be asked), `linked_worktree` says whether this is the
primary checkout, and `leaseholder` says whether the process answering is still the one
(below). `branch` and `head` are the branch and commit the served answers were read from,
as the index read them (I1031). The home page and the Swagger shell at `/swagger` carry the
same identity in one line beside their content; none of it is a path, because whoever can
reach the socket is told which checkout answered and never where it sits.
`GET /api/v1/server` — the tool `majordomus_server`, the resource
`majordomus://server` — lists every checkout git registers for the repository, the primary
first, each with its lease, where its server stands and how many peers it holds, and says
where this checkout's own server stands measured against what this executable would serve:

<div class="overflow-x-auto" tabindex="0">

| standing | meaning |
|---|---|
| `absent` | no lease: nothing serves this checkout |
| `starting` | a lease without an address, young enough that its owner is still binding |
| `ready` | the server the lease names answers for this checkout, from the file on disk, at this executable's version |
| `outdated` | it answers, but from another version, or from a file replaced since it started: everything it says is yesterday's |
| `stale` | the lease names a server that does not answer, or is not a lease at all; the reason says which |

</div>


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
— because its executable was replaced by a rebuild, which the election reads as "it is
serving code that is no longer on disk", or because it is an older release than the client
electing. The installer puts each release in a directory of its own, so an upgrade changes
the executable's path and the rebuild check never sees it; the release check does. One
verdict (`lease::version_verdict`) decides for the election, `serve ensure` and
`server.status` alike, ordering releases by the semantic-version precedence
`release::version::Version` defines, so a server at `1.0.0-alpha.1` is newer than `0.29.0`
and older than `1.0.0-alpha.2` and `1.0.0`:

<div class="overflow-x-auto" tabindex="0">

| the server's release is | a client of this executable |
|---|---|
| the same | attaches |
| older | takes the lease over; the older server hands its sessions on as below |
| newer | attaches, and its `initialize` answer says first that this client is the older one |
| unpublished, or not ordered as a release (not a semantic version, such as a local build tag) | attaches; `server.status` reports `outdated` |

</div>


A newer server is never taken over by an older client, so two releases side by side do not
fight over the lease: the newer one holds it (I2163). The superseded process is not killed. It
keeps its socket, its peer board and the generation of the layer it loaded, and it goes on
serving the sessions it already had until they end. That is deliberate: a client mid-answer
should not lose its server because somebody ran `cargo build`.

Nor, since I2059, does a build take the lease. A process that may hold it — `majordomus mcp`,
and `majordomus serve` without a subcommand — started from a cargo build output (the file
under a directory carrying cargo's `CACHEDIR.TAG`) first replaces its own process image with a
copy of that file, `<profile dir>/served/majordomus-<mtime>-<size>` — a second link to the
build's own file where the filesystem allows one, the bytes otherwise — made once per build,
never replaced, and shared by every server started from it (`served_executable`). The lease names the copy; a
rebuild replaces `target/debug/majordomus` and never the copy, so the server stays `ready`
on the address it had, and a launcher of the new build attaches to it, because a lease
naming another path makes no rebuild claim. The new build serves once something restarts
the server on purpose: `serve stop`, or its idle timeout. Copies older than a week are removed
when a new one is made; a copy that cannot be made is said on stderr and the server runs in
place, as before. An installed release is never copied: its path is already versioned.
`test/cases/3061` holds it, and `test/cases/885` the server run from a file that is replaced.

What it must not do is go on being *the* server. From the moment its own reader finds the
lease no longer carries its token:

<div class="overflow-x-auto" tabindex="0">

| | |
|---|---|
| its index says so | `GET /` answers `leaseholder: false`; every other field — `repository`, `repository_id`, `git_repository_id` — is as true of it as of the current server, which is exactly why one field has to separate them |
| the probe refuses it | `lease::probe` asks three questions, not two: a Majordomus server, this checkout, still the leaseholder. A client holding an address from before the takeover is told there is no server there rather than served a board nobody else can see |
| it takes on nobody new | an `initialize` with no session gets `409 lease_lost`, naming the launcher as the way to the current server |
| it leaves the mesh | discovery and cooperation stop; the successor alone represents this checkout, so the retired process cannot keep replacing its authenticated links |
| its open sessions are handed on | they are answered for the declared busy grace (`server.busy_grace_seconds`), so a request in flight and the ones behind it finish where they started; after it, each session is closed and answered 404, the client's bridge opens a new one, is refused 409 `lease_lost` and elects the current server; with no session left the superseded process ends (I2127) |

</div>


`tests/mesh_lease_retirement.rs` proves the takeover with real servers: the retired
process still answers its attached MCP client, stops participating in the mesh, and the successor
synchronizes with a remote runtime without repeated reconnects.

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

<div class="overflow-x-auto" tabindex="0">

| the question | the answer | who asks it |
|---|---|---|
| Is a **surface's** producer's output on disk? | `http::Served::ready(surface_id)` — the directory a producer writes into has files in it; a route this executable answers is always ready | the home page (`GET /`), which renders a surface with no output as `not built` rather than serving a 404 |
| Can **this process** answer a request? | `health.ready` — `GET /api/v1/ready`: the registry and the index it built at start-up, how the layer read, and whether the executable it was loaded from is still on disk — a replaced one makes it not ready, with the reason in `stale` — and whether this process still holds the checkout's lease: a server whose lease another process took is not ready either. Every reason is listed in `reasons` as `code: words` (`lease_lost`, `stale`, and `layer_degraded`, which does not make it not ready: a degraded layer is served as read, with its diagnostics). `health.report` carries the mesh as a dimension of its own, judged as `mesh doctor` judges the runtime, and `unknown` in a process no server runs in. Local initialisation only | a hosting platform's readiness probe. It contacts nothing outside this process on purpose: a readiness check that probes a dependency fails a deployment for something that is not this process |
| Does **anything** accept a connection at the address the lease published? | `environment::ServiceAvailability` — one TCP connect with a hard budget and no name resolution (`environment::probe::reachable`) | the environment snapshot, which runs on a shell prompt (`majordomus env`, `.envrc`) and may not spend an HTTP round trip or reach DNS to say what it knows |
| Is what answers there **current**? | `ServerStanding` — `server.status`, from `lease::probe` (this checkout's identity, over HTTP) and the version and executable the lease carries | anyone who has to trust what the server says: `serve ensure`, `serve stop`, the `server` check of `health.report`, and the session briefing |

</div>


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
only when the election would take the lease over (the same executable, replaced on disk, or
an older release than this executable) and is otherwise reported with the remedy, because a server of another build that answers
is not this command's to end. Run twice, it starts nothing the second time; run by three
shells at once, the election lets one of the three servers bind and the others defer. The
call is bounded by `--wait`; a server that did not become ready in time is reported with
the standing it reached and exit 10.

A server `ensure` starts has no client of its own. It ends when nothing has used it for
`--idle` seconds (fifteen minutes by default) — no MCP session attached, no HTTP request
answered and no live `/events` channel open, so a person reading the Cockpit or calling the
REST API keeps the server they are using (I2131) — which is what keeps ADR 0003's line —
there is no process without a client — true in time rather than at every instant: an
agent's entry is owed a server before its first attach, and a checkout nobody works in
does not keep one.

What keeps it is a client, not the machine (I4007). Every server on a machine hears the
mesh's multicast and links to the others, a board gather reads every sibling checkout's
board, and `serve status`, `serve ensure` and the gather probe every lease's server; when
each of those requests counted as use, the servers of one Mac kept each other alive — fifteen
of them, for up to eight hours, with no peer attached. So the idle timer, in
`apps/majordomus-cli/src/idle.rs`, decides from what the server holds: no MCP session and no
row on its peer board, no execution queued or running, no live `/events` channel, and no
request from a client since its last look. The mesh transport (`/api/v1/mesh/link/hello`,
`/api/v1/mesh/link/sync`, `/api/v1/mesh/register`), a probe of `/` (which this executable
marks with `X-Majordomus-Probe`) and a sibling's `GET /api/v1/peers?checkouts=this` are
not use. When the bound passes, the server logs the reason (`idle for N s (bound N s): no
peer attached, no execution queued or running, ...; stopping`), runs the same ordered stop
`serve stop` asks for — sessions and episodes closed, listeners closed, mesh drained, lease
released — and exits 0. It leaves on its own; nothing signals it (ADR 0200). The bound is
`server.idle_seconds` below; a server started without one — plain `serve`, a deployment,
the server `majordomus mcp` hosts for its stdio client — never idle-exits.
`test/cases/4007_an_idle_server_leaves_by_itself.sh` holds it.

`serve stop` signals the server the lease names, when that server answers for this
checkout, and waits for *that server's* lease to go — the document it read, by its token,
not merely the path — and then for the process to exit. The `SIGTERM` it sends is the
ordered stop and not a kill: the server gives attached peers two seconds to leave, closes
their sessions and the episodes they carried, closes its listeners (an answer in flight is
finished first), releases its lease and drains the mesh. Only the process whose token the
file still carries removes it: every removal and every rewrite of the lease re-reads it
under the lock beside it (`.ai/local/state/mcp/server.lock`), so a server that is stopped
just after its lease was taken over leaves its successor's lease alone. A `serve` then
exits 0 by its own return; a stop that does not finish within eight seconds, or a second
`SIGTERM`, still ends it. A `serve ensure` still waiting in the election takes the freed path
within milliseconds, so waiting for the path would report a server that would not stop
when it had already stopped; the take-over is named in the answer instead. A lease that
names a server of another checkout, or one that does not answer, is left alone and said
so; nothing here kills a process that was not asked for by name.

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
meantime, and two electors that judged one lease stale end with exactly one lease, because
each re-reads the file under the lock beside it before removing it; an owner whose lease was
taken over while it was binding refuses to publish and degrades, rather than writing over the
winner's address, and it publishes through a temporary file of its own, synced before the
rename; and a server whose lease is taken over later stops claiming it — nothing it does on
its way out touches the file, which is somebody else's — serves the peers it has for the busy
grace, hands each of them on to the current server, and ends. The server's own reader also
forgets the HTTP sessions that stopped pinging on every path, not only while the owner
waits for peers to leave, so a dead peer never stays `attached` on the board.


### The machine names the interface

A local server binds loopback. That default does not move: the layer, its diagnostics, its
peers and the two tools that answer about `.ai/local/` are not for every host on the
network a laptop happens to be on.

One machine may still want to be reached — a second machine attaching over the LAN, a mesh
peer dialing in, a phone opening the Cockpit — and until 0.14 the only way to say so was
`majordomus serve --host 0.0.0.0`, typed by hand. That lasted exactly as long as the
process: `serve ensure`, which is what a session start and a shell entry run, starts a
server with no host at all, so the next idle stop or the next session put the checkout back
on loopback without a word, and `serve status` had been reporting `desired 127.0.0.1` the
whole time.

So the interface is read from one place every starter inherits:

```sh
export MAJORDOMUS_HTTP_HOST=0.0.0.0     # every interface; or one address of this machine
```

<div class="overflow-x-auto" tabindex="0">

| who starts the server | what it binds |
|---|---|
| `majordomus serve` | `--host` when given; else `MAJORDOMUS_HTTP_HOST`; else `127.0.0.1` |
| `majordomus mcp` (a client electing itself) | `--http-host` when given; else `MAJORDOMUS_HTTP_HOST`; else `127.0.0.1` |
| `majordomus serve ensure` (session start, shell entry) | it passes no host; the server it starts inherits the variable |
| `majordomus serve --deployment <id>` | the deployment object's `listen` block, and nothing else: a declared address is not overridden by a machine's environment |
| any of the first three, in a linked worktree | `127.0.0.1` when only `MAJORDOMUS_HTTP_HOST` names an interface; a flag still binds where it says. The variable is the machine's face on its network, which is the primary checkout's server; a worktree's server serves its own sessions, and its mesh commands go through the primary's ([`MESH.md`](@/docs/mesh.md#configuration)) |

</div>


Four things follow from it being a variable and not a setting:

- **It is the machine's, never the repository's.** Nothing tracked can set it. A key in
  `.ai/repo/policy.yaml` would make every clone of a public repository listen on whatever
  network it woke up in.
- **The command line wins**, a blank value is no value, and a value nothing can bind
  refuses to start, naming the address.
- **The warning stands.** A bind the variable chose still logs that every host reaching the
  interface can read the layer; the log line before it says the variable named the address,
  so a bind is never untraceable. Only a deployment object *declares* an exposure, and only
  a declared one is silent.
- **`server.status` answers for the environment it is asked in.** `desired.host` is what a
  server started from that environment would bind, so the address a server binds and the
  address the status calls desired are one resolution (`cli::resolve_http_host`) and cannot
  disagree. A running server answers `serve status`, and it answers for the environment it
  was started in.

Set it where every starter will see it — the shell's own startup file (`~/.zshenv`,
`~/.profile`). `.envrc.local` is too late for one of them: `.envrc` evaluates
`majordomus-env enter`, which ensures the server, *before* it sources `.envrc.local`, so
the server a first `cd` starts would not have the variable while everything started from
that shell afterwards would.

A server already running keeps the address it bound. After setting the variable,
`majordomus serve stop` and the next `serve ensure` — or the next session — brings it up
on the named interface.

There are no accounts and no tokens on this surface, so a request is judged by where it
came from (ADR 0126). Binding beyond loopback hands every reachable host the read surface
and nothing else: a request from an address that is not loopback runs a capability only
when the capability reads (`effect: read`) or when its input is a signed message the
handler verifies against the trust policy — the mesh's `link.hello`, `link.sync` and
`register`. Every other route, and every MCP tool that changes something, answers that
caller `forbidden` (HTTP 403) before its handler runs, including `executions.start`, which
would otherwise run any capability. The decision is per request, not per session: an MCP
session id is a name, not a proof. Reads are still served to the whole network, unencrypted;
to write from another machine, reach the loopback server through an SSH tunnel
(`ssh -L 8741:127.0.0.1:8741 <machine>`), whose requests arrive from loopback.

A request from loopback must name this server by an address or `localhost`: one that names any
other host is refused `forbidden` before any surface answers, reads included, because that is
what a page whose domain was made to resolve to this machine (DNS rebinding) sends. A request
from another host may name the machine as it knows it. `/swagger` is served with a
Content-Security-Policy that admits only its pinned CDN files, whose integrity hashes the page
names, and its one inline script.

### What a contest is judged by

Four numbers decide which process owns the lease, and a fifth keeps the owner able to
answer. They are **declared**, in `.ai/repo/policy.yaml`'s `server:` block, and read once
when the repository is opened: Both `mcp` and `serve` read it before their first election, so the values below are the ones every election in the process uses (I2129); before that, a value read before the policy was the compiled default for the life of the process.

<div class="overflow-x-auto" tabindex="0">

| key | what it decides |
|---|---|
| `probe_timeout_seconds` | how long a probe waits for the current owner to answer before that silence counts as evidence. Too low and a live but slow owner is judged stale and taken over **while it is still serving**; too high and a wedged one holds the checkout's server for that long |
| `bind_grace_seconds` | how long a lease naming no URL yet is left alone — a server writes its lease before it can serve, so a fresh lease without a URL is a *starting* owner, not a dead one |
| `join_timeout_seconds` | how long a process waits to join or create the lease file before refusing rather than waiting forever |
| `busy_grace_seconds` | how long a live owner that does not answer is waited on, and asked again, before its lease is taken over. The election gives a busy owner this long, and so does `serve ensure` before it starts anything: a probe that times out against a live process is not a dead server, but one that stays silent this long is wedged and replaced. A server that `ensure` started after waiting out this grace is told which lease it judged, so its election does not wait on it a second time |
| `read_deadline_seconds` | how long the server lets a connection go without a request read in full on it — waiting for a head, kept alive between requests, or reading a body — before it closes the connection. A client sending one byte a second would otherwise hold a request thread forever, and enough of them would leave the probe above unanswered. A connection whose request is being answered, or that was upgraded to the live channel, is never closed by it. Each close is counted in `server.status` (`connections.closed_by_deadline`), beside the requests refused `503` because all 64 request handlers were busy (`connections.refused_busy`); the probe, `GET /`, is answered even then. The HTTP library keeps a thread on each connection while it lives and can queue a connection that arrives in a burst until one is free; the deadline is also what bounds that wait (I2156) |
| `idle_seconds` | how long a server started for a checkout no client owns — by `serve ensure`, or by a shell entering the checkout — outlives its last peer before it stops by itself; 900 when it is not declared. `--idle` on the command line overrides it. A server started by hand with plain `serve` and no `--idle` runs until it is stopped, as before: its owner is whoever started it (I2701) |

</div>


Until 2026-09-15 all of them were compiled constants in `apps/majordomus-cli/src/lease.rs`:
unchangeable without a rebuild, stated nowhere a reader would look, and invisible to every
projection — while `probe_timeout` is precisely the number that decides whether a live owner
keeps what it owns. That is a decision about how a repository is supervised, not an
implementation detail.

The schema (`share/schemas/majordomus/policy/policy.v1.schema.json`) owns the block with
`additionalProperties: false`, so a misspelt timing is **refused** rather than read as absent;
`test/cases/354_a_lease_contest_is_judged_by_a_declaration.sh` holds that, and
`lease::Timings::from_policy` carries the doc test proving a declared value is the one used
and that a key the policy omits keeps its constant. The values shipped are the constants they
replaced, so declaring them changed no behaviour — deliberately: a change to how a contest is
judged should be visible as a change, not arrive inside a refactor.

## Starting it from a client

The root of this repository carries the configuration each client reads, all naming the
same launcher, so that the first client to open the repository becomes the server and the
others attach:

<div class="overflow-x-auto" tabindex="0">

| client | file | what it names |
|---|---|---|
| Claude Code | [`.mcp.json`](https://github.com/korczis/prismatic-majordomus/blob/@source-ref@/.mcp.json) | a stdio server, `bin/majordomus-mcp`; Claude Code asks once whether to trust a project server |
| Gemini CLI | [`.gemini/settings.json`](https://github.com/korczis/prismatic-majordomus/blob/@source-ref@/.gemini/settings.json) | the same launcher under `mcpServers.majordomus` |
| Codex | [`.codex/config.toml`](https://github.com/korczis/prismatic-majordomus/blob/@source-ref@/.codex/config.toml) | `[mcp_servers.majordomus]`, loaded when the project is trusted |
| bb | nothing of its own | an orchestrator: the agent it starts (Claude Code, Codex, an ACP agent) reads its own file above, so a bb thread attaches through the agent, not through bb (ADR 0024). Claude Code under bb runs with `settingSources: project`, which loads `.mcp.json`; a server loaded from a settings file gets two seconds before the first turn, so a cold checkout that has to build the executable shows it `pending` at init and connected afterwards |
| anything speaking Streamable HTTP | the running server's `/mcp` | `initialize` answers with an `Mcp-Session-Id`; every later request carries it; `DELETE /mcp` ends the session; an idle session expires and the client re-initialises on the 404, as the transport prescribes |

</div>


The provider rows are the providers whose declaration names a client configuration; the whole
set, with what each reads and where it keeps its scratch checkouts, is
[`docs/generated/providers.md`](https://github.com/korczis/prismatic-majordomus/blob/@source-ref@/docs/generated/providers.md), generated from the same declaration.

`bin/majordomus-mcp` never builds (ADR 0224, which supersedes ADR 0172). It runs this
checkout's executable (`MAJORDOMUS_BUILD_PROFILE=release` for the release build; `MAJORDOMUS_BIN`
names another), sets `MAJORDOMUS_SHARE` to the distribution's `share/` beside it, writes nothing
to stdout, and passes every argument to `majordomus mcp`. When that executable is missing or older
than its sources, the installed release (`MAJORDOMUS_INSTALLED_BIN`, by default `majordomus` in
`~/.local/bin`, where `install.sh` puts it) answers instead only when it is of this tree's
generation: the commit its `RELEASE.json` names is in this repository and carries every crate
input of the working tree unchanged (`mj_rust_same_generation` in `lib/rust_bin.sh`, the rule the
gates borrow an executable on). It says so on stderr. A version string is not a generation, so any
other release is refused: exit 12, naming why and the explicit build, `just build`, which the
client reports as a failed server — a session told it has no server can be fixed, one served by a
release of another crate answers for a tree it was not built from. The same answer holds for every
command of the launcher, not `mcp` alone. A file inside the repository is never taken for the
installed release, nor one that does not answer `--version`. The case is
`test/cases/3177_a_client_always_gets_a_server.sh`; every entry path and gate is held to it by
`test/cases/3958_no_entry_path_or_gate_builds.sh`.

`just mcp`, `just serve`
and `just inspect` at the root do the same for a person; `just docs-ui` opens the running
server's Swagger UI. The same document, with its tags, examples and statuses, is the API
reference on the site at `/docs/api/` and is served raw at the site's `/openapi.json`; the
`initialize` instructions, the OpenAPI `info` and `GET /` open with the one summary from
`about.rs` ([`CAPABILITIES.md`](@/docs/capabilities.md)).

### A session that starts by joining

A hook can only add context; nothing but a first message starts a turn. So a session that is
merely opened is handed the briefing and waits for a person to type, and until someone does
it is not on the board (ADR 0226, ADR 0172). `share/shell/session-start.sh` closes that for
Claude Code: sourced from `~/.bashrc` or `~/.zshrc`, it wraps a **bare** `claude` so that,
inside a repository with an `.ai/` layer, the first message is
`majordomus prompt render session-start` — the repository's own prompt
([`.ai/repo/prompts/session-start.md`](https://github.com/korczis/prismatic-majordomus/blob/@source-ref@/.ai/repo/prompts/session-start.md)): say how you are
attached, read the board, find the work that is owed, check for collisions, announce, then work
inside the task's scope and keep everything local. Any argument is the person's own and is
passed through, so is a run outside a supervised repository, with `MAJORDOMUS_NO_AUTOSTART=1`,
or when the prompt cannot be rendered. The prompt is the repository's, so changing it is a
commit and every checkout that pulls it is updated; the file ships in the release's `share/`
and is updated with it. `test/cases/3221_a_bare_session_start_joins_the_cluster.sh` proves it in
bash and zsh. It is not installed for you: the installer touches nothing outside its prefix
(`docs/DISTRIBUTION.md`), so sourcing the file is the one line a person adds.

### Claude in the browser

Three Claude surfaces meet a browser. Only one of them reaches this server today, and the
other two must not be made to look as if they did:

<div class="overflow-x-auto" tabindex="0">

| surface | reaches this server through | reaches a page through | state |
|---|---|---|---|
| Claude Code (terminal, desktop, IDE) | `.mcp.json` above: stdio, then the shared server, as a loopback caller that reads and changes | the Claude in Chrome extension's bridge: the `mcp__claude-in-chrome__*` tools, on with `claude --chrome` or `/chrome` | works: the one path that drives a page and the live mesh in one session |
| Claude in Chrome side panel | nothing yet | itself | an agent of its own on claude.ai, which calls only a connector Anthropic's cloud reaches over the public internet; a server on loopback or a LAN address is out of its reach |
| claude.ai chat (web, desktop) | nothing yet | — | the same: a custom connector or nothing |

</div>


A tunnel from the public internet to the loopback server would give a vendor's requests the
loopback caller's right to change the repository (ADR 0126 judges a caller by its address), so
none is set up. The side panel and the chat reach the mesh when a vendor caller has a listener
of its own that authenticates it and serves reads only (I3285, which covers a claude.ai custom
connector as well as a ChatGPT one), and the hosted hub serves the mesh over TLS to an
authenticated caller only (the owner's decision of 2026-10-21: TLS first, then the hub, and no
anonymous caller reads the mesh). Until then a side-panel session reads what is published:
<https://majordomus.dev> and its `/ui/`, a snapshot built without a running mesh, which is why it
says the mesh is not observed. To work on the live mesh from a page, drive the page from Claude
Code.

Set it up once, per person rather than per repository:

- In Claude Code run `/chrome` and choose **Enabled by default**. It is a user setting: Claude Code
  ignores `CLAUDE_CODE_ENABLE_CFC` in a project's settings ("a project can't turn on Claude in
  Chrome", Claude Code documentation, `code.claude.com/docs/en/chrome`), so this repository does not try.
- Sign the extension in to the claude.ai organization that `/status` names in Claude Code. Claude
  Code needs a claude.ai login (Pro, Max, Team, Enterprise); an API key or a `setup-token` keeps
  the bridge off.
- The first enable writes the native messaging host — on macOS
  `~/Library/Application Support/Google/Chrome/NativeMessagingHosts/com.anthropic.claude_code_browser_extension.json`,
  which runs `claude --chrome-native-host` — and Chrome finds it after a restart.

Every browser action still passes the extension's own per-site permissions; a rule in
`.claude/settings.json` does not lift them, and this repository pre-approves no browser tool.

When it does not connect — `list_connected_browsers` answers an empty list, or a browser tool says
"Browser extension is not connected":

1. The extension is not running, or it is signed in to another account or organization than
   `/status` names. Sign it in again with that organization.
2. `/chrome` → **Reconnect extension**, then restart Chrome and Claude Code.
3. An organization IP allowlist that blocks `bridge.claudeusercontent.com` blocks the bridge
   (Claude Code documentation, `code.claude.com/docs/en/network-config`).
4. A JavaScript dialog open in a controlled tab stops every later browser event until a person
   dismisses it.

The mesh half is diagnosed as always, from the same session: `majordomus mesh doctor`, and
`majordomus_peers` for the board.

## Peers

Every session is a peer: the server's own stdio client, every bridged `majordomus mcp`,
and every client speaking `/mcp` directly. A peer is named by what its client sent in
`initialize` (`clientInfo.name` and `version`: `claude-code`, `codex`, `gemini-cli`, or
whatever the client calls itself), numbered `p1`, `p2`, ... in attachment order, with the
transport, when it attached and when it was last seen. The `initialize` result's
`instructions` tell a client the server's URL, its own peer id and every other peer with
what it announced, before its first tool call.

A peer's row also carries its `address` (I3956): where the worker behind it can be reached
from outside its connection — the machine, the worker's own process (`CLAUDE_PID`, else the
parent of the process it started), the provider's session id and the socket other sessions
message it through (`CLAUDE_CODE_SESSION_ID`, `CLAUDE_CODE_MESSAGING_SOCKET`). It is derived
from the environment the provider hands the process it starts, never announced by hand: the
server's own stdio client is located from the server's environment, and a bridged
`majordomus mcp` carries its own under `_meta["dev.majordomus/address"]` in the `initialize`
it forwards. The provider's messaging token is a credential and is never read. A client that
exposes none of it is still named by its machine and its process. The provider and session
id together spell the mesh session the edit guard judges the provider's claims under
(`claude-code-<session id>`), which joins the board row to the mesh.

Two tools serve the board: `majordomus_peers` reads it (`peers.list`) and
`majordomus_announce` writes to it (`peers.announce`). Their arguments and answers are the
registry's, and this page does not repeat them: the generated reference
([`generated/capabilities.md`](https://github.com/korczis/prismatic-majordomus/blob/@source-ref@/docs/generated/capabilities.md), `/registry/mcp/` on the site) and
`majordomus_capability` at runtime carry them.

An announcement is one line of intent and the repository-relative paths the peer expects
to touch. The board lives in the server's memory and is gone with the process;
`peers.announce` is of kind `command`, because it changes that memory,
and it is announced to MCP clients as not read-only. Over plain HTTP there is no caller,
so `POST /api/v1/peers/announce` is refused (422) and `GET /api/v1/peers` answers without
a `caller`.

### The board is the repository's (ADR 0044)

A server serves a checkout, so a repository worked on through linked worktrees has one
board per worktree — and until ADR 0044 `peers.list` answered with one of them while the
bootstrap in `CLAUDE.md` and `AGENTS.md` told every worker it had seen the repository.
Measured on 2026-09-11: seven live servers, one `git_repository_id`, seven boards, nine
agents across sixty worktrees each reading a board that held itself.

`peers.list` now gathers. It enumerates the checkouts git registers and reads the lease of
each — the same readers `server.status` uses — takes its own board from this process's
memory, and asks every other checkout whose server answers for **its own board alone**
(`checkouts=this`). That parameter is what keeps the gather one hop deep: a server asked
for `this` enumerates no checkout and probes no server, so it can never ask back. There is
no retry without it, because a server too old to know the parameter would answer its whole
board and a cycle is the one failure this must not have; such a checkout is reported unread
with the reason instead.

<div class="overflow-x-auto" tabindex="0">

| field | what it says |
|---|---|
| `peers[].checkout` | the checkout a peer is on — its id, worktree, branch, and whether it is this one. Not decoration: `p1` is the first session of *every* board, so a merged listing without it holds several `p1`s |
| `overlaps` | every pair of claims that meet, now across checkouts as well as within one, each naming the checkout the other worker is on |
| `covered` | how many checkouts the answer covered, listed in `boards` or counted in `omitted` |
| `boards` | one entry per checkout the answer lists, reached or not, with its standing and the reason it was not read. Under the default `view: summary` that is this checkout, every checkout with a worker or a standing server, and every board that could not be read; `view: full` lists every covered checkout |
| `omitted` | the covered checkouts the summary counts rather than lists, by standing (`absent`, `stale`): no server, nobody attached, no board. On 2026-10-22 that was 601 of 625, and the default answer went from 168 kB to about 18 kB |
| `complete` | whether every board covered could be read. `false` means a checkout could not be asked — a short board is never to be mistaken for an empty repository |
| `checkouts` (input) | `repository` (the default) or `this`: this checkout alone, enumerating nothing, probing nothing, reading one board out of memory |
| `view` (input) | `summary` (the default) or `full`, the same words as `majordomus_mesh_peers` |

</div>


A peer id is a **position on one board**, handed out in attachment order and reassigned
after a reconnect — a session that was `p3` this morning is `p1` once its bridge
re-attaches. Nothing may correlate a worker across time by it; the checkout a peer carries
is the durable half of its identity. An announcement likewise belongs to a connection: the
bridge repeats its client's last one after a re-attach or a takeover, and a worker whose
bridge process is replaced announces again.

Costs one lease read per registered checkout, one probe per checkout whose lease names an
address, and one round trip per server that answers. On this repository on 2026-09-11 —
118 registered checkouts, 7 live servers — the enumeration and probing `server.status`
already performs took 3.0s wall.

**A claim is answered, not merely recorded.** `peers.announce` compares the scope it is
given against every other announcement and returns the peers whose claims meet it, with
the pairs of paths that meet: two claims meet when they are equal or one is inside the
other (`apps` contains `apps/majordomus-cli`; `app` does not, because a claim is a path
and not a prefix of a string). `peers.list` reports the same collisions across the whole
board, each pair once. It is still not enforcement, and neither is the shell tool's
`check --overlap`, which reports other worktrees' task scopes and exits 0. What refuses is the
task's own scope from `start --scope`, at `check`, at `finish` and in the pre-push hook — but a
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
([ADR 0103](https://github.com/korczis/prismatic-majordomus/blob/@source-ref@/.ai/repo/adrs/0103-every-client-gets-an-episode-and-every-provider-capability-cites-its-evidence.md)).
Until it, drawing the boundary below the model was wired for Claude Code alone, and the
repository's claim to do so was a claim about one vendor.

A connection holds an episode with `majordomus_session_attach` (`episodes.attach`), closes it
with `majordomus_session_detach` (`episodes.detach`) and reads them with
`majordomus_episodes` (`episodes.list`); their arguments and answers are in the generated
reference ([`generated/capabilities.md`](https://github.com/korczis/prismatic-majordomus/blob/@source-ref@/docs/generated/capabilities.md), `/registry/mcp/`).

<pre class="mermaid">
stateDiagram-v2
  [*] --&gt; Attached: initialize, a peer attaches and no episode opens
  Attached --&gt; Open: episodes.attach opens the episode, or resumes this client's own
  Open --&gt; Open: every message is the episode's heartbeat
  Open --&gt; Detached: the connection goes, detached and not closed
  Detached --&gt; Open: episodes.attach again resumes the same episode under a new peer id
  Open --&gt; Closed: episodes.detach closes it deliberately into a session record
  Detached --&gt; Closed: no reconnect in 15 minutes, the reaper closes it as interrupted
  Open --&gt; Closed: the server stops, closed as shutdown
  Detached --&gt; Closed: the server stops, closed as shutdown
  Closed --&gt; [*]
</pre>


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
the repeated semantic definition [`CAPABILITIES.md`](@/docs/capabilities.md) forbids. The
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

<div class="overflow-x-auto" tabindex="0">

| decides | read from |
|---|---|
| where the layer is | the nearest ancestor holding `.ai/manifest.yaml`; `.git` and `.majordomus/` are not markers |
| which sections exist | `sections:` in the manifest |
| which files are sources, of which kind | `.ai/repo/knowledge/sources.yaml`, one pathspec and kind per class, through the git index |
| how a kind is read and which keys it may carry | `share/kinds.yaml` and `share/schemas/<kind>.schema.json` in the distribution, plus a repository's own under `.ai/repo/knowledge/` |
| which tools exist | the executable capabilities with an MCP tool exposure, composed under `apps/majordomus-cli/src/capability/builtin/` |

</div>


Consequences a repository can rely on:

- a new rule, prompt, profile, milestone, issue, claim or document is served as soon as
  `git add` has put it in the staging index, with no change to the executable and no
  restart: staging moves a file the server watches, so the next request rebuilds the layer
  (`project.the-server-sees-the-current-tree`);
- a new class in `sources.yaml` naming a known kind, or a new kind with its schema under
  `.ai/repo/knowledge/`, is served the same way;
- `.ai/local/` is never served, tracked or not;
- the YAML read is the subset [`SCHEMAS.md`](@/docs/schemas.md) defines, so a file the shell tool
  refuses is refused here with the same line named.

## Resources

<div class="overflow-x-auto" tabindex="0">

| URI | content |
|---|---|
| `majordomus://repository` | JSON: root, layer schema, sections, git state, discovery mode, kinds present, every diagnostic |
| `majordomus://scope` | JSON: the repository scope as read, its origin, and every tracked file tallied against it ([`SCOPE.md`](@/docs/scope.md)) |
| `majordomus://<kind>/<identity>` | the file as read, `text/markdown` or `application/yaml` |

</div>


A URI is resolved once, by one function, wherever it is asked for: `resources/read`,
the `majordomus_get` tool and `GET /api/v1/object` answer the same URI alike. A URI the
registry does not know is not found on every one of them. `majordomus://repository` is a
query (`repository.info`) with a resource exposure: read as a resource it is the report
as JSON text; read through `majordomus_get` it is the report as data (`answer`) with the
same text beside it (`content`). The registry refuses a query exposed as a resource whose
input requires anything, because a read supplies none.

An object also has an *address*, which the URI is not: `majordomus://rule/project.x@1` is an
identity and `/cockpit/objects/rule/project-x-1` is where a reader is sent. The address is
derived from the kind and the identity by one function, so it exists for every object and is
the same on every surface; `majordomus_entity` answers to either spelling, and
`majordomus_kinds` reports any two identities of one kind that reduce to one address.
`majordomus_entity` also names the object's public documentation page (`documentation`:
the projection, `route`, `url`, or the `reason` its kind is not published), read from the
repository's `site/data/publication.toml` and the site's `base_url` — null when the
repository declares no publication.

Identity is the kind's identity fields joined with `@` — `majordomus.scope-integrity@1`
for a rule, `continue` for a prompt, `implementation` for a profile, `M001` for a
milestone — or, for a kind with no identity fields (policy, document), the
repository-relative path. Every listed resource carries `_meta.majordomus` with the kind,
the identity and the provenance: path, directory, the class that discovered it, the
manifest section it falls under, and its size.

## Tools

Every tool is a projection of one capability of the registry, under the name that
capability declares (`objects.list` is served as `majordomus_list`, `worktree.status` as
`majordomus_worktree_status`). The list grows whenever a module is composed, so this page
keeps no copy of it: a hand-written table of tools is the second definition ADR 0004 calls
a design defect, and the copy that stood here had already fallen behind the registry it
repeated. The total reference, with every tool's capability, arguments and answers, is
derived:

- [`generated/capabilities.md`](https://github.com/korczis/prismatic-majordomus/blob/@source-ref@/docs/generated/capabilities.md) — every capability with its MCP
  tool, HTTP route and command (`majordomus generate`);
- `/registry/mcp/` on the site — one row per tool the registry projects, each linking its
  capability's page (case 532 holds it to the registry, no more and no fewer);
- `majordomus_capabilities` and `majordomus_capability` at runtime — the registry of the
  executable that is answering.

Where the sections above and below name a tool, they say why it exists and how it behaves;
what it takes and returns is the registry's. Case
`3062_the_mcp_reference_repeats_no_registry_entry` refuses a hand-written tool row in any
document under `docs/` outside `docs/generated/`.

### Sessions and continuity

Two of the tools answer about `.ai/local/` — the half of the layer that names this machine —
and they are the reason the server binds to the loopback interface. They are served and
never published: no generator writes them into `docs/generated/` and no site page carries
one.

They are `majordomus_continuity` (`continuity.state`) — what a worker resuming *here* would
be handed — and the operator's five: `majordomus_lifecycle_episodes`,
`majordomus_lifecycle_recovery`, `majordomus_lifecycle_runtime`,
`majordomus_lifecycle_providers` and `majordomus_lifecycle_closed` (`lifecycle.*`). Because
no generated page carries them, their arguments and answers are read where they are served:
`majordomus_capability` with the id, or `majordomus capabilities describe <id>`.

The first answers the worker's question and the rest answer the operator's, which is a
different question and not a superset: `continuity.state` follows
`state/session-current.yaml`, and that pointer is a symlink the most recent start event
re-aims. [`CONTINUITY.md`](@/docs/continuity.md) has the model and the whole path.

### The projection, described

`mcp.projection` — the tool `majordomus_mcp`, the resource `majordomus://mcp`, the route
`GET /api/v1/mcp`, the Cockpit page `/cockpit/mcp` and the first lines of
`majordomus mcp --inspect` — answers what this document would otherwise have to list and
keep true by hand:

```bash
majordomus mcp --inspect    # its lines beginning server, protocol, effect, writes and client are this answer
curl -s http://127.0.0.1:8741/api/v1/mcp | jq '{server, protocol_versions, effects, writers, clients}'
curl -s 'http://127.0.0.1:8741/api/v1/mcp?effect=repository_mutation' | jq '.tools[].name'
```

<div class="overflow-x-auto" tabindex="0">

| field | what it is | where it comes from |
|---|---|---|
| `server`, `protocol_versions` | who answers `initialize`, and with which versions | the constants the server answers with; the version is the executable's |
| `transports` | `stdio` and `http`, and the sessions attached over each right now | this process's peer board at the moment of asking |
| `serving` | the methods a request may name; that prompts and notifications are not served | the list the dispatcher consults before looking at a request |
| `tools`, `tool_count`, `effects`, `writers` | every tool (or those of one `effect`) with its capability, effect and hints | the registry; the effect is the capability's classification |
| `resources` | how many capabilities answer a URI, how many objects the layer holds, the URI shape | the registry and the index |
| `clients` | each client the distribution declares a configuration for, and whether the file here is `wired`, `foreign` or `absent` | `share/providers.yaml` and one read of each file |
| `findings` | a configuration that exists and does not name the launcher; no client configured at all | derived from `clients`, each with its remedy |

</div>


Nothing in it is a list of its own, so it cannot disagree with `tools/list`: a capability
that gains a tool is in the answer, on the Cockpit page and in the generated reference at
the next request, with no other file edited.

### What a tool may change

A tool's annotations are not written in the MCP code: they are the capability's
classified hints (`ExecutionPolicy::hints`), which follow from its effect, and the effect
itself is carried in `_meta.majordomus.effect`.

<div class="overflow-x-auto" tabindex="0">

| effect | what a call changes | `readOnlyHint` | `destructiveHint` | `idempotentHint` | `openWorldHint` |
|---|---|---|---|---|---|
| `read` | nothing | `true` | `false` | `true` | `false` |
| `process_state` | this process's memory and nothing outside it (a peer announcing itself) | `false` | `false` | `false` | `false` |
| `repository_mutation` | the repository's own files | `false` | `true` | `false` | `false` |

</div>


The classification is conservative in the direction a caller can survive: nothing that
changes something is announced as safe to repeat, because no handler has declared that a
second call is a no-op, and anything that writes the repository is announced as able to
overwrite or remove. `openWorldHint` is `false` throughout because no handler reaches
beyond the machine: observing the forge and consulting an advisor are commands a person
runs, not capabilities. Which tools write the repository is never a list in this document:
the `initialize` instructions name them, derived from the registry, and so does
`majordomus mcp --inspect`. Each tool carries the canonical id in `_meta.majordomus.id` and
its `inputSchema` and `outputSchema` from the canonical schemas. A refused call is a result with
`isError: true`, the reason as text, and the category as one word in
`_meta.majordomus.error.code` — `invalid_input`, `not_found`, `refused` or `forbidden`, the same word the
HTTP route answers as `error.code`, because both read it from the error itself. A
refused result carries no `structuredContent`: the output schema describes a success. An
unknown tool, method or resource is a protocol error.

## Failure behaviour

Each row names the failure, what a client observes, and what proves it: the tests that drive
it, or a stated limitation that says why nothing does. A row with neither, or one citing a
test or case this tree does not hold, is refused by
`test/cases/3056_every_failure_class_names_its_proof.sh`.

<div class="overflow-x-auto" tabindex="0">

| state | what happens | proof |
|---|---|---|
| no `.ai/manifest.yaml` above the working directory | exit `12`, the start directory named | `test/cases/3015_an_unsupervised_repository_exits_12_from_both_tools.sh` |
| project data under `.majordomus/` and no manifest | exit `12`, naming `majordomus migrate` | `apps/majordomus-cli/tests/repository_discovery.rs::the_pre_ai_layout_is_refused_by_name` |
| manifest or `sources.yaml` malformed, unknown key, unsupported schema | exit `10`, the path and the key or line named | `apps/majordomus-cli/tests/repository_discovery.rs::a_nearer_broken_manifest_is_an_error_not_a_skip` |
| one file that cannot become an object | excluded; an error diagnostic names its path and a stable code; the index is `degraded` and still serves; `--strict` exits `10` instead | `apps/majordomus-cli/tests/metadata_contract.rs::malformed_front_matter_is_named_and_excluded`, `apps/majordomus-cli/tests/mcp_stdio.rs::strict_refuses_a_degraded_layer_before_serving` |
| `git` unusable | `--discovery vcs` (the default) exits `13` naming `--discovery filesystem`; the git block of `majordomus://repository` reads `unavailable` with the reason | `apps/majordomus-cli/tests/index_behavior.rs::without_git_vcs_discovery_is_refused_and_the_walk_still_works` |
| no share directory (kinds and schemas) found | exit `12`, every directory tried named, and `--share` / `MAJORDOMUS_SHARE` named as the remedy | `apps/majordomus-cli/tests/metadata_contract.rs::without_a_share_directory_nothing_starts_and_the_search_is_named` |
| a kind schema or a schema file invalid, or a repository redefining a distributed kind | exit `10`, both files named | `apps/majordomus-cli/tests/external_extension.rs::a_repository_defines_a_new_kind_with_its_schema_and_it_is_served_without_a_rust_change` |
| client closes its pipe | the process ends with `0`, after the last attached peer has left when it was the server | `apps/majordomus-cli/tests/mcp_stdio.rs::a_client_that_closes_its_read_end_ends_the_session` |
| the port is taken | a free port is bound instead; both are logged | `apps/majordomus-cli/tests/mcp_shared.rs::a_taken_port_falls_back_to_a_free_one_and_serve_defers_to_the_running_server`, `apps/majordomus-cli/tests/serve_lifecycle.rs::ensure_takes_a_free_port_when_the_one_it_asks_for_is_taken` |
| the lease names a server that does not answer for this root | it is taken over; `stale lease` is logged with the URL | `apps/majordomus-cli/tests/mcp_shared.rs::a_stale_lease_is_taken_over` |
| the shared server a bridge is attached to dies | the bridge takes over on its next message, or re-attaches to the process that did; the client never re-initialises | `apps/majordomus-cli/tests/mcp_shared.rs::a_bridged_peer_takes_over_when_its_server_dies`, `apps/majordomus-cli/tests/mcp_shared.rs::a_bridged_peer_re_attaches_when_another_process_took_the_lease_first` |
| a bridge cannot take over (its `--strict`, a broken layer) | the client's request is answered with a JSON-RPC error (`-32603`) naming why; the stdio session stays open | `apps/majordomus-cli/tests/mcp_shared.rs::a_peer_that_cannot_serve_the_layer_says_so_instead_of_taking_over` |
| the lease file is corrupt, empty, or has had no URL for longer than the bind grace | it is taken over; `corrupt lease`, `empty lease` or `abandoned lease` is logged with the path | `apps/majordomus-cli/tests/mcp_shared.rs::a_corrupt_lease_is_taken_over`, `apps/majordomus-cli/tests/mcp_shared.rs::a_lease_of_another_schema_and_an_old_empty_file_are_taken_over`, `apps/majordomus-cli/tests/mcp_shared.rs::an_abandoned_lease_is_taken_over_without_waiting_when_it_is_old` |
| the lease cannot be created, joined or replaced (a filesystem refusing writes under `.ai/local/`), or the shared server cannot start | the client is served alone, as `--standalone` would: `cannot use the shared server` is logged with the path and the reason, then `serving this client alone`; no port, no lease, no peers; the layer's own errors still exit as above | `apps/majordomus-cli/tests/mcp_shared.rs::an_unwritable_lease_directory_degrades_to_a_standalone_session` |
| the server gets `SIGTERM`, `SIGINT` or `SIGHUP` | it stops in order: sessions and episodes closed, listeners closed, the lease released if it is still the server's own; `serve` exits `0`, a `majordomus mcp` then dies of the signal; a second signal or eight seconds end it at once; its bridges elect again on their next message | `apps/majordomus-cli/tests/mcp_shared.rs::sigterm_removes_the_lease_before_the_process_dies`, `apps/majordomus-cli/tests/mcp_shared.rs::sigint_and_sighup_remove_the_lease_before_the_process_dies`, `apps/majordomus-cli/tests/lease_holder.rs::serve_stop_ends_the_server_in_order_and_closes_its_episodes` |
| the server is killed with `SIGKILL` | nothing runs on its way out; the lease it leaves names a process that is gone, and the next client takes it over at once | `apps/majordomus-cli/tests/serve_lifecycle.rs::a_stale_lease_and_a_killed_server_are_both_recovered`, `apps/majordomus-cli/tests/lease_busy.rs::a_silent_server_whose_process_is_gone_is_taken_over_at_once` |
| the lease's owner is alive and frozen: it answers nothing | a contender waits the declared grace, then takes the lease over; the log names the owner given up on | `apps/majordomus-cli/tests/lease_busy.rs::a_live_server_that_stays_silent_loses_its_lease_after_the_grace`, `apps/majordomus-cli/tests/serve_lifecycle.rs::a_live_owner_that_never_answers_is_still_taken_over`, `apps/majordomus-cli/tests/lease_declared.rs::a_busy_owner_is_given_up_on_after_the_declared_grace_and_the_log_names_it` |
| the server's lease is taken by another process | the server says so, takes on no new client, and an unbounded server ends by itself | `apps/majordomus-cli/tests/lease_lost.rs::a_server_that_lost_its_lease_says_so_and_takes_on_nobody_new`, `apps/majordomus-cli/tests/cov_runtime_a_server.rs::an_unbounded_server_whose_lease_is_taken_over_ends_by_itself` |
| `--http-host`, `--host` or `MAJORDOMUS_HTTP_HOST` names an address that is not loopback | served, with a warning that every host reaching that interface can read the layer, its diagnostics and its peers; when the variable supplied it, the line before says so | `apps/majordomus-cli/tests/mcp_shared.rs::a_non_loopback_bind_is_warned_about`, `test/cases/992_the_local_bind_is_the_machines_to_name.sh` |
| `MAJORDOMUS_HTTP_HOST` names an address nothing can bind | the process exits non-zero with `cannot bind <address>`, and leaves no lease behind | `test/cases/992_the_local_bind_is_the_machines_to_name.sh` |
| an HTTP client leaves without `DELETE /mcp` | its session expires after ninety seconds of silence; a server whose owner has already left ends then, never later | `apps/majordomus-cli/tests/shared_units.rs::the_endpoint_opens_reaps_and_closes_sessions`, `apps/majordomus-cli/tests/serve_in_use.rs::a_server_read_over_http_outlives_its_idle_timeout_and_ends_when_left_alone` |
| an HTTP client sends its request slowly, or not at all | its connection is closed at the declared read deadline and counted; the other clients are answered | `apps/majordomus-cli/tests/http_slow_client.rs::slow_client_released_test` |
| every request handler is held | a further request is refused at once as busy; the liveness probe is still answered | `apps/majordomus-cli/tests/http_slow_client.rs::past_the_handler_bound_a_request_is_refused_and_the_probe_is_answered` |
| a request handler never returns (a frozen worker) | its worker is held for as long as it runs: the read deadline bounds a request's arrival, not its handling | limitation: not tested — no test freezes a handler, and nothing bounds one; the busy refusal above is what a client meets once every handler is held |
| the disk is full | doctor and the pre-commit hook: one `FAIL disk` naming the volume and its free space, before every other check, each of which is not judged; the lease, the mesh journal and generated output each write through their own error path | doctor's part: `test/cases/4323_a_full_disk_is_one_finding.sh` (an unwritable scratch directory and a floor above the free space stand for a full volume); the writers' part: limitation, not tested — no case fills a filesystem |
| the mesh journal directory refuses writes (read-only) | on master an own event whose append fails is not reported to its caller as not durable | limitation: not tested on master — PRs #1179 and #1196 (I2706, case 3052) report a refused append as not durable and drive it end to end; until they land, no test on this tree does |
| the process is killed between creating the lease and its first write | the empty lease left behind is taken over once it is older than the bind grace, as the row for an empty lease says | limitation: not tested as a kill — no test kills a process at that instant; the state it leaves is the one `apps/majordomus-cli/tests/mcp_shared.rs::a_lease_of_another_schema_and_an_old_empty_file_are_taken_over` builds by hand |
| a lease keep-alive write fails | the failure is dropped and the next tick tries again; nothing is logged or reported | limitation: not tested — `Lease::keep_alive` discards the error by design and no test drives a failing touch |

</div>


Two files of one kind claiming one identity are both excluded and both named, as the
rules contract requires. Nothing is repaired, defaulted or rewritten.

## Not served, on purpose

- **MCP prompts.** The repository's prompt assets render `{{CONTEXT}}` from checkout-local
  state the shell tool owns; served unrendered they would read as finished. They are
  resources (`majordomus://prompt/<name>`), not prompts.
- **The hierarchy of bootstrap files.** Root `README.md`, `AGENTS.md` and the other
  provider files are served as documents with their directory recorded; nothing merges
  or ranks them, because the repository defines no merge semantics. Recorded in
  [`.ai/repo/adrs/0001-rust-cli-and-stdio-mcp.md`](https://github.com/korczis/prismatic-majordomus/blob/@source-ref@/.ai/repo/adrs/0001-rust-cli-and-stdio-mcp.md).
- **A mutation of the repository that is not a capability.** A tool writes the repository
  only when its capability declares `.writes_repository()`; the handler is the one the
  HTTP route and the command line reach, so a refusal there is the same refusal here, and
  MCP adds no writer of its own.
- **Subscriptions, list-change notifications**, and a server-initiated stream on `/mcp`
  (this server sends nothing unasked). The HTTP
  projection of the same registry is served by the shared server and by `majordomus
  serve`; see [`CAPABILITIES.md`](@/docs/capabilities.md).
- **Persistent coordination.** A peer board is one process's memory, and the gathered board
  is a read of several of them at the moment of asking: what a peer is working on across
  sessions and machines is the shell tool's task record and scope, not this. A branch
  pushed before a session started carries no announcement and no less of a claim;
  `scripts/collision-check` is the reader for that.

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
unwritable lease directory degrading to a standalone session, `SIGTERM` stopping the server in
order and leaving no lease behind (`tests/lease_holder.rs`: a signal after a takeover sparing
the successor's lease, and `serve stop` closing the server's episodes and exiting `0`),
malformed traffic on `/mcp`, and the bridge's transparency: a bridged session and a
restarted server answer byte for byte what the first server did. The doctrine behind the
failure table is the rule `project.shared-server-resilience`. `tests/server_status.rs` holds
the two-worktree case and the stale-lease case; `tests/health_server.rs` holds the `server`
check of `health.report` against a real server, a checkout nobody serves and a lease naming
an address nobody answers at, and asserts that the check and the status say one word about
one lease. `test/cases/992_the_local_bind_is_the_machines_to_name.sh` holds the interface:
loopback when nothing names one, the variable followed by `serve`, by the server
`serve ensure` starts and by the one `mcp` elects, the flag winning, a blank value ignored,
the warning kept, `desired.host` agreeing, and an unbindable address refused without a
lease; `85_deployment_bind.sh` holds the declared address beside it. `tests/hot_path.rs` sends
hundreds of frames and requires the startup counters (`majordomus_perf`) unchanged;
`majordomus bench` times every tool through a real child process
([`CAPABILITIES.md`](@/docs/capabilities.md)). The claims are in [`CLAIMS.yaml`](@/guarantees/_index.md)
under `mcp-`, `hot-path-no-rebuild` and `benchmark-coverage-derived`.
{% endraw %}
