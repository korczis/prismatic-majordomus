+++
title = "The inbound admissions a mesh declaration implies on a machine — the multicast group's port, the declared hub ports whose address is the machine's, the server's port beyond loopback — are derived from the declaration by `majordomus mesh firewall`, which names the host's firewall backend, the commands that admit them and the firewall's own verdict, exits 10 when a rule is observed missing or the kernel logged a drop toward those ports, and whose `apply` runs the commands only as root and never over HTTP or MCP; `mesh doctor` carries the same verdict as its `firewall` check"
description = "The declaration says who may link; the host firewall says what reaches the socket at all,"
weight = 224
[extra]
claim_id = "mesh-firewall-is-derived"
status = "guaranteed"
source = "docs/claims/mesh-firewall-is-derived.md"
+++
{% raw %}

## What it means

The declaration says who may link; the host firewall says what reaches the socket at all,
and nothing tied the two together. On 2026-10-08 lundra, a declared rendezvous hub, ran ufw
with default deny and no rule for port 8791 or the multicast group: the kernel dropped every
registration and every advertisement from the LAN — 187 `[UFW BLOCK]` lines in a week — while
`mesh doctor` reported every check holding, because every check it ran was a fact of the
process and none was a fact of the host. This claim is that the admission is derived from the
declaration, printed with the commands that make it so, observed against the firewall's own
word, and failed loudly when the host drops what the declaration promises.

## How it works

`apps/majordomus-cli/src/mesh/firewall.rs` is pure where it can be. `plan` is a function of
the declaration, this machine's interface addresses and the server's port: the multicast
group's port when multicast is on, the broadcast port when it is a permitted fallback, every
`rendezvous.endpoints` entry whose address is this machine's, and the server's port when one
is given; the source networks are the enclosing private ranges of every address the
declaration names (hubs, seeds), or this machine's own when it names none, and a public
address derives nothing. `detect` is a function of the platform and what is on the path (ufw
before nft on Linux; the application firewall on macOS; nothing elsewhere). `render` is a
function of the two: one `ufw allow in` or `nft add rule` per rule and source, each with the
comment `majordomus mesh: …`, or the server's executable admitted on macOS. `judge_ufw`,
`judge_nft` and `judge_application_firewall` are functions of the firewall's own text, and
`count_blocks` of the kernel log's. Only `observe`, `kernel_blocks` and `apply` touch the host:
the first two say when they could not (root is needed to read ufw and nft; the kernel log reads
without it for a user in `adm`), and `apply` refuses, running nothing, without root or without
a backend, records every command with its exit and output, and asks the firewall again, so the
verdict afterwards is the firewall's.

The capability module projects `mesh.firewall` (CLI, `GET /api/v1/mesh/firewall`,
`majordomus_mesh_firewall`) and `mesh.firewall.apply` (CLI only — it runs a privileged host
tool, and the module's own test holds that no other command is served nowhere over HTTP). The
doctor (`apps/majordomus-cli/src/mesh/doctor.rs`) runs the report as its `firewall` check,
between the process's checks and the server's `runtime` verdict, and fails it exactly when the
report fails: a rule observed missing, or a drop logged within the five-minute window.

## How to see it

```
majordomus mesh firewall                     # the plan, backend, commands, observation, drops, verdict
majordomus mesh firewall --format json       # the same, for scripts; exit 10 when the host does not admit the mesh
sudo majordomus mesh firewall apply          # admit it; refuses without root
majordomus mesh doctor                       # the `firewall` line
bash test/run.sh 495_the_firewall_admits_the_mesh
cargo test --manifest-path apps/majordomus-cli/Cargo.toml --lib -- mesh::firewall
```

On lundra, 2026-10-08, as the owner: `unobservable` with 81 and 28 drops in the window; as
root: `missing`, both rules; after `apply` (`Rule added`, `Rule added`, `Rule updated`):
`present`, no drop, and the hub's `mesh nodes`, which had listed nobody, listed both Macs of
the LAN as present and trusted. `test/cases/495_the_firewall_admits_the_mesh.sh` holds the
derivation over this repository's declaration and over fixtures (no declaration, this machine
as a hub, the server port once), the doctor's check, the refusal without root and the MCP
tool, and never applies anything; the crate's unit tests and doctests hold the plan, the
rendering and every judgement.

## What it does not cover

It is not a firewall manager: it admits the mesh's ports from the fleet's private networks,
touches no other rule, never widens a source to a public range and removes nothing. It does
not observe a firewall it cannot read — ufw and nftables need root, and without it the check
holds as an absence unless the kernel log shows drops. It does not run anywhere but the
command line for `apply`, so a firewall nobody runs the command on admits nothing. On macOS
the application firewall admits an executable, not a port, so a new executable is a new
admission. And it does not make a hub whose declared address is stale reachable; that is the
declaration's row to fix.

## Why it exists

A hub that every other machine registers with and that lists nobody is the mesh at its most
misleading: every self-check holds, the server is active, and the fleet is still alone. Making
the host's admission a derived requirement with a verdict turns "the hub heard nobody for a
week" into one failed line the first `mesh doctor` prints.
{% endraw %}
