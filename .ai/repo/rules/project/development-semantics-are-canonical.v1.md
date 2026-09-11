---
id: project.development-semantics-are-canonical
version: 1
kind: rule
title: Development semantics live in the canonical runtime; a surface only consumes them
description: A semantic of the development lifecycle — the plan, tasks, sessions, context, executions, peers, evidence, obligations and completion — is declared once as a capability of the Rust executable's registry, and the Cockpit, the CLI, the HTTP API, OpenAPI and MCP request it rather than deciding it; a surface that computes a status, decides a transition or writes a development object itself is a second implementation and a defect.
statement: A development semantic shared by more than one surface is declared once as a capability of the canonical runtime and consumed by every surface; no surface — Cockpit, CLI, HTTP, OpenAPI or MCP — implements a development semantic of its own, shells out to another program to obtain one, or writes a development object except through the runtime.
status: active
class: blocking
depends_on: [project.rust-canonical-declaration@1, project.interfaces-are-projections@1, project.commands-are-projections@1]
tags: [architecture, capabilities, cockpit, runtime]

x-majordomus:
  tests: [scripts/development-semantics-check]
---

# Rationale

The Cockpit is becoming a development surface, and a development surface has to answer
questions like "is this issue ready?", "what does this task still owe?", "may this session
close?". Every one of those is a semantic, and every one of them already has exactly one
correct answer in this repository. The risk is not that a surface answers them wrongly. It
is that a surface answers them at all, because a second answer is right on the day it is
written and diverges on the first change to the first one.

The measurement that makes this concrete is in ADR 0040. Of the 78 built-in capabilities
`majordomus-cli capabilities list --format json` reports, three can mutate anything, and
all 28 tool-origin nodes of `majordomus-cli commands graph --format json` are withheld
from every machine surface. So the runtime today offers a development surface almost
nothing to consume, and the pressure on that surface to decide things for itself is real
rather than hypothetical. This rule exists to make the pressure resolve towards the
registry instead of into a template.

The rule is deliberately not "the Cockpit must be a projection" — `docs/COCKPIT.md` and
ADR 0013 already say that, and the Cockpit already obeys them: its sources name `.ai/`
once, as a display label. This rule is the converse, aimed at every surface at once,
including the shell tool that currently owns the semantics. It says where a semantic
lives, so that adding a surface never means adding an implementation.

# Required behaviour

A development semantic is any rule, derivation or transition over the plan (`issue`,
`milestone`), the active `task`, compiled `context`, a `session`, an `execution`, a `peer`,
an `event`, `evidence`, an `obligation` or completion. Each one is declared once, as a
`capability!` under `apps/majordomus-cli/src/capability/builtin/`, with a typed input and
output, and reaches its surfaces through the exposure policy in
`command_graph/policy.rs` — never through a hand-written route, tool list or page.

A surface consumes. The Cockpit renders capability output and posts capability input; it
computes no status, decides no transition, and reads and writes no file of the layer. The
CLI, the HTTP API, OpenAPI and MCP are the same registry projected differently. No surface
spawns another program to obtain a development semantic: a surface that shells out owns
the argument construction, the exit-code interpretation and the error rendering, which are
semantics wherever they are written.

A development object is written only by the runtime. Its storage is the storage the
repository already has — `.ai/repo/project/{issues,milestones}/*.yaml`,
`.ai/repo/sessions/*.md`, `.ai/local/state/` and its `ledger.jsonl`, `evidence[]` inside
the plan object it discharges, `docs/generated/**` through `generate::Target`. A capability
that needs a new place to put something records a decision first.

A durable state change is a ledger event whose name is registered in `share/events.yaml`.
The typed execution stream over `/events` is the live view and never the record. Neither
tier may hold a durable fact the other cannot see.

Anything exposed follows one derivation chain: canonical declaration → serialization
schema → HTTP route and generated OpenAPI → MCP tool or resource → the Cockpit page that
renders it. A surface added anywhere along that chain by hand, rather than derived, is a
violation of this rule and of `project.rust-canonical-declaration@1`.

The transitional debt is accepted explicitly, not silently. `lib/*.sh` is the only writer
of most development objects today — `lib/plan.sh:514-515` writes every issue and milestone
YAML, `lib/session.sh` every session record, `lib/evidence.sh` every discharge. Those
commands are listed in `.ai/repo/development-semantics-baseline.txt` and are permitted to
stay there while they converge on capabilities. The baseline may shrink and may not grow.

# Failure behaviour

A violation is a `FAIL` from `scripts/development-semantics-check`, run in CI as the gate
`development-semantics` in the `structure` job, and the gate exits 10 with each violation
named and its file given.

The gate decides two things, both from tracked files and without building anything:

- **An unbacked development command.** A public command of `share/commands.yaml` whose
  `class` mutates and whose `writes` names a development object tree
  (`.ai/repo/project/`, `.ai/repo/sessions/`, `.ai/repo/adrs/`, `.ai/local/state/`), for
  which `docs/generated/registry.json` declares no capability of kind `command` in the
  matching module. Such a command has no typed input schema on any machine surface, so a
  surface that wants its semantic must reimplement it. The baseline is a ratchet: a command
  that leaves it may not return, and one that was never in it fails on the day it appears.
- **A surface reaching into the layer.** A reference to `.ai/repo`, `.ai/local` or a
  `majordomus` executable inside `apps/majordomus-cli/src/cockpit/**` or
  `share/cockpit/**`, excluding a generator's own provenance header. Those trees render
  capability output; a path or a subprocess in them is a second implementation starting.

Two things the gate does not decide, stated so that nobody mistakes its silence for a
pass. First, whether a derivation inside a surface is a *semantic* or a presentation
choice: a template that pluralises a count is not a second implementation and a template
that computes readiness is. A gate that guessed would be unauditable, so it does not, and a
reviewer decides — exactly as `majordomus.decision-threshold@1` leaves its own threshold to
a reviewer. A change that puts a semantic in a surface and passes the gate is still refused
at review. Second, who writes a development object: a write in shell has no reliable
syntactic shape, and a grep for the object trees would report every reader as a writer.
Deciding it would take the writers declared as data — the `writes` field of
`share/commands.yaml` extended to name the writing function, reconciled against `lib/`
the way `exit_codes` already is — and that is an open item rather than a check.

# Verification

`scripts/development-semantics-check` decides the three mechanical checks and prints its
counts; `just gate development-semantics` runs it locally, and the `structure` job runs it
in CI from `.ai/repo/ci/gates.yaml`. The measurements it rests on are reproducible on
their own: `majordomus-cli capabilities list --format json` for what the registry offers,
and `majordomus-cli commands graph --format json` for which commands reach which surface
and why each withheld one is withheld.

The architecture, the ownership boundaries and the full measured inventory are
`docs/DEVELOPMENT_RUNTIME.md`; the decision is ADR 0040.
