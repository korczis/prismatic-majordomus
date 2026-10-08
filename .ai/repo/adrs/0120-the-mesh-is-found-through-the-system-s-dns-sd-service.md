---
schema: adr/v1
id: adr-0120
kind: adr
title: The mesh is found through the system's DNS-SD service
status: proposed
date: 2026-10-08
tags:
  - mesh
  - discovery
  - security
related:
  - file:.ai/repo/adrs/0050-mesh-peer-discovery-is-provider-based-observation-with-authe.md
  - file:.ai/repo/adrs/0059-the-mesh-is-on-for-this-repository-and-every-session-start-h.md
  - rule:project.mesh-is-observation-not-authority
  - file:.ai/repo/mesh/majordomus.yaml
  - file:apps/majordomus-cli/src/mesh/bonjour.rs
  - file:apps/majordomus-cli/src/mesh/bonjour_dnssd.rs
  - file:apps/majordomus-cli/src/mesh/bonjour_avahi.rs
  - test:test/cases/1002_the_mesh_is_found_over_bonjour.sh
  - file:docs/MESH.md
provenance:
  origin: authored
---

# 120. The mesh is found through the system's DNS-SD service

## Context

ADR 0050 made discovery a contract — a provider observes and hands raw bytes up, the
manager verifies and decides — and shipped three implementations: UDP multicast, UDP
broadcast and rendezvous. It named mDNS as "one more implementation of the same contract",
and rejected building it then: "It would drag a dependency or a second protocol
implementation for a benefit the multicast provider already gives on the same segments."

The second half of that sentence stopped being true on the machines this repository is
developed on. Its own mesh declaration records the measurement: "Multicast cannot cross the
tailnet, and the macOS firewall drops it inbound, which is why the hubs exist"
(`.ai/repo/mesh/majordomus.yaml`). The macOS application firewall drops inbound multicast
addressed to an ordinary process, so on a segment of Macs the multicast provider transmits
and hears nothing, and two Macs side by side find each other only through a rendezvous hub
on a third machine that must be running. The benefit multicast was to give on the same
segment is the one it does not give there.

The same firewall lets one process through: the system's own DNS-SD daemon. Every Mac runs
mDNSResponder, most Linux machines run avahi, and both will register a service and browse
for others on behalf of any local process. That removes the first half of the sentence as
well: nothing has to be implemented and nothing depended on, because the protocol is
already running on the machine.

## Decision

A fourth provider, `bonjour`, under the contract of ADR 0050 and changing none of it.

1. **The system's service, never a socket of ours.** The provider opens no socket and
   speaks no mDNS. It asks the platform: on macOS through the DNS-SD C API of `<dns_sd.h>`
   (in libSystem; an `extern "C"` block, no crate and no linking), on Linux through
   `avahi-publish` and `avahi-browse` as child processes. The mechanism sits behind one
   small trait (`mesh::bonjour::DnsSd`), so that everything above it is tested with a
   fake. A machine with neither is `unavailable`, with the reason: a status, not a failure.
2. **One instance, named by the key.** A runtime registers one instance of
   `_majordomus._tcp` in `local.`, named `majordomus-<node>-<runtime>` — eight hex
   characters of the node id and of the runtime slot, both of which the envelope carries in
   full — on the port of its HTTP endpoint. No host name, user name or path is put into
   the name.
3. **The same envelope, in the TXT record.** The signed envelope every provider sends is
   split, unencoded, across ordered TXT strings: `txtvers=1`, `n=<count>`, `e0=…` …
   `e<n-1>=…`, at most 252 payload bytes each. The largest envelope the protocol allows
   (1200 bytes) makes a record of 1234 bytes, inside the 1300 that RFC 6763 recommends as a
   ceiling; base64 of it would not be. An envelope that cannot be carried is never
   truncated: the instance is registered with `n=0` and the status says so.
4. **Raw bytes up.** A browsed instance's TXT strings are joined back into bytes and sent
   to the manager with the source `bonjour`. The provider does not read them. An instance
   whose parts are missing, repeated, out of order or larger than a datagram is counted in
   the provider's status and dropped. The provider adds no filter of its own for its own
   registration: the manager's own-runtime rule already skips it, and counts it.
5. **A fresh envelope every interval.** A TXT record is a standing answer, not a datagram,
   and the manager refuses an envelope whose timestamp is more than 300 seconds from its
   clock, drops a repeated sequence without renewing presence, and holds a node `present`
   for 60 seconds after one accepted advertisement. So the record is replaced with a newly
   signed envelope every declared interval — 15 seconds by default, 60 at most, which the
   declaration's parser enforces. On macOS that is `DNSServiceUpdateRecord`: one
   announcement, the registration stands. avahi has no tool that replaces a record, so on
   Linux it is a new `avahi-publish`: a goodbye, a probe and an announcement per interval,
   and about a second in which the instance is not registered.
6. **Off unless declared.** A `bonjour:` block in the mesh declaration (`enabled`,
   `interval_seconds`, and a `service` type that only a test has reason to change), off
   when absent — unlike `multicast:`, which a declaration gets by saying `enabled: true`
   and nothing else. This repository's declaration enables it; the skeleton still ships no
   declaration at all.
7. **`mesh doctor` says which.** One `bonjour` line: declared and started, declared and
   failed (a failure), or declared and unavailable on this platform — which holds and is
   marked `WARN`, a third word the report did not have, because a declaration shared by
   Macs and a Linux hub without avahi is not a broken mesh.

A provider state `unavailable` joins `running`, `failed` and `stopped`, and a source
`bonjour` joins the sighting sources. No capability, route, tool or page is added: the
provider appears in `mesh status`, in the node listing's provenance and in the Cockpit
because those already render whatever providers and sources exist.

## Consequences

- **What becomes visible on the segment.** The instance's name and port are answered by
  the system daemon to any device that browses mDNS — phones, printers, televisions and
  anyone's `dns-sd -B _majordomus._tcp` — where a multicast advertisement reached only a
  listener that had joined the mesh's group. The name says that a Majordomus server runs
  here and states sixteen hex characters of two identifiers; the TXT record is the
  envelope, which carries what a multicast advertisement already carries (public key,
  instance, endpoints, transports, repository digests, version) and no more. A fleet for
  which the existence of the server is itself sensitive leaves the block out.
- **Discovery still creates awareness and no authority.** A runtime found through Bonjour
  is verified by its signature on the path every provider shares and judged by the same
  trust policy; `deny_unknown` observes it and trusts it for nothing. Nothing in mDNS is
  authenticated and nothing here relies on it: a forged TXT record is a refused signature
  and a counted refusal. The trust policy, the allowlist and the link handshake are
  unchanged.
- **Reach is the link.** mDNS is link-local. It crosses no router and no tailnet, so the
  rendezvous hubs stay for what they were always for; what Bonjour removes is the need for
  a hub between two machines of one segment.
- **The crate gains `unsafe` FFI of a second kind.** Until now the crate's `unsafe` was
  socket options and signals through `libc`. `mesh::bonjour_dnssd` declares eight
  functions of `<dns_sd.h>` and two callbacks; every block states its invariant, the
  callbacks only copy into an inbox, and every reference is deallocated on drop. It is
  compiled on macOS alone, so the Linux coverage run does not see it: the ignored test
  `the_real_service_registers_this_runtime_and_browses_its_envelope_back` is what
  exercises it, by hand, on a Mac.
- **The Linux path is not proved on Linux yet.** The avahi backend's process handling is
  tested with stand-in tools, and its parser against lines written from avahi's documented
  parseable format, not captured from a daemon. Two properties of real avahi are assumed:
  that `avahi-browse -rp` prints TXT strings as they are between double quotes, and that a
  registration ends with its publisher. An envelope read wrongly is refused by its
  signature, so the failure mode is a provider that finds nothing, not one that admits
  something.
- **Children.** On Linux a server holds two child processes while Bonjour runs, and ends
  and reaps them when the provider stops, fails or is dropped.

## Alternatives rejected

- **A private mDNS responder on 224.0.0.251:5353.** It is the second protocol
  implementation ADR 0050 refused, and it would not work where this is needed: the
  firewall that drops the multicast provider's datagrams would drop its datagrams too. It
  would also compete with the system daemon for the port.
- **A crate (`mdns-sd`, `zeroconf`, `astro-dnssd`).** The pure-Rust ones are the private
  responder above; the binding ones add a dependency and a build-time link for eight
  functions the platform already exports. `Cargo.lock` is unchanged by this decision.
- **`/usr/bin/dns-sd` as child processes on macOS.** It needs no `unsafe`, and it cannot
  replace a TXT record: every envelope would be a deregistration and a registration, and
  its line output would have to be parsed where the C API hands over the record's bytes.
  The crate already accepts reviewed `unsafe` over `libc`; the same standard is applied.
- **Replacing multicast.** Multicast needs no daemon, works on Linux segments and in the
  mesh lab's containers, and discloses the server only to a listener of its group. The
  providers are additive by contract: a node heard on both is one record with two
  sightings.
- **Encoding the envelope as text (base64url) in the TXT record.** It would survive any
  tool that mishandles quotes, and it grows a 1200-byte envelope past the 1300-byte
  ceiling. The record's wire format is binary-safe; the bytes are carried as they are.
- **An environment variable to force the provider unavailable in tests.** The mesh has no
  environment variables (`docs/MESH.md`), and one read only by a test seam is still one
  somebody will set. The seam is the trait.
