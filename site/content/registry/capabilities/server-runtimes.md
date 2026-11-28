+++
title = "server.runtimes"
description = "Every Majordomus runtime of this user on this host, whatever repository it serves, with no checkout open: each checkout with its root and identity, the git repository it belongs to, where its server stands (the same five words as `server.status`, measured against the answering executable, and `live` only when a server answers — a lease whose process is gone is `stale`), the port its lease names, the version it runs (the lease's, else what its executable reports to `--version`), the `serve` and `mcp` processes working on it and whether each holds the lease, and this host's node id. The checkouts come from two places and neither is believed: the disposable index of checkout roots each server adds itself to beside `node.json` (`checkout-roots` in `$XDG_STATE_HOME/majordomus`, else `~/.local/state/majordomus`), and the process table, where every `majordomus serve` and `majordomus mcp` is found by executable path — which is how a runtime older than the index, or one with another state directory, is listed. Every row is re-read from its checkout's lease, so deleting the index loses nothing but speed. Read-only: no process of its own, no registry, nothing started or stopped; peers stay the mesh registry's. The command line answers it without a server (`majordomus runtimes --host`, from any directory), and any running server answers the same reading here."
weight = 210
slug = "server-runtimes"
[extra]
id = "server.runtimes"
source = "apps/majordomus-cli/src/capability/builtin/server.rs"
+++
