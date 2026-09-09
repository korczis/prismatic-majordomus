---
schema: adr/v1
id: adr-0030
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
    - file:apps/majordomus-cli/src/discovery/mod.rs
    - file:scripts/lib/ui-audit.mjs
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

An audit taken before this record was accepted established two things that a first reading of
the layer gets wrong, and both change the answer.

`discovery: state` is not the shared mechanism it looks like. `share/knowledge-sources.yaml`
has exactly one reader, `lib/knowledge.sh:30`, the shell knowledge compiler; the dispatch is
`lib/knowledge.sh:99-103` and the reader `mj_kdisc_state` is a non-recursive `ls -1` over one
directory (`lib/knowledge.sh:130-146`). The Rust index — the one that feeds `objects.list`,
every MCP resource, every HTTP route, OpenAPI and the Cockpit — knows a single discovery,
`DiscoveryKind::Vcs`, tracked files through the git index
(`apps/majordomus-cli/src/discovery/mod.rs:33-37`), and excludes `.ai/local` at three
independent points (`app.rs:82-84`, `repository.rs:176-178`, `discovery/mod.rs:283-285`). A
state class therefore buys visibility in `majordomus knowledge` and nowhere else. `search` and
`context` do not read it either: both hardcode their record kinds and paths
(`lib/search.sh:44-51`, `lib/context.sh:176-262`). The one precedent for local state reaching
a capability at all is `continuity`, a hand-written reader over a hardcoded `STATE_DIR`
(`apps/majordomus-cli/src/capability/builtin/continuity.rs:51`) exposed as exactly one
resource that is, in its own words, served and never published.

The repository already drives a browser, and it is not Rust. `scripts/lib/ui-audit.mjs:262`
and `scripts/lib/cockpit-probe.mjs:436` both call `chromium.launch({ channel: 'chrome' })` —
Playwright over the system-installed Chrome, deliberately never a downloaded one — with
`playwright` and `axe-core` as root devDependencies, wired into `validate.yml` for the `site`
and `cockpit` jobs, and skipping cleanly with a defined exit code where Chrome is absent.
Playwright's transport to that browser is CDP.

## Decision

**A workspace is not a provider.** The external thing gets its own noun: a *workspace* is a
body of content held by another vendor over which the operator is already authenticated, and
which this repository reads. The code that reaches one is a *workspace adapter*; the ways it
reaches it are *transports*. `provider` keeps the meaning ADR 0024 gave it and no adapter
takes that word, no row in `share/providers.yaml`, and no field of the provider table. That
Claude Code is a provider and a Claude Project would be a workspace is the distinction, not
an ambiguity to be smoothed over.

**The network code does not enter `majordomus-cli`, and it is not a second crate either.**
The transport and the sync live in the Node tooling layer that already drives Chrome:
Playwright over the system browser, the posture `scripts/lib/*.mjs` established and CI already
runs. `majordomus-cli` acquires no dependency from this work — not an async runtime, not an
HTTP client, not TLS, not a CDP client — and a gate proves it. `apps/` stays a single crate.
Writing a CDP client in Rust would reimplement, in the language with the strictest dependency
budget in this repository, the one thing the repository already has a working, CI-wired
implementation of.

**The CLI's share of the subsystem is one read capability.** It is written in the shape
`continuity` established: a hand-written reader over the state directory, one capability,
served and never published, with no `docs/generated/` or site projection. There is no free
ride: the kind pipeline reaches tracked files only, and nothing under `.ai/local` is indexed
by construction and by three explicit guards.

**Synced content is checkout state.** A workspace's content lands under
`.ai/local/workspaces/<workspace>/`, which is what ADR 0005 already says it is: state, never
a source, git-ignored at `.gitignore:55` and gated by `mj_validate_ai_layout`
(`lib/doctor.sh:431-436`) which fails the pre-commit hook if a file there is ever tracked. It
is not an input to `derive`, not an input to the site, and never served on a public surface.

It is deliberately **not** declared as a class in `share/knowledge-sources.yaml`. That would
buy visibility in one shell command and charge for it in the worst possible currency: the
knowledge compiler hashes the full content of every file it discovers (`lib/knowledge.sh:80`,
`lib/common.sh:465-470`) and re-parses each one afterwards, so a synced corpus of thousands of
conversations would make `majordomus knowledge` linear in the size of a body of text that
command has no reason to read.

Its retention is its own. `mj_validate_retention` (`lib/doctor.sh:303-316`) is three
hand-written stanzas over the ledger, handovers and checkpoints; a new store gets no cap and
no `doctor` line unless one is written for it. One is written for it, because an unbounded
store that nothing measures is the defect this repository keeps finding in itself.

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

**A second Rust crate in a Cargo workspace.** This record proposed it before the audit and
the audit withdrew it. It keeps the dependency boundary but pays for it twice: a hand-written
CDP client beside a working Playwright one, and a second binary to build, ship and explain,
for a subsystem whose browser half the repository can already perform.

**A `discovery: state` class for the synced corpus.** It reads as the reuse the doctrine
demands and is not: one shell command gains it, no surface does, and the compiler behind that
command hashes and re-parses every byte it is given.

**Synced content under `.ai/repo/`.** It is not this repository's statement, it is
machine-local, it is potentially large and potentially private, and ADR 0005 has already
answered the question in general terms.

**A separate repository.** The canonical model, the schemas, the kinds and every surface are
here; a split would recreate across two repositories the duplication the layer forbids inside
one.

## Consequences

The incoming plan changes shape in three places. Its "canonical provider core" is a Node
module beside the existing browser tooling, not a trait inside the Rust binary and not a crate
beside it. Its "one application layer" projecting the corpus through CLI, REST, OpenAPI, MCP
and Cockpit is not available at all: the machinery that would have carried it reaches tracked
files only, so the CLI's share is one `continuity`-shaped capability and the rest of the plan's
surface list is a thing that would have to be built, deliberately, against the grain, for
content the operator has not said should be public. Its sync engine gets a retention cap
written by hand, because nothing generic would ever notice the store.

The subsystem is therefore two halves in two languages with a directory between them: Node
writes `.ai/local/workspaces/`, Rust reads it, and neither links the other. That boundary is
the file system, which is the same boundary `continuity` already lives on.

The distribution model gains a question it does not yet have an answer to: the Node tooling is
a devDependency of this repository, not something the installed tool carries, so a workspace
sync is an operator's instrument in a checkout rather than a feature of the released binary.
Whether that changes is left open; it is decided when there is something to ship.

A gate must assert that `majordomus-cli`'s dependency list did not grow, or the boundary is
a sentence rather than a constraint.

Nothing synced is public until a person promotes it, which means the first useful output of
this subsystem is visible to the operator's own tooling and to no one else. That is the
intended asymmetry.
