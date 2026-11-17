---
schema: adr/v1
id: adr-0209
kind: adr
title: "Prismatic Lumen names the owner's mesh-wide view, and it owns no scheduler, event bus, store or API"
status: proposed
date: 2026-10-18
tags:
  - product-adoption
  - chatgpt-project
related:
  - rule:project.no-new-nouns
  - rule:project.derived-once
  - file:.ai/repo/adrs/0012-the-cockpit-is-a-projection-not-an-application.md
  - file:.ai/repo/adrs/0089-an-interface-shows-only-what-is-derived-and-unknown-is-never-healthy.md
  - file:.ai/repo/adrs/0133-everything-is-followed-in-one-stream.md
  - file:.ai/repo/adrs/0135-majordomus-ui-is-the-primary-surface.md
  - file:.ai/repo/adrs/0161-a-mesh-node-follows-the-published-release.md
  - file:apps/majordomus-cli/src/ui/mesh.rs
  - file:.ai/repo/adrs/0207-a-new-mechanism-is-admitted-only-with-the-measured-problem-it-re.md
  - file:.ai/repo/project/milestones/prismatic-lumen.yaml
  - file:.ai/repo/project/issues/I3137.yaml
  - file:.ai/repo/project/issues/I3138.yaml
  - file:.ai/repo/project/issues/I3139.yaml
  - file:.ai/repo/project/issues/I3140.yaml
  - file:.ai/repo/project/intents/the-mesh-is-operated-from-any-device.yaml
  - file:.ai/repo/adrs/0201-the-persistent-runtime-is-the-authority-every-operator-surface-l.md
  - file:.ai/repo/project/milestones/the-mesh-is-operated-from-one-page.yaml
provenance:
  origin: extracted
  derived_from:
    - issue:I3100
    - file:.ai/repo/adrs/0012-the-cockpit-is-a-projection-not-an-application.md
    - file:.ai/repo/adrs/0089-an-interface-shows-only-what-is-derived-and-unknown-is-never-healthy.md
    - file:.ai/repo/adrs/0133-everything-is-followed-in-one-stream.md
    - file:.ai/repo/adrs/0135-majordomus-ui-is-the-primary-surface.md
    - file:.ai/repo/adrs/0161-a-mesh-node-follows-the-published-release.md
    - file:apps/majordomus-cli/src/ui/mesh.rs
    - file:.ai/repo/adrs/0207-a-new-mechanism-is-admitted-only-with-the-measured-problem-it-re.md
    - file:.ai/repo/project/milestones/prismatic-lumen.yaml
    - file:.ai/repo/project/issues/I3137.yaml
    - file:.ai/repo/project/issues/I3138.yaml
    - file:.ai/repo/project/issues/I3139.yaml
    - file:.ai/repo/project/issues/I3140.yaml
    - file:.ai/repo/project/intents/the-mesh-is-operated-from-any-device.yaml
    - file:.ai/repo/adrs/0201-the-persistent-runtime-is-the-authority-every-operator-surface-l.md
    - file:.ai/repo/project/milestones/the-mesh-is-operated-from-one-page.yaml
---

# 209. Prismatic Lumen names the owner's mesh-wide view, and it owns no scheduler, event bus, store or API

## Context

On 2026-10-18 the owner asked for one skill and one page that keep the tool updated, join the mesh, report regularly and act as the interface to the whole mesh, and chose the name prismatic-lumen over Oracle, Helm and Prism. The proposal that followed mapped it to /lumen, /ui/lumen, majordomus lumen and lumen.*; the critique the owner asked for warned that Lumen and Majordomus would compete (two cockpits, two context engines, two APIs) unless Lumen owned only interaction and projection.

## Decision

Lumen is the name of a composed view: it reads the runtime's existing capabilities (blackboard, peers, mesh, monitor, intents) and invokes existing capabilities for any action. It adds no scheduler, event bus, store, registry or API namespace of its own. Its first slice is one page with freshness and reconnect recovery and one authorised action. Whether the name also becomes a route, CLI command, MCP namespace or skill is decided by the owner separately; until then the page is /ui/mesh. Scheduled reports, federation across Prismatic projects and a Pages gateway are out of its scope until each has its own measured problem.

## Alternatives rejected

- Lumen as a separate application or repository
- Lumen as an orchestration layer above the mesh
- A lumen.* capability namespace duplicating mesh.*

## Consequences

Lumen work is post-alpha and sequenced after 5a's blackboard and f6's /proceed entrypoint, each proposed on its own unlanded branch. docs/CONCEPTS.md gains one term. Intent the-owner-oversees-the-mesh-from-one-page carries it.

## Sources

Promoted on 2026-10-18 from the owner's ChatGPT project (workspace `chatgpt-majordomus`, issue I3100). The owner has not accepted it; it is proposed for the owner's decision.

- `chatgpt-majordomus:c296#0`
- `chatgpt-majordomus:c296#8`
- `chatgpt-majordomus:c296#9`
- `chatgpt-majordomus:c296#14`
- `chatgpt-majordomus:c296#18`
