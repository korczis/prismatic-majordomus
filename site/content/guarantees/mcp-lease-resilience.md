+++
title = "Nothing a client leaves behind locks another client out of the shared server; a corrupt, empty, stale or abandoned lease is taken over and named, a client that cannot share is served alone with the reason logged, and a signal removes the lease before the server dies"
description = "The lease under .ai/local/state/mcp/ is the only thing the shared server writes, and it is the only thing that can go wrong between clients. Whatever it contains when the next majordomus mcp starts, that process gets a server: a live one is attached to, a stale one (the server was killed), a corrupt one (not a lease document), an empty one (the owner died between creating and writing it) or an abandoned one (no URL after the bind grace) is taken over, and the log says which it was. When the file cannot be written or replaced at all, or the shared server cannot start, the client is served alone exactly as --standalone would serve it, with cannot use the shared server and the reason in the log; only the layer's own errors turn a client away. Ctrl-C, a client killing its server, or a closing terminal remove the lease before the process dies."
weight = 104
[extra]
claim_id = "mcp-lease-resilience"
status = "guaranteed"
source = "docs/claims/mcp-lease-resilience.md"
+++
{% raw %}

## What it means

The lease under `.ai/local/state/mcp/` is the only thing the shared server writes, and it is the only thing that can go wrong between clients. Whatever it contains when the next `majordomus mcp` starts, that process gets a server: a live one is attached to, a stale one (the server was killed), a corrupt one (not a lease document), an empty one (the owner died between creating and writing it) or an abandoned one (no URL after the bind grace) is taken over, and the log says which it was. When the file cannot be written or replaced at all, or the shared server cannot start, the client is served alone exactly as `--standalone` would serve it, with `cannot use the shared server` and the reason in the log; only the layer's own errors turn a client away. Ctrl-C, a client killing its server, or a closing terminal remove the lease before the process dies.

## How it works

`src/lease.rs` classifies an existing lease on every attempt (live, stale, corrupt, empty, abandoned, still binding), takes over every unusable one, and bounds the attempt by time rather than by a count of rounds; a `SIGTERM`, `SIGINT` or `SIGHUP` handler unlinks the recorded lease path and re-raises the signal. `src/commands/mcp.rs` turns an election or start failure into a standalone session (`Backend::Alone`), for a fresh client and for a bridge that can neither take over nor re-attach. `src/http/server.rs` warns when the bound address is not loopback.

## How to see it

```bash
just build
# in a checkout where no shared server is running (cat .ai/local/state/mcp/server.json says so)
mkdir -p .ai/local/state/mcp && printf '{not a lease' > .ai/local/state/mcp/server.json
apps/majordomus-cli/target/debug/majordomus mcp < /dev/null
# stderr: corrupt lease: not JSON (...); taking it over ... then: shared server listening on http://127.0.0.1:8741 ...
chmod 555 .ai/local/state/mcp
apps/majordomus-cli/target/debug/majordomus mcp < /dev/null
# stderr: cannot use the shared server: .../server.json: Permission denied ...; serving this client alone ...
chmod 755 .ai/local/state/mcp
```

## What it does not cover

`kill -9` cannot be caught, so it leaves the lease for the next process to take over, which is the stale case above. A bridge that takes over after its server died starts with an empty peer board: announcements are one process's memory. An HTTP client that leaves without `DELETE /mcp` keeps a server whose owner has already left alive until the idle timeout. A client served alone has no peers, no Swagger UI and no URL in its `initialize`, because there is nothing shared to point it at.

## Why it exists

Adversarial probes of the shared server on 2026-09-05 found that a corrupt lease locked every client out with exit 13, and that an unwritable lease directory did the same, both silent from the client's side. The rule `project.shared-server-resilience` and the amendment to `.ai/repo/adrs/0003-shared-mcp-server-peers-and-client-autostart.md` make the guarantee doctrine; `apps/majordomus-cli/tests/mcp_shared.rs` proves each path over real processes, pipes and sockets, and `test/cases/90_mcp_shared_server.sh` plants a corrupt lease behind `bin/majordomus-mcp` and watches the session succeed and the lease disappear afterwards.
{% endraw %}
