+++
title = "Sessions on several machines work one repository without colliding"
description = "Each runtime is a key-authenticated participant; discovery (multicast, broadcast, rendezvous, seeds) only finds candidates, and a signed handshake admits a link only for the same repository, a compatible protocol and a trusted key. Linked runtimes replicate one signed event journal every heartbeat and fold it into the same state everywhere, so an exclusive claim on one machine refuses an overlapping claim on another, a handover published on one is consumed on another, and a runtime that crashes expires everywhere on its own. The CLI, HTTP, OpenAPI, MCP and the Cockpit render the same state, and every guarantee is held by tests over real processes and a container lab on a real network."
weight = 45
[extra]
id = "mesh"
status = "stable"
source = ".ai/repo/features/mesh.md"
+++
{% raw %}

## What it does

Several AI coding sessions — two terminals on a laptop, a desktop, a build machine — work
one repository. With an enabled `mesh` declaration each checkout's server is a runtime of
the mesh: it announces itself (signed, over multicast, a broadcast fallback, a rendezvous,
or declared seeds), and when it finds a runtime of the **same repository** whose key it
**trusts**, the two open a link through a signed handshake. From then on, every heartbeat,
they exchange what the other lacks of one journal of signed events:

- **sessions** — who is working, with which client, on which issue, branch and head;
  the peer board's announcements are projected in automatically;
- **claims** — an exclusive claim made on one machine refuses an overlapping claim on every
  linked machine, `claim_conflict` naming the claim it meets;
- **handovers** — `mesh handover publish` on one machine, `mesh handover consume` on
  another, and the record lands where `majordomus handover --resolve` finds it;
- **reviews** — ask a named runtime for a review and see its answer on your machine.

Every linked runtime folds the same events into the same state and prints the same digest.
A runtime that stops beating — killed, crashed, cut off — expires on its peers within the
declared expiry, its claims stop excluding, and when it comes back it reconnects as the same
runtime. `majordomus mesh peers`, `mesh state`, `mesh verify`, `/api/v1/mesh/*`, the MCP
tools and the Cockpit's Mesh page show machines, runtimes, sessions, claims, links and why
anything was refused.

## What it does not do

It does not trust the network: discovery finds candidates and grants nothing, a link needs
the same repository, a compatible protocol and a trusted key, and nothing a peer sends
executes anything. It is not a strongly consistent lock service — during a partition both
sides may claim the same scope, and the conflict is named with one winner after healing.
It does not encrypt: links belong on a private network or an overlay such as a tailnet. It
does not cross a WAN, traverse NAT or copy source files — git owns the code; the mesh
carries what people and agents say about their work. And it sends nothing until a person
commits `enabled: true` and lists the keys to trust.
{% endraw %}
