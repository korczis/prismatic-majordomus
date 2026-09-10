---
id: project.shared-server-resilience
version: 1
kind: rule
title: Nothing a client leaves behind locks another client out
description: The shared MCP server is a client's child; whatever state a client or a dead server leaves behind is taken over or bypassed, and a client that cannot share is served alone, never turned away.
statement: A client of the shared server always gets a server built from the current code, or is served alone with the reason logged; no lease file in any state, no signal, no unwritable directory, no departed peer and no rebuild underneath a running server may leave a client without one, or with one that answers from code the tree no longer has.
status: active
class: blocking
depends_on: [project.rust-cli-evidence@1]
tags: [rust, mcp, resilience]
---

# Rationale

The shared server is the one process several clients depend on, and its only shared state
is one lease file under the checkout-local half of the layer. Anything that file can
contain after a crash, a kill or a copy of the checkout is a way for one client to lock
the others out, and a repository whose local half refuses writes would otherwise turn
every client away. The design refuses a daemon; what it owes in return is that the process
a client started never fails to serve that client because of what another process left
behind.

# Required behaviour

Under `apps/majordomus-cli/`:

- The election reads the lease on every attempt and classifies it. A server that answers
  for this root *from this version of the code* is attached to. A lease whose server does
  not answer, a file that is not a lease document, an empty file, and a lease whose owner
  published no URL within the bind grace are each taken over, and the log names which it
  was. A lease without a URL that is younger than the grace is waited for. The attempt is
  bounded by time, never by a count of rounds.
- Answering is not the same as being current. The lease records the executable the server
  was started from (path, mtime, size), and the probe reads the server's version off its
  index. A server of another version (`outdated lease`, both versions named) and a server
  whose own file has been replaced on disk (`superseded lease`) are taken over rather than
  joined, and this is decided before the server is asked whether it answers. A lease
  naming a different executable path makes no claim either way, so a release install and a
  debug build never fight over the lease.
- A server checks its own executable every reap interval and, when the file has changed,
  stops and releases the lease — logging `has been replaced on disk` — so that the next
  client elects a server built from the current code. Nothing outside the process may do
  this for it: a client that killed servers would kill every session attached to them.
- When the lease cannot be created, joined or replaced, or the shared server cannot start,
  `majordomus mcp` serves its client alone, as `--standalone` does, and logs `cannot use
  the shared server` with the path and the reason. The layer's own errors (no manifest,
  `--strict` refusing a degraded index) still exit with their codes: degrading applies to
  sharing, never to the layer.
- A bridge whose server is gone elects again on its next message; when it can neither
  serve nor attach, the same degrade applies before any error reaches the client.
- `SIGTERM`, `SIGINT` and `SIGHUP` remove the lease before the process dies of the signal.
  `kill -9` cannot be caught; the stale lease it leaves falls under the first point.
- A server whose owner has left ends with its last attached session; an HTTP session that
  never says goodbye expires at the idle timeout, so the server never outlives its clients
  by more than that timeout.
- A server whose lease has been taken over stops acting as the checkout's server, and says
  so before anybody has to ask twice. Its index answers `leaseholder: false`; the probe
  every reader of a remembered address goes through refuses it; a new `initialize` is
  refused with the reason and where the current server is; and its own health report says
  that what it describes is another process. The sessions it already had continue, and the
  process ends with them — a client mid-answer does not lose its server because somebody
  rebuilt the executable. What it must never do is take on a client that could have had
  the real one: that client gets a peer board, an execution history and a generation of the
  layer no other client of the checkout can see, with nothing in any answer saying so.
- A bind to an address that is not loopback is served with a warning naming what it
  exposes.
- A bridged session and a restarted server answer byte for byte what a local session
  answers.

Every point above is proved by a named case in `tests/mcp_shared.rs`, in
`tests/lease_lost.rs` or in `test/cases/90_mcp_shared_server.sh`, and a change to any of
them lands with its case.

# Failure behaviour

CI fails: `cargo test` runs `tests/mcp_shared.rs` in the `rust` job, and `test/run.sh`
runs case 90 in the `test` and `pages` jobs. A reviewer refuses a change to the lease, the
election, the bridge's failover or the signal handling that does not come with its case,
and a claim page for `mcp-lease-resilience` whose test no longer names the behaviour.

# Verification

`RUSTFLAGS='' cargo test --manifest-path apps/majordomus-cli/Cargo.toml --test mcp_shared`,
`RUSTFLAGS='' cargo test --manifest-path apps/majordomus-cli/Cargo.toml --test lease_lost`,
`bash test/run.sh 90_mcp_shared_server`, `scripts/rust-check`, and the claim
`mcp-lease-resilience` in `docs/CLAIMS.yaml`.
