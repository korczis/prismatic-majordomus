+++
title = "The peer blackboard"
description = "the peer blackboard: who works on what across every checkout's board and every linked machine, one projection of the board, the mesh journal and the monitor — workers keyed by explicit identifiers, the claims that meet, the reviews and handovers that wait, and what could not be read"
weight = 65
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

One answer for one checkout on the shared development machine, with 52 workers and this
repository's journal, took about 0.8 s (measured 2026-10-18 over HTTP). The monitor and the mesh
fold each read the journal.

## Not yet

These are planned in milestone `peer-blackboard` and are not built:

- pushing the blackboard over a WebSocket;
- the `/ui` and `/cockpit` screens that render it;
- its public-safe published snapshot.

Until they exist, nothing on this page claims them.

## Providers

A provider appears on the blackboard only through an interface that proves it is there: an MCP
initialize on a checkout's board, or a session it opened on the mesh. The client name is the
provider's own and no surface keeps a list of providers. A provider that offers no supported,
authenticated event source is not shown as present. This includes OpenAI Dots, which today offers
no such source to a local runtime. A GitHub event is a GitHub signal, never a provider's
presence.
{% endraw %}
