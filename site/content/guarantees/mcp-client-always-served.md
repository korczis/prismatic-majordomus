+++
title = "A client that starts the MCP server from a checkout whose executable is missing or stale is served by the installed release when it is of this tree's generation and told so, and is otherwise refused with exit 12 naming the explicit build; nothing is built"
description = "A client that spawns the MCP server is waiting on a handshake, and one that does not get it in time reports the server as failed and carries on without it. When this checkout's executable is missing or older than its sources, mcp is served by the installed release only when that release is of this tree's generation, and stderr names the release and the commit it was built from. Otherwise the launcher refuses with exit 12, naming why the release was not used and the explicit build, just build. Nothing is ever built on the way, for mcp or for any other command (ADR 0224)."
weight = 136
[extra]
claim_id = "mcp-client-always-served"
status = "guaranteed"
source = "docs/claims/mcp-client-always-served.md"
+++
{% raw %}

## What it means

A client that spawns the MCP server is waiting on a handshake, and one that does not get it in time reports the server as failed and carries on without it. When this checkout's executable is missing or older than its sources, `mcp` is served by the installed release only when that release is of this tree's generation, and stderr names the release and the commit it was built from. Otherwise the launcher refuses with exit 12, naming why the release was not used and the explicit build, `just build`. Nothing is ever built on the way, for `mcp` or for any other command (ADR 0224).

## How it works

`bin/majordomus-cli` asks `mj_rust_resolve` in `lib/rust_bin.sh`: `MAJORDOMUS_BIN` when it is set, the tree's own executable when it is current, then the installed release (`MAJORDOMUS_INSTALLED_BIN`, by default `majordomus` in `~/.local/bin`, where `install.sh` puts it) when `mj_rust_same_generation` says the commit its `RELEASE.json` names is in this repository and carries every crate input of the working tree unchanged, with nothing untracked — the same rule the pre-push gates borrow an executable on. A release whose stamp names another commit, names none, or predates a crate edit is refused, whatever version it calls itself. A file inside the repository is never taken for the installed release, and neither is one that does not answer `--version`.

## How to see it

```bash
CARGO_TARGET_DIR=$(mktemp -d) bin/majordomus-mcp --inspect   # no executable here: a release of this generation, or exit 12 naming just build
bash test/run.sh 3177_a_client_always_gets_a_server          # the cases, over a cargo that must not be called and a stamped release
bash test/run.sh 3958_no_entry_path_or_gate_builds           # every entry path and gate, missing and stale: zero cargo calls
```

## What it does not cover

It does not install a release, and it does not build one: with no release of this generation the client is told there is no server, and `just build` is the remedy. It does not judge an executable named by `MAJORDOMUS_BIN`, which the launcher trusts as it always did; the gates ask `mj_rust_same_generation` before they let one judge the tree. It does not start a turn: a session that is merely opened still waits for a first message (`session-start-prompt`).

## Why it exists

On 2026-10-16 a session opened in a checkout whose executable had never been built, on a volume with 4.5GB free against a floor of 5120MB; the launcher exited 12 and the client reported `CONNECTION_CLOSED` while a release that speaks the same protocol was installed beside it (ADR 0156). ADR 0172 answered with a fallback for `mcp` on a version string and kept the build for a stale executable. On 2026-10-22 a pre-push timed out at ninety minutes behind the shared build lock, and ADR 0224 replaced it: no ordinary command builds, and a fallback is a release of the same generation.
{% endraw %}
