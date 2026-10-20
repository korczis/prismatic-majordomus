---
schema: adr/v1
id: adr-0203
kind: adr
title: "GitHub Pages is a signed bootstrap anchor for the mesh, never its signalling channel"
status: proposed
date: 2026-10-18
tags:
  - mesh-runtime
  - chatgpt-project
related:
  - file:.ai/repo/adrs/0050-mesh-peer-discovery-is-provider-based-observation-with-authe.md
  - file:.ai/repo/adrs/0141-the-ui-is-published-as-a-snapshot-read-at-one-commit.md
  - file:.ai/repo/adrs/0157-the-mesh-offers-a-shared-build-cache.md
  - file:scripts/pages
  - file:.ai/repo/project/issues/I3188.yaml
  - file:.ai/repo/project/milestones/mesh-connectivity.yaml
  - file:.ai/repo/project/milestones/an-authorised-node-connects-from-anywhere.yaml
  - file:.ai/repo/project/intents/an-authorised-node-connects-from-anywhere.yaml
provenance:
  origin: extracted
  derived_from:
    - issue:I3100
    - file:.ai/repo/adrs/0050-mesh-peer-discovery-is-provider-based-observation-with-authe.md
    - file:.ai/repo/adrs/0141-the-ui-is-published-as-a-snapshot-read-at-one-commit.md
    - file:.ai/repo/adrs/0157-the-mesh-offers-a-shared-build-cache.md
    - file:scripts/pages
    - file:.ai/repo/project/issues/I3188.yaml
    - file:.ai/repo/project/milestones/mesh-connectivity.yaml
    - file:.ai/repo/project/milestones/an-authorised-node-connects-from-anywhere.yaml
    - file:.ai/repo/project/intents/an-authorised-node-connects-from-anywhere.yaml
---

# 203. GitHub Pages is a signed bootstrap anchor for the mesh, never its signalling channel

## Context

On 2026-10-16 the owner asked whether Pages can serve as serverless discovery. Pages is static and cannot accept writes; the GitHub API is rate limited and unsuited to heartbeats.

## Decision

Pages carries a signed manifest of hub identities and public reachability hints, with the trust root delivered independently, canonical serialisation, a monotonic sequence and an expiry; a node falls back to its last valid copy. Discovery is hierarchical (existing mesh, local cache and multicast, the manifest, rendezvous) tried in parallel with bounded attempts. Once linked a node needs Pages for nothing.

## Alternatives rejected

- Pages plus GitHub API polling as signalling
- A custom rendezvous server as the only path
- A fixed try-one-to-five order

## Consequences

I3188; private addresses never enter the manifest (I3176).

## Sources

Promoted on 2026-10-18 from the owner's ChatGPT project (workspace `chatgpt-majordomus`, issue I3100). The owner has not accepted it; it is proposed for the owner's decision.

- `chatgpt-majordomus:c282#2`
