---
id: project.the-server-sees-the-current-tree
version: 1
kind: rule
title: A long-lived process serves the repository as it is, never as it was when it started
description: The shared server follows the repository it serves; a commit made while it runs is visible through every surface it offers, without a restart and without a poll.
statement: A process that outlives a request — the shared server, a stdio MCP session — checks before it answers whether the repository has moved, and rebuilds what it derived from it when it has. The check is a fingerprint over the files git writes when it moves a repository, taken on the request's own thread; it is never a background poll, never a subprocess and never a network call. Everything derived from the index is rebuilt with the index; what is this process's own life — the peer board, the running executions — survives the rebuild.
status: active
class: blocking
depends_on: [project.rust-hot-path@1]
tags: [rust, mcp, server, freshness]
---

# Rationale

On 2026-09-11, on `origin/master` at `faba8fdef`, a shared server was started, asked for
the repository's head, given a commit, and asked again:

    HEAD before   2e70f780ba5cddd9f42b4d139456645357d72fac
    the API said  2e70f780ba5cddd9f42b4d139456645357d72fac
    HEAD after    2c6f59557d650903ee582bdb80dd0b164ed4028c
    the API said  2e70f780ba5cddd9f42b4d139456645357d72fac

The same stale answer came from MCP and from the Cockpit, a rule committed while the server
ran was absent from `objects`, and a working tree reported `clean` while it was dirty.
Nothing anywhere said the picture was old.

That is the most expensive shape a wrong answer can have. A session attached to this server
for a day reads a repository that no longer exists, decides on it, and is never told —
the answers are well-formed, confident and current-looking, and the only remedy is a
restart nobody knows to perform. The server exists so that several workers share one
reading of the repository; a shared reading that is hours out of date is worse than none,
because each worker would otherwise have read the repository itself.

# Required behaviour

`crate::live::Live` holds the repository a long-lived process reads. Every path that is
about to *use* the context takes it from there, and taking it compares a
`crate::live::Stamp` — the size and modification time of the git control files, about eight
`stat` calls — with the one the current generation was built at. When they differ, the
request that noticed rebuilds the layer; a second request arriving mid-rebuild is answered
from the generation that exists rather than queued behind it.

Everything that is a projection of the index is memoised per *generation* and not once per
process: the OpenAPI document, the MCP tool and resource listings, the resolved web
surfaces. `Context::continuing` carries what is not a projection of anything across the
rebuild — the peer board, the running executions, the capability executor — because a
reload is not a restart and a peer must not lose its place because somebody committed.

This refines `project.rust-hot-path` and does not loosen it. That rule's counters are about
work a request repeats over a repository that has not changed, and they still do not move
across any number of such requests. What this rule adds is that the work is done once
when the repository *has* changed, by the request that noticed, rather than never.

A one-shot command, a benchmark and a test take `Live::pinned` and pay nothing: the picture
cannot go stale inside a process that lives for 40 ms, and making them prove it per call
would be a regression bought with nothing.

# Failure behaviour

A layer that will not load after a move is not an error to the client: the last generation
that did load goes on being served, the reason is logged, and the stamp is recorded so the
failure is not retried on every request. A repository that is not a git work tree has no
moves to follow and is pinned, which is said once in the log.

# Verification

`test/cases/150_server_sees_the_current_tree.sh`: a server is started, its head recorded,
a commit made, and the same process — not a restarted one — asked again through the HTTP
API and through an MCP session opened before the commit. The object committed in that
commit must be in the index, and the peer must still be on the board.

`crate::live`'s own tests hold the check to the cost that makes it possible: a stamp is
taken in well under a millisecond, because it runs before every request the server answers.
