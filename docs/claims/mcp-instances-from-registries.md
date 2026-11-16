# Majordomus lists the live MCP instances that announce themselves in instance registries

## What it means

Software that runs its own MCP server on this machine can make itself discoverable without any
launcher knowing about discovery: when its server starts it writes one owner-only JSON record of
itself (schema `mcp-instance/v1`: name, kind, owner pid, endpoint, the path of its 0600 token
file, never the token) to a shared registry directory, and removes it when it stops.
`instances.list` reads every registry the configuration names and answers each instance with
whether its owner process is still alive. A malformed file, or a record that carries a token, is
refused with the reason and nothing of its content. A stale record stays where it is. With
`probe`, each live instance's loopback endpoint is sent an MCP `ping`.

## How it works

`apps/majordomus-cli/src/capability/builtin/instances.rs` declares the builtin module
`instances` and its one capability, `instances.list`, projected to the MCP tool
`majordomus_instances`, the resource `majordomus://instances` and `GET /api/v1/instances`. The
registries are the input's `registries`, else the colon-separated `MAJORDOMUS_INSTANCE_REGISTRIES`;
an absent directory is reported with `exists: false`, not as an error. Liveness is the same
owner-process test the lease uses. A probe reads the token from the record's token file in
memory, refuses an endpoint that is not loopback or that differs from the token file's own
endpoint, sends one JSON-RPC `ping` with the bearer token over HTTP/1.0 and keeps the token out
of every field of the answer.

## How to see it

```bash
MAJORDOMUS_INSTANCE_REGISTRIES=~/.local/state/prismatic-mcp/instances majordomus run instances.list --input '{}'
majordomus run instances.list --input '{"registries": ["/path/to/registry"], "probe": true}'
bash test/run.sh 4621_mcp_instances_are_discovered_from_registries
cargo test --manifest-path apps/majordomus-cli/Cargo.toml instances
```

## What it does not cover

It neither writes nor removes records: the instances own them. It does not admit an instance to
the mesh, sign anything or let it take part in claims; an instance is a pointer to an endpoint,
not a node. It never probes a non-loopback endpoint, and whether the instance's own tools work is
the instance's to prove.

## Why it exists

An adopter running a research cluster of MCP-driven instances found that every instance already
announced itself in such a registry and its own control panel listed them, while Majordomus,
running beside every repository, could not see one: the mesh admits signed Majordomus nodes and
web surfaces are mounts this executable serves, and neither describes a live third-party MCP
server. `test/cases/4621_mcp_instances_are_discovered_from_registries.sh` holds the behaviour
with a temporary registry: a live record, a stale one, a malformed file, one carrying a token,
and a probe that must not leave the machine.
