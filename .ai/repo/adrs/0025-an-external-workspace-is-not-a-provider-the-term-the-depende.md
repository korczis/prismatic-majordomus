---
schema: adr/v1
id: adr-0025
kind: adr
title: An external workspace is not a provider: the term, the dependency posture, and where synced content lives
status: proposed
date: 2026-09-09
tags:
  - architecture
  - workspace
provenance:
  origin: authored
  derived_from:
    - decision:adr-0005
    - decision:adr-0024
    - file:share/providers.yaml
    - file:share/knowledge-sources.yaml
    - file:apps/majordomus-cli/Cargo.toml
---

# 25. An external workspace is not a provider: the term, the dependency posture, and where synced content lives

## Context

The operator wants machine-readable access to his own ChatGPT Projects — enumerate them,
read their conversations, sync incrementally, and see the result through the surfaces this
repository already has. Official coverage is incomplete, so any implementation reaches for
an authenticated browser and observed private traffic for the part the vendor does not
document. That is a legitimate thing to build over one's own authorised account, and the
gray zone is not what this record is about.

What this record is about is that the design as it arrives collides with three things the
repository has already settled, and each collision is one a session would otherwise resolve
silently, in passing, in the middle of a phase that was supposed to be about something else.

The first is the word. `provider` is spent: ADR 0024 defines it as a tool that works *in*
this repository — a bootstrap file it reads, a client configuration that starts the shared
MCP server for it, and the hooks through which it hands Majordomus a prompt — and
`share/providers.yaml` is the table with `agents`, `claude`, `codex`, `gemini` and `bb` in
it. A ChatGPT Project is the opposite direction of the same arrow: content held elsewhere
that this repository reads. Both are named after the same vendors, and a session told to
"extend an existing equivalent invariant rather than duplicate it" would merge them. The two
neighbouring nouns are spent too. `project` is a declared kind, `majordomus.project/v1`, and
it is *this* repository's own record; `source` is the knowledge compiler's class in
`share/knowledge-sources.yaml`.

The second is the dependency posture. `majordomus-cli` has eleven dependencies and the manifest
argues for each one: no YAML crate because the layer's YAML is the subset `docs/SCHEMAS.md`
defines, `jsonschema` with `default-features = false` because nothing may resolve over the
network, `tiny_http` because the loopback server is synchronous and there is no async runtime
and no framework. There is no HTTP client and no TLS anywhere in the tree. CDP is a WebSocket
carrying JSON-RPC and private HTTP is TLS; adding either to this crate is not a choice of
crates but a change of category, and it lands on the one executable `bin/majordomus-mcp`
builds on demand — already, per ADR 0024, slower than the two seconds a settings-loaded MCP
server is given before the first turn.

The third is mutability. Everything the tool does today is a read-only projection of files
that are in the repository. A workspace sync is a stateful network client with a durable
store, holding content the repository did not write and may not be entitled to publish.
ADR 0005 already draws the line it needs — `.ai/local/**` is checkout state, never a source —
but nothing in the incoming design says which side of it the store is on.

## Decision

**A workspace is not a provider.** The external thing gets its own noun: a *workspace* is a
body of content held by another vendor over which the operator is already authenticated, and
which this repository reads. The code that reaches one is a *workspace adapter*; the ways it
reaches it are *transports*. `provider` keeps the meaning ADR 0024 gave it and no adapter
takes that word, no row in `share/providers.yaml`, and no field of the provider table. That
Claude Code is a provider and a Claude Project would be a workspace is the distinction, not
an ambiguity to be smoothed over.

**The network code does not enter `majordomus-cli`.** `apps/` becomes a Cargo workspace and
the subsystem is a second crate beside the first. `majordomus-cli` acquires no dependency
from this work — not an async runtime, not an HTTP client, not TLS, not a CDP client — and a
gate proves it. Where the CLI must cause a sync, it starts the other binary as a process; it
never links it.

**Synced content is checkout state.** A workspace's content lands under
`.ai/local/workspaces/<workspace>/`, which is what ADR 0005 already says it is: state, never
a source. It is not an input to `derive`, not an input to the site, and never served on a
public surface. Discovery reuses the mechanism that exists for exactly this shape — a state
class in `share/knowledge-sources.yaml`, `discovery: state`, one directory, no recursive walk
of anything that happens to exist — so the synced corpus is visible to `knowledge`, `search`
and `context` on the day it lands without a second registry being invented for it.

Content becomes the repository's own statement only when a person promotes it into
`.ai/repo/`, in the shape the layer already uses for a decision recorded while working and
later written down as an ADR. No sync writes to the tracked tree.

**Credentials never touch the repository.** The browser adapter attaches to an
already-authenticated browser profile named by configuration outside the tree. No cookie,
token or authorisation header is copied into a file the repository can read, and none reaches
a fixture, a log, a snapshot or the state directory.

## Alternatives rejected

**A row in `share/providers.yaml`.** The provider table knows three things about a tool — its
bootstrap, its client configuration, its hooks — and not one of them means anything for a
corpus held elsewhere. Filling the table with a member that answers none of its questions
costs the table its meaning, and buys a shared word that was already the source of the
confusion.

**Async, an HTTP client and a CDP crate inside `majordomus-cli`.** It is the shorter path and
it spends the property the crate was built around. Every agent session pays the build, every
`doctor` run pays the link, and the argument in the manifest for refusing a YAML crate stops
being true the moment a TLS stack is in the tree for an unrelated feature.

**Synced content under `.ai/repo/`.** It is not this repository's statement, it is
machine-local, it is potentially large and potentially private, and ADR 0005 has already
answered the question in general terms.

**A separate repository.** The canonical model, the schemas, the kinds and every surface are
here; a split would recreate across two repositories the duplication the layer forbids inside
one.

## Consequences

The incoming plan changes shape in two places. Its "canonical provider core" is a crate
boundary rather than a trait inside the existing binary, and its "one application layer" for
CLI, REST, OpenAPI, MCP and Cockpit is not a new service to be written: synced content that
lands as declarative objects under a discovered state class is projected to every surface by
the machinery that already exists. That is the cheaper design here and the one the doctrine
demands; a session following the plan literally would have built a parallel one.

`apps/` gains a workspace manifest, and the distribution model gains a question it does not
yet have an answer to: whether the second binary ships with the tool or stays a developer
artifact, and what `doctor` should say on a machine where it is absent. That is left open
deliberately; it is decided when there is something to ship.

A gate must assert that `majordomus-cli`'s dependency list did not grow, or the boundary is
a sentence rather than a constraint.

Nothing synced is public until a person promotes it, which means the first useful output of
this subsystem is visible to the operator's own tooling and to no one else. That is the
intended asymmetry.
