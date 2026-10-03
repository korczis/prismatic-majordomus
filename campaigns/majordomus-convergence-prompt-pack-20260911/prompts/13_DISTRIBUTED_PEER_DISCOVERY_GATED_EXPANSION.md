# Stage 13 — Distributed Peer Discovery (Only After Foundation Green)

Run this stage only if stages 02–12 are green and the repository's current plan still calls for LAN/distributed Majordomus cooperation.

Do not resurrect distributed complexity on top of a split-brain local runtime.

## Mission

Make Majordomus instances discover each other and cooperate through the canonical peer/capability runtime, with explicit trust boundaries and observability.

## Design constraints

- local machine first, LAN optional/configurable,
- no secret broadcast,
- stable peer identity distinct from ephemeral process identity,
- TTL/expiry and restart semantics,
- protocol version negotiation,
- capability advertisement derived from canonical registry,
- provider/model/session state exposed only according to policy,
- authenticated/authorized mutation across peers where necessary,
- graceful partition/network failure,
- no assumption that UDP discovery equals trust.

## Possible transport

Inspect existing architecture before choosing. If UDP multicast/broadcast is already planned/implemented, finish it. Otherwise compare mDNS/UDP/static seed/HTTP/WebSocket mechanisms against project constraints. Discovery and control transport need not be identical.

## Required surfaces

- CLI peer discovery/status,
- API/OpenAPI,
- MCP peer tools/resources,
- Cockpit peer topology/control,
- diagnostics/doctor,
- docs/site,
- integration tests with multiple local processes/ports,
- capability compatibility tests.

## Cooperation semantics

Do not invent a second orchestration framework. Peers exchange identity, status, advertised capabilities, leases/session/work claims and messages through canonical objects. Distributed execution should use the existing execution/capability engine.

## Tests

Cover:

- discovery/no discovery,
- duplicate announcements,
- peer restart,
- expiry,
- incompatible protocol,
- unauthorized peer,
- malicious/malformed packet,
- multi-interface behavior,
- simultaneous discovery,
- network partition/recovery,
- capability changes,
- multiple providers/models visible/manageable without leaking secrets.

## Acceptance

Distributed peers are an extension of the canonical local runtime, not a parallel product. All semantics are tested, observable, documented and policy-controlled.
