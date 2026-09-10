---
schema: adr/v1
id: adr-0035
kind: adr
title: A checkout's server is one of the repository's, and its state is served like every other fact
status: proposed
date: 2026-09-10
tags:
  - mcp
  - architecture
  - coordination
provenance:
  origin: extracted
  derived_from:
    - decision:adr-0003
    - decision:adr-0021
    - file:docs/ENTRY_AUDIT.md
---

# 35. A checkout's server is one of the repository's, and its state is served like every other fact

## Context

ADR 0003 decided one shared server per repository: the first `majordomus mcp` binds
loopback HTTP beside its stdio session, every later one attaches, and the rendezvous is one
lease file under the checkout's local half. The election it describes has held up — a
lease of every shape is taken over, a client is never turned away, a replaced executable
loses its lease — and it was written before ADR 0021 gave this repository linked worktrees
as the ordinary way to work on a branch.

The audit of repository entry (`docs/ENTRY_AUDIT.md`, measured 2026-09-10) found what the
two decisions had never been made to agree on. The lease lives at the checkout root, the
root is the nearest `.ai/manifest.yaml`, every worktree has one, and the identity a probe
compares is a digest of that root. So "one server per repository" was one server per
*checkout*: a linked worktree elected its own server on its own port with its own peer
board, and no reading of any lease said that two such servers belonged together. A session
in one worktree could not see the peers of another, `context`'s peers section read the
primary's board from a worktree by guessing at the primary's path, and the mandate's
worktree scenario — same repository, distinct worktrees, peers that can tell the scopes
apart — was not failing; it was unreachable.

The same audit found the lease read by hand in four places: the election, the read-only
`serving`, the environment snapshot, and two shell readers (`jq` in `lib/context.sh`, `sed`
in `.just/serve.just`), with no type between them. What the lease records — pid, address,
start time, the executable — was on disk and on no surface; the health route counted peers
and said nothing about the process; readiness had three unrelated definitions (a
directory's output exists; the registry is non-empty; a TCP connect succeeded); and none of
them asked the one question a stale server class had already cost a day: is the process
behind this address the code on disk?

## Decision

**A server serves a checkout, and every checkout of a git repository is one repository's.**
The election is unchanged: one lease per checkout, at the checkout's local half, one
atomic create, every stale shape taken over. What changes is that the servers of one
repository can now be told from the servers of two, and listed from any of them:

- `repository::git_identity` asks git which repository a checkout belongs to — the git
  directory every work tree shares — and digests it. The digest is one value for every
  checkout of the repository and never a path. The index route (`/`) answers it beside the
  checkout's own `repository_id`, with `linked_worktree`, so that a reader of any server
  knows which repository it is one of and whether it is the primary. Where git cannot be
  asked the field is absent; a repository of the layer does not have to be version
  controlled, and this decision changes nothing for one that is not.
- The checkouts of a repository are read from git's own registry of them
  (`git worktree list`, the reader ADR 0021 already keeps), never from a file of this
  tool's. There is no registry of servers: a server is found where its checkout is, through
  the lease that checkout already has.

**The lease is one type, read once.** `lease::LeaseDocument` is what a server writes and
what every reader parses; `lease::LeaseFile::read` is the one reading, and it never fails:
absent, empty, corrupt and a document are its four answers, and what to do about each is
the reader's decision. The election, `serving`, the environment's service discovery and the
server's status all read through it. A key a reader does not know is ignored, so a lease
written by a newer executable still reads; a key an older lease lacks takes its default.
The document gains `version`, the executable's version, beside the executable identity it
already carried; the process that publishes a lease keeps the document it wrote
(`lease::held`), so that it can say what it wrote down without reading its own file back.

**The server's state is a capability.** `server.status` — `GET /api/v1/server`, the tool
`majordomus_server`, the resource `majordomus://server` — answers where this checkout's
server stands, measured against what this executable would serve: `absent` (no lease),
`starting` (a lease still binding), `ready` (answers for this checkout, from the file on
disk, at this version), `outdated` (answers, but from another version or from a file
replaced since it started) or `stale` (does not answer, or is not a lease), with the reason;
the lease this process holds when it is the server; and every checkout of the repository,
the primary first, each with its lease, its standing and how many peers its server holds.
The decision is a pure function of the lease file, its age, one probe and this executable's
version, tested branch by branch; the capability reads the files and the servers on every
call and caches nothing, because the leases are written by other processes.

## Alternatives rejected

- **Moving the lease to the primary checkout, one server per git repository.** It answers
  the mandate's words literally and serves the wrong tree: a server serves the layer of the
  checkout it was started in, and a session on a feature branch attaching to the primary's
  server would read the trunk's rules, decisions and plan. One process indexing every
  worktree would be the honest form of that decision; it is a larger change than this
  slice, and this decision does not foreclose it — a server that serves several checkouts
  would still be listed here, once per checkout.
- **A registry of servers under the primary's local half.** A file every server writes into
  and removes itself from is a second lease with the same failure modes and a new stale
  state of its own. Git already registers the checkouts; the lease each one has is enough.
- **Comparing versions in the election.** A lease naming a server of another version is
  taken over on `fix/stale-runtime-is-loud`, which is that decision's own branch; the status
  reports the mismatch as `outdated` and the election's behaviour is that branch's to
  change.
- **A command-line projection in this slice.** `serve status` belongs beside `serve
  ensure` and `serve stop`, which the next decision brings; a command is furnished once,
  with its documentation, its fixture and its cases, not twice.

## Consequences

`server.status`, the `git_repository_id` and `linked_worktree` fields of the index route,
`LeaseDocument` and its `version` field become compatibility surfaces. Every worktree
carries its own server as before; what is new is that each one can name the others.

`context`'s peers section and the `.just/serve.just` recipes still read the lease
themselves; they are the readers ADR 0003's amendment left in shell, and they move to the
executable's answer with the next slice (`docs/ENTRY_AUDIT.md`, stage 09).

The mandate's worktree scenario is now reachable: two worktrees of one repository report
the same `git.id`, distinct `checkout_id`s, and each other's servers and peer counts. A peer
still carries no worktree of its own on the board; that is the next decision's, on top of
the board journal ADR 0034 brings.

`tests/server_status.rs` holds the two-worktree case and the stale-lease case;
`tests/shared_units.rs` and `tests/mcp_shared.rs` hold the election as before, through the
same typed reading.
