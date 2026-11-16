---
schema: adr/v1
id: adr-0202
kind: adr
title: "The mesh routes work to an identity, never an address, and selects among proven transports instead of implementing NAT traversal"
status: proposed
date: 2026-10-18
tags:
  - mesh-runtime
  - chatgpt-project
related:
  - file:.ai/repo/adrs/0050-mesh-peer-discovery-is-provider-based-observation-with-authe.md
  - file:.ai/repo/adrs/0067-mesh-cooperation-is-authenticated-links-and-one-replicated-journal.md
  - rule:project.mesh-is-observation-not-authority
  - file:docs/MESH.md
  - file:.ai/repo/project/issues/I3187.yaml
  - file:.ai/repo/project/issues/I3189.yaml
  - file:.ai/repo/project/issues/I3190.yaml
  - file:.ai/repo/project/issues/I3176.yaml
  - file:.ai/repo/project/milestones/mesh-connectivity.yaml
  - file:.ai/repo/project/milestones/an-authorised-node-connects-from-anywhere.yaml
  - file:.ai/repo/project/issues/I3198.yaml
  - file:.ai/repo/project/intents/an-authorised-node-connects-from-anywhere.yaml
provenance:
  origin: extracted
  derived_from:
    - issue:I3100
    - file:.ai/repo/adrs/0050-mesh-peer-discovery-is-provider-based-observation-with-authe.md
    - file:.ai/repo/adrs/0067-mesh-cooperation-is-authenticated-links-and-one-replicated-journal.md
    - file:docs/MESH.md
    - file:.ai/repo/project/issues/I3187.yaml
    - file:.ai/repo/project/issues/I3189.yaml
    - file:.ai/repo/project/issues/I3190.yaml
    - file:.ai/repo/project/issues/I3176.yaml
    - file:.ai/repo/project/milestones/mesh-connectivity.yaml
    - file:.ai/repo/project/milestones/an-authorised-node-connects-from-anywhere.yaml
    - file:.ai/repo/project/issues/I3198.yaml
    - file:.ai/repo/project/intents/an-authorised-node-connects-from-anywhere.yaml
---

# 202. The mesh routes work to an identity, never an address, and selects among proven transports instead of implementing NAT traversal

## Context

Between 10-12 and 10-18 the owner asked four times to make discovery work across NAT, with 'fast-flux', mothership, hole punching, STUN/TURN and WebRTC ideas. Master has multicast and directly-reachable rendezvous hubs and states 'no NAT traversal'.

## Decision

A peer is a stable cryptographic identity; endpoint candidates are expiring observations with source and confidence; DNS is a discovery projection, never identity. The logical network (identity, capability, intent) is separate from the physical (address, NAT, relay). Majordomus owns the contracts and the choice: it integrates proven transports and relays, chooses by measured success among authenticated candidates with hysteresis, starts relay-first and upgrades to direct, and never re-implements ICE. Rendezvous, relay and gateway are roles a trusted peer holds, not fixed servers. The phrase 'fast flux' names nothing in code or docs. DHT, coordinator election and custom consensus wait for a measured need.

## Alternatives rejected

- Literal DNS fast-flux or double-flux
- A custom NAT traversal stack or own ICE
- Address-based task dispatch
- A fixed mothership server
- DHT first

## Consequences

mesh-connectivity (I3187..I3190); deferred past v1.0.0-alpha.1 by Operation Convergence.

## Sources

Promoted on 2026-10-18 from the owner's ChatGPT project (workspace `chatgpt-majordomus`, issue I3100). The owner has not accepted it; it is proposed for the owner's decision.

- `chatgpt-majordomus:c237#4`
- `chatgpt-majordomus:c279#2`
- `chatgpt-majordomus:c279#4`
- `chatgpt-majordomus:c279#7`
- `chatgpt-majordomus:c279#10`
- `chatgpt-majordomus:c282#3`
- `chatgpt-majordomus:c282#5`
