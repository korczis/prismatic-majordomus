+++
title = "A client that starts the MCP server from a checkout with no executable to run, or none it can build, is served by the installed release and told so, instead of being turned away"
description = "A client that spawns the MCP server is waiting on a handshake, and one that does not get it in time reports the server as failed and carries on without it, so the session is silently the only worker of a repository that has many. When this checkout has no executable to run, or cannot make one, mcp is served by the installed release instead, and stderr names the release and the reason. Nothing else changes: every other command still exits 12, and a stale executable that can be rebuilt is rebuilt."
weight = 136
[extra]
claim_id = "mcp-client-always-served"
status = "guaranteed"
source = "docs/claims/mcp-client-always-served.md"
+++
{% raw %}

## What it means

A client that spawns the MCP server is waiting on a handshake, and one that does not get it in time reports the server as failed and carries on without it, so the session is silently the only worker of a repository that has many. When this checkout has no executable to run, or cannot make one, `mcp` is served by the installed release instead, and stderr names the release and the reason. Nothing else changes: every other command still exits 12, and a stale executable that can be rebuilt is rebuilt.

## How it works

`bin/majordomus-cli mcp` decides before it builds. If the crate's executable is missing it runs the installed release (`MAJORDOMUS_INSTALLED_BIN`, by default `majordomus` in `~/.local/bin`, where `install.sh` puts it). If the executable is stale it tries the build as before, and falls back when there is no cargo, less free space than `MAJORDOMUS_MIN_FREE_MB` (5120 by default) or the build fails. A file inside the repository is never taken for the installed release, and neither is one that does not answer `--version`. `MAJORDOMUS_MCP_BUILD=1` makes `mcp` build like every other command, for a contributor working on the server itself.

## How to see it

```bash
MAJORDOMUS_NO_BUILD=1 bin/majordomus-mcp --inspect      # with no built executable: serves from the installed release, stderr says why
bash test/run.sh 3177_a_client_always_gets_a_server      # the cases, over stubs of cargo, the free-space floor and the installed release
```

## What it does not cover

It does not install a release: with none installed the launcher fails as it always did, exit 12 and the command to run. It does not make the installed release the checkout's build, so a worker may be served by an older release than the sources; the line on stderr names which. It does not start a turn: a session that is merely opened still waits for a first message (`session-start-prompt`).

## Why it exists

On 2026-10-16 a session opened in a checkout whose executable had never been built, on a volume with 4.5GB free against a floor of 5120MB. The launcher refused the build, exited 12, and the client reported `CONNECTION_CLOSED` while a release that speaks the same protocol served the same checkout on its port. The session worked without `majordomus_peers` and `majordomus_announce`, which is the failure ADR 0156 describes. ADR 0172 decided the fallback; `project.shared-server-resilience` carries it.
{% endraw %}
