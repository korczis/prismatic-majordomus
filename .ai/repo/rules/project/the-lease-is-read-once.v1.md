---
id: project.the-lease-is-read-once
version: 1
kind: rule
title: The shared server's lease has one reader
description: The file that says which process serves a checkout and at which address is parsed by one typed reader in the executable and served from there; no other file in this repository opens it, and a caller that needs the address asks `serve status`.
statement: The lease is read by `lease::LeaseFile::read` and by nothing else; a caller that needs to know where a server is asks the executable, and never the file.
status: active
class: blocking
depends_on: [project.interfaces-are-projections@1, project.derived-once@1, project.no-machine-paths@1]
tags: [mcp, coordination, derived, gates]
---

# Rationale

ADR 0003 gave this repository one shared server per checkout and one rendezvous: a lease
file naming the process that serves it and the address it bound. One file, written by one
writer — and, by 2026-09-10, read by five.

Three of them were in the executable (the election, the read-only `serving`, the
environment's service discovery), each with its own idea of what the document contains.
ADR 0035 made those one typed reading, `lease::LeaseFile::read`, whose four answers —
absent, empty, corrupt, a document — leave the decision to the reader instead of hiding it
in an error. Two survived in shell, and they were worse than duplicated: they disagreed.

`lib/context.sh` found the file itself, guessed at the primary checkout's path when the
current worktree had none, and pulled the address out with `jq`. `.just/serve.just` and
`scripts/cockpit-probe` both ran

    sed -n 's/.*"url":"\([^"]*\)".*/\1/p'

over JSON. That expression is correct for exactly one way of writing the document — the
compact one — and matches nothing the day any writer pretty-prints it, adds a space after
the colon, or puts another key ending in `url` before it. What such a reader then reports
is not "I cannot read this". It is "there is no server": a silent, confident, wrong answer
in the one place a worker looks to find out whether anybody else is here. This repository
has already paid for that class twice — a glob that never matched an ADR, a design gate
whose colour check cried wolf — and the cost each time was a feature that had stopped
working and said nothing.

The price of five readers is not five bugs. It is that no single change can fix them,
because nothing in the tree knows they are the same question.

# Required behaviour

`apps/majordomus-cli/src/lease.rs` declares where the lease is (`LEASE_PATH`), what it is
(`SCHEMA`) and how it is read (`LeaseFile::read`). Everything that needs to know where a
server is reads it through that type, or through what that type is served as:

- inside the executable, `LeaseFile::read` — never `serde_json` over the path, never a
  hand-rolled probe of the file;
- outside it, `majordomus serve status` (`GET /api/v1/server`, the tool `majordomus_server`,
  the resource `majordomus://server`), which reports every checkout of the repository, the
  standing of each server and the address its lease published. `--format json` is the shape
  a script reads; `bin/majordomus-cli` is the launcher a shell caller already has.

A shell caller may name the lease path in order to *watch* it — `.envrc` asks direnv to
re-evaluate when it changes — and may not open it. A test may write a lease and read it
back: there the file is the subject under test rather than a dependency of the code.

A caller on a hot path never builds the executable to ask the question. `lib/context.sh`
and `scripts/cockpit-probe` use an executable that is already there and skip the answer
when there is none, which is the same policy the session start event's `serve ensure`
keeps: an executable that is missing is named, not compiled.

# Failure behaviour

`scripts/ci/lease-reader-check` exits 10 when a file that is neither the reader, nor
documentation, nor a test names the lease path or the lease schema, and runs as the
`lease-reader` gate in every plan. It reads both names out of the reader's own declarations
rather than carrying a copy, so renaming either renames it here too. Enforcement is a
ratchet over `.ai/repo/lease-reader-baseline.txt`, which is empty as this rule lands:
nothing but the reader reads the lease, and a file that starts to fails the gate.

This is a CI gate and not a `lib/` validator: it is a fact about this repository's own
source tree, not about a layer the tool serves to somebody else
(`project.rule-is-a-doctrine`).

# Verification

`test/cases/110_lease_reader.sh` proves it by mutation on a fixture: a tree whose only
reader is `lease.rs` passes; a shell script that `jq`s the lease, and a Rust module that
parses it by schema string, are each refused with the file named; a documentation page and
a test that name the lease are not; `.envrc` watching the file is not; a reader on the
baseline does not fail and a new one beside it still does; and a reader that declares
neither name makes the gate refuse to measure rather than pass.

`test/cases/106_context_peers.sh` holds the other half — that the peers section is never
load-bearing — across the move: no server, a server that does not answer, an unreadable
lease and an empty board all leave `context` exactly as it was, exit 0, and say nothing.
