---
schema: knowledge/v1
id: lundra-is-the-always-on-hub
kind: knowledge
class: fact
title: "lundra is the owner's always-on machine behind NAT, already a trusted mesh node and hub"
description: "When placing a persistent service, an offload or a hub."
status: verified
epistemics: observed
date: 2026-10-18
tags:
  - mesh-runtime
  - chatgpt-project
provenance:
  origin: extracted
  derived_from:
    - issue:I3100
    - file:docs/MESH.md
    - file:docs/OFFLOAD.md
    - file:.ai/repo/mesh/majordomus.yaml
relations:
  - type: relates_to
    target: file:.ai/repo/adrs/0204-the-hub-is-reached-privately-over-the-tailnet-first-a-public-ing.md
  - type: documents
    target: file:.ai/repo/project/issues/I3195.yaml
  - type: documents
    target: file:.ai/repo/project/milestones/the-hub-is-reached-from-any-device.yaml
  - type: relates_to
    target: file:.ai/repo/adrs/0199-a-hub-is-a-declared-host-service-a-working-checkout-s-server-sta.md
  - type: relates_to
    target: file:.ai/repo/project/issues/I3176.yaml
---

# lundra is the owner's always-on machine behind NAT, already a trusted mesh node and hub

The owner described lundra on 2026-09-11 as a home machine behind NAT with no public address, and majordomus.dev as a domain under the owner's Cloudflare account. lundra is listed in the mesh trust row and runs this repository's hub as a systemd user unit (docs/MESH.md). It is also the derive offload and headless worker host. Reach it by node identity; its addresses belong to discovery, not to tracked files.

## Sources

- `chatgpt-majordomus:c118#5`
- `chatgpt-majordomus:c118#4`
