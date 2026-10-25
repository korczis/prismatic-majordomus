+++
title = "The peer blackboard"
description = "the peer blackboard: who works on what across every checkout's board and every linked machine, one projection of the board, the mesh journal and the monitor — workers keyed by explicit identifiers, the claims that meet, the reviews and handovers that wait, and what could not be read"
weight = 66
[extra]
source = "docs/BLACKBOARD.md"
+++

{% raw %}

`majordomus blackboard` answers who works on what in a repository: every worker attached to a
checkout's board or holding an active mesh session on any linked machine, what each announced,
the claims it holds, the claims that meet, the reviews and handovers that wait, and whether every
source could be read. It is one projection of records that already exist, computed in one
capability, `blackboard.snapshot`. The decision is ADR 0176
(`.ai/repo/adrs/0176-the-blackboard-is-one-projection-pushed-over-one-socket.md`).

## What it reads

<div class="overflow-x-auto" tabindex="0">

| source | what it knows | where it is decided |
|---|---|---|
| the board of every checkout (`peers.list`) | the MCP clients attached to each checkout's server, their named claims (announcements) | ADR 0044 |
| the mesh journal (`mesh.state`) | the sessions, claims (advisory or exclusive), reviews and handovers of every linked machine | ADR 0067 |
| the monitor (`monitor.stream`) | what happened, folded, with a cursor to resume from | ADR 0133 |

</div>


The blackboard writes nothing and keeps nothing. No surface may join these sources again: a page,
a scene or a report that shows peers renders this answer.

## Quick start

<!-- majordomus:unrun they ask a running server's board; case 3380 runs all three against a server with two attached MCP clients -->
```sh
majordomus blackboard                      # every checkout's board, the mesh, the newest signals
majordomus blackboard --this-checkout      # this checkout's board alone
majordomus blackboard --format json        # the answer itself
curl -s http://127.0.0.1:<port>/api/v1/blackboard?checkouts=this
```

Over MCP the tool is `majordomus_blackboard`. The command line asks the server holding this
checkout's lease, because a board is that server's memory; with no server it composes the answer
itself, and the answer's sources then say what that left unread.

## Workers and identity

A worker is keyed `board:<checkout>/<peer>` or `mesh:<stream>/<session>`. A peer id such as `p1`
is a position on one server's board: every server hands out `p1`, and a restarted server hands it
out again. It is never an identity on its own.

A board peer and a mesh session are one worker only through an explicit identifier. The server
holding a board projects each attached peer into its own mesh stream as the session `board-pN`
(`Cooperation::project_board`), and `peers.list` names that stream for each board. The two are
joined when a session of exactly that stream names the same board session, the same checkout and
the same client. When two sessions fit, each is a `candidate` relation and nothing is joined.
When a board's server names no stream (no mesh, or a server older than the field), its peers are
joined to nothing.

A joined worker keeps its session's live claims beside its announcements. An exclusive
`mesh.claim` taken through the worker's MCP session is therefore on the blackboard. The advisory
copies its server projected from the announcements are not counted twice.

A worker that announced nothing has no `intent`. Nothing is inferred from a branch, a name or a
commit. `last_beat_ms` is the runtime's heartbeat, which shows liveness, not work done.

## Relations, conflicts, what waits

- **Relations** are `same_worker` (`verified` or `candidate`, as above) and `review`. A review's
  addressee is a worker only when the request names a whole session key. A bare `board-p3`
  appears on every board, so it stays the word the record holds.
- **Conflicts** are pairs of claims of two different workers whose scopes meet. `confirmed` means
  both claims are exclusive, or the mesh marked one conflicted. `potential` means at most one is
  exclusive. The answer lists at most 100 and counts them all in `conflicts_total`.
- **Reviews** are the open ones. **Handovers** are the untaken ones, without their bodies, which
  `mesh.state` holds. The journal names no recipient for a handover, so none is inferred.

## Completeness

`complete` is `false` when a checkout's board could not be read or when a mesh the repository
declares is not running. Each such source is listed in `sources` with its reason. The answer
never reads "nobody is here" when the truth is "I could not ask".

- A mesh the repository does not declare, or declares off, is `skipped`.
- The monitor's sources (`monitor.mesh`, `monitor.executor`, `monitor.github`) are reported
  without deciding `complete`. They tell what happened; the boards and the mesh tell who is here.
- GitHub is asked only when `--sources` names it.

## Cost

Measured on 2026-10-18 on the shared development machine, against this repository's journal,
at load averages between 37 and 55:

<div class="overflow-x-auto" tabindex="0">

| reading | time |
|---|---|
| `blackboard.snapshot`, one checkout, 52 workers | about 0.8 s; 1.0 to 1.8 s with `signals: 0` |
| `mesh.state` alone (the full fold of the journal) | 0.45 to 0.6 s |
| `peers.list`, one checkout | 20 ms |
| announcement to frame, live channel, 20 announcements | p50 1.1 s, p95 1.5 s, max 1.5 s |
| announcement to frame, case 3381 fixture (small journal) | 265 ms |

</div>


The live channel's latency is the snapshot's cost plus a 150 ms settle. The snapshot's cost
is mostly the mesh fold, which rereads the whole journal. A target of one second at p95 on
this machine needs an incremental fold. That is not built, and it is planned on its own.

## Following it live

The blackboard rides the one live channel the server already has, `/events`:

<!-- majordomus:unrun it is a WebSocket a running server answers; case 3381 follows it with a real client -->
```text
GET /events?follow=blackboard                               the blackboard, then each change
GET /events?follow=blackboard&checkouts=this                this checkout's board alone
GET /events?follow=blackboard&after=<cursor>&epoch=<epoch>  resumed
GET /events?follow=blackboard&frames=signals&for=1b         the signals alone, marked for 1b
GET /events?follow=blackboard&sources=mesh,executor,github  the signals of these sources
```

`frames` names what a connection is written: `snapshot`, `signals`, or both (the default).
`sources` names the monitor sources of the signals, mesh and executor when not given. `for`
names the reader whose signals are marked `for_you`, as `majordomus monitor --for` does.
Anything else in the query is refused.

The board and the mesh journal advance one change generation whenever they change
(`crate::change`): a peer attaching, identifying, announcing or leaving, and a journal event
written or ingested. A follower waits on that generation. The frames are:

<div class="overflow-x-auto" tabindex="0">

| frame | when |
|---|---|
| `stream.ready` | first; carries the server's `epoch` |
| `blackboard.snapshot` | on connect, and whenever what the blackboard says changed; carries the connection's `cursor` |
| `blackboard.signals` | what happened after the connection's cursor, one frame per page, each with the cursor of its last signal; an empty one after a resync on a connection that follows the signals alone |
| `stream.resync` | the cursor or epoch sent cannot be served; the snapshot after it starts again |
| `stream.lagged` | changes passed while the connection was still being written to; the next snapshot holds them |
| `stream.heartbeat`, `stream.closing` | as on every connection of `/events` |

</div>


- **Trigger.** Every `blackboard.*` frame names its `trigger`. `change` means the board or
  the journal said so. `refresh` means nothing said so and 30 s passed; that is how a source
  that cannot push is followed: a runtime whose beat stopped, the executor's trail, GitHub.
  `connect` is the first frame of a connection.
- **Resuming.** A cursor is good only on the server process that issued it, which is what the
  epoch names. A follower that reconnects sends both back. A cursor of another process, or
  one with more than ten pages of signals after it, is answered with `stream.resync` and a
  fresh snapshot. Every snapshot carries the cursor, so a client that was sent no signal
  resumes from it too.
- **Frames fit.** A page of signals whose frame would pass the 256 KiB frame bound is read
  again with half as many, so the cursor only advances through signals that were written. A
  single signal larger than a frame is a resync, never a closed connection.
- **One read for every tab.** One read of the blackboard per generation, and one page of
  signals per cursor, size, sources and reader, are shared by every connection.
- **Slow clients.** A slow client blocks only its own connection, is told it lagged
  (`stream.lagged`), and is written the newest snapshot, not a queue of old ones. Whether it
  hung up, and the heartbeat, are asked on every round, so a stream of changes that say
  nothing new keeps no departed client's thread.
- **Large snapshots.** A snapshot too large for one frame is written as a `withheld` reference
  to `GET /api/v1/blackboard`.
- **For whom.** The snapshot is reader-neutral, so one read serves every tab. Signals are
  marked for the reader the connection names with `for`, by the monitor's own rule.
- **Who may follow.** A page of another origin is refused, as on `/events`. A caller beyond
  loopback follows the blackboard exactly when it may read `blackboard.snapshot` (ADR 0126).

## In the browser

`share/cockpit/socket.js` is the one WebSocket client of the browser. On a blackboard channel
it keeps the cursor of the last `blackboard.signals` frame and the epoch of `stream.ready`,
reconnects with both, and hands a `stream.resync` to the page.

`/cockpit/monitor` is the first page on it. The server renders the first rows and the channel
that continues from their cursor (`frames=signals`, all three sources, the page's reader), and
the server pushes each signal as it happens. A page whose channel does not open within five
seconds closes it, asks `/api/v1/monitor` every five seconds while visible, and says
"Polling" where it would say "Live". A GitHub signal reaches an open page with the next change
or refresh: GitHub is read through a cache shared for 30 s, and cannot push.

`/ui/peers` is the screen of the blackboard. It is rendered by the server from the read the
channel's followers share, and it follows `frames=snapshot` through `share/ui/live.js` and a
byte copy of the same client (`share/ui/socket.js`). Every snapshot frame reads the screen
again, a connection's first one too: a change made between the page being served and its
channel opening would otherwise never reach a page that says it is live. One reading runs at a
time, and a frame that arrives during one asks for one more after it. A reading of the
blackboard is current only at the blackboard's own change generation, so a screen read after a
change frame never shows the blackboard from before it. Its state line says live,
reconnecting or closed, and while the channel is not live the UI's minute refresh reads it
instead. The UI's published snapshot withholds it (`docs/MAJORDOMUS_UI.md`).

`/cockpit/peers` renders the same answer, asked as the channel's followers ask it (every
checkout, no signals). It shows every worker with where it was found, whether it is here, its
client, its intent and its claims, then the claims that meet, what waits, the typed relations,
every source and every checkout's board. It follows the same `frames=snapshot` channel through
`share/cockpit/peers.js` and the one client. Every snapshot frame reads the page again from its
own address, a connection's first one too, and the part the page marks `data-mj-peers` is
replaced by the same part of that reading. One reading runs at a time, and a frame that arrives
while the page is hidden is read once the page is shown. Each reading asks the capability
again, so a reading made after a change frame is never from before it. The state line says
live, reconnecting or offline, and says so when the last reading failed. The page does not
poll: while its channel is not live it is the reading it last made. The Cockpit's published
snapshot carries no script, so there the page is the reading it was rendered from.

## On the published site

The published site is files, read once at the commit it was built from, and it follows no
server: nothing on it is live, and it carries no blackboard. The UI's snapshot withholds
`/ui/peers` until a public-safe form of the blackboard exists, labelled with the revision and
time it was read (I2984).

The site's home page has a section on the blackboard. It says what the blackboard shows, names
`/ui/peers` and `/cockpit/monitor` as the pages a running server follows it live on, says that
the site itself follows no server and names the commit it was built from, and lists the issues
under [Not yet](#not-yet) with the title, page and status the plan gives each. Its words are
`site/data/marketing.toml`'s. The issues are named once more in `site/data/homepage.toml`
(`[blackboard] waits`), so that the page reads each one from the plan rather than saying how it
stands, and `test/cases/3387_the_home_page_explains_the_blackboard.sh` holds that list and this
one to each other and finds nothing on the page that is live or says it is.

## Not yet

These are planned in milestone `peer-blackboard` and are not built:

- the home page's panel;
- the blackboard's public-safe published snapshot, and with it `/ui/peers` on the published
  site (I2984);
- acting from `/ui/peers` on what waits: answering a review, taking a handover, releasing
  one's own claim, each through its capability and shown only when the blackboard shows it
  (I2999).

Until they exist, nothing on this page claims them.

## Providers

A provider appears on the blackboard only through an interface that proves it is there: an MCP
initialize on a checkout's board, or a session it opened on the mesh. The client name is the
provider's own and no surface keeps a list of providers. A provider that offers no supported,
authenticated event source is not shown as present. This includes OpenAI Dots, which today offers
no such source to a local runtime. A GitHub event is a GitHub signal, never a provider's
presence.
{% endraw %}
