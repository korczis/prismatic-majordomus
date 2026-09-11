---
schema: adr/v1
id: adr-0040
kind: adr
title: Development semantics are capabilities of one runtime; every surface is a consumer
status: accepted
date: 2026-09-10
tags:
  - architecture
  - capabilities
  - cockpit
  - runtime
  - governance
related:
  - rule:project.development-semantics-are-canonical
  - rule:project.rust-canonical-declaration
  - file:docs/DEVELOPMENT_RUNTIME.md
  - file:scripts/development-semantics-check
provenance:
  origin: authored
---

## Context

The Cockpit is to become a browser-based software-development surface: issues,
milestones, sessions, handovers, peers, workflows, rules, decisions, git. The question
this decision answers is not what the Cockpit should show. It is who owns the semantics
it would show, because the answer as measured today is: a program the Cockpit cannot
reach.

This repository runs two executables, and the split between them is not a layering, it
is a fault line.

`bin/majordomus` is a shell tool. `majordomus --help` groups its commands as TASK,
CONTEXT, MEMORY, RULES, PLAN and SYSTEM — `start`, `check`, `finish`, `session`,
`checkpoint`, `evidence`, `handover`, `decision`, `question`, `plan`, `adr`, `usecase`.
That list is the development lifecycle. Its implementation is `lib/*.sh`, and `lib/` is
the only writer of the objects the lifecycle produces: `lib/plan.sh:514-515` is the one
place an issue or milestone YAML is written; `lib/session.sh` is the one place a session
record is written; `lib/evidence.sh` is the one place an obligation is discharged.

`bin/majordomus-cli` is the Rust executable. It owns the capability registry and every
projection of it — MCP, HTTP, OpenAPI, Swagger, the Cockpit, the command line, the
generated reference. `majordomus-cli --help` states the split in the negative: *"The task
lifecycle (init, start, check, finish, doctor, ...) is the shell tool bin/majordomus in
the same repository; this executable does not implement those commands."*

Three measurements make the consequence exact.

`majordomus-cli capabilities list --format json` reports 1236 capabilities, of which 78
are built in. Of those 78, exactly three have `kind: command` — `executions.start`,
`executions.cancel` and `peers.announce`. Every other capability of the runtime is a
query or a resource. No development object can be mutated through the runtime at all.

`majordomus-cli commands graph --format json` reports 163 commands over three origins:
95 `executable`, 40 `workflow`, 28 `tool`. All 28 tool-origin commands carry a
`projections.withheld` reason — 21 as *"asks the person something; a request/response
surface would hang"*, four as *"no capability declares this command line"*, three as
*"effect RepositoryMutation is above the machine ceiling LocalMutation"*. Not one of them
reaches MCP, HTTP or the Cockpit. The same measurement shows the tool half of the graph
is one level deep — 28 nodes, all at depth 1 — while the executable half reaches depth 3
in 77 of its 95 nodes. `majordomus plan done <id>` is not a node of the command graph at
all; only the group `majordomus plan` is.

`majordomus-cli executions list` answers *"no server is serving this repository; an
execution lives in the process that accepted it, so this command has nothing to read."*
The execution plane, which is where the runtime's only mutating capabilities live, keeps
nothing across a process.

The Cockpit is not the offender here. Its sources reference `.ai/` exactly once, as a
display label in `apps/majordomus-cli/src/cockpit/pages.rs`; it reads no state and writes
none. It is already a pure projection of capability output, which is what ADR 0013 and
`docs/COCKPIT.md` require of it. The defect is on the other side: the runtime it projects
has no development semantics to offer, so a Cockpit asked to become a development surface
has exactly two options — reimplement the lifecycle in the browser or in
`src/cockpit`, or shell out to `bin/majordomus`. Both make a second implementation of
development semantics, and the second implementation is the one that drifts.

Two further duplications are already live and would multiply under a second
implementation. Durable events and live events are disjoint models: `share/events.yaml`
registers 20 ledger event names written by `lib/*.sh` into `.ai/local/state/ledger.jsonl`,
while `apps/majordomus-cli/src/execution/event.rs` defines 15 stream messages
(`execution.created` … `stream.closing`) that no ledger records. Neither is a superset of
the other and nothing bridges them. And `docs/generated/` has two writers:
`apps/majordomus-cli/src/generate.rs` with its 15 targets, and a shell set including
`lib/usecase.sh`, `bin/majordomus`, `scripts/derive` and `scripts/generate-site-data`.

The repository already has a name for every stage a development surface needs — `issue`,
`milestone`, `task`, `context`, `session`, `workflow`, `execution`, `peer`, `capability`,
`event`, `evidence`, `obligation`. None of them needs inventing. What is missing is that
the runtime cannot change any of them.

## Decision

**The canonical development runtime is the capability registry of the Rust executable,
and development semantics are capabilities of it. Every surface — the Cockpit, the CLI,
the HTTP API, OpenAPI, MCP — is a consumer that requests operations from the runtime and
subscribes to its state and events. No surface implements a development semantic of its
own.**

Concretely:

1. **The registry is the boundary.** A development operation — moving an issue, opening or
   closing a session, recording a checkpoint or a handover, attaching evidence, proposing
   a decision, evaluating completion — exists as a `capability!` declaration under
   `apps/majordomus-cli/src/capability/builtin/`, with a typed input and output, and is
   exposed by the policy in `command_graph/policy.rs` rather than by hand. This is
   `project.rust-canonical-declaration@1` and ADR 0004 applied to the half of the model
   they do not yet cover.

2. **No new nouns.** Each stage of a development pipeline maps onto a concept the
   repository already has. `issue` and `milestone` are the plan; the unit of accepted work
   is the existing `task`, not a new "development task"; compiled context is the existing
   `context` builder and its `.ai/local/session-contexts/` freeze; a development session is
   the existing `session` episode; running work is the existing `execution`; an actor is
   the existing `peer`; completion is the existing `obligation` closure and finish
   contract. Introducing a synonym for any of these is a defect, not an extension.

3. **The shell tool becomes a consumer, not the owner.** `lib/*.sh` holding the only
   writer of a development object is transitional debt, recorded as such. Each lifecycle
   command converges on a runtime capability that the shell invokes, exactly as
   `lib/context.sh` and `lib/capture.sh` already invoke the Rust binary through
   `lib/rust_bin.sh`. The command graph is the ledger of that convergence: a
   `projections.withheld` reason of *"no capability declares this command line"* on a
   development command is the open item, stated by measurement.

4. **Storage is reused, never invented.** No new datastore. Issues and milestones stay
   `.ai/repo/project/{issues,milestones}/*.yaml`; closed sessions stay
   `.ai/repo/sessions/*.md`; the open session, the active task, checkpoints, handovers and
   questions stay under `.ai/local/state/`; the durable event log stays
   `.ai/local/state/ledger.jsonl` under the vocabulary in `share/events.yaml`; completion
   evidence stays the `evidence[]` array inside the plan object it discharges; context
   provenance stays `.ai/local/session-contexts/`; artifacts stay `docs/generated/**` and
   `site/data/**` written only by `generate::Target`. A capability that needs somewhere new
   to put something is a decision of its own, recorded before it lands.

5. **Two event tiers, named.** A durable state change is a ledger event with a name
   registered in `share/events.yaml` — that is the record. The typed execution stream over
   `/events` is the live view of a runtime in motion and is never the record. A development
   capability that mutates state appends to the ledger; the stream carries the same change
   as it happens. Neither tier may hold a durable fact the other cannot see.

6. **One derivation chain for anything exposed.** Canonical type (the `capability!`
   declaration, or for a layer object its kind in `share/kinds.yaml`) → serialization
   schema (`share/schemas/majordomus/<kind>/<kind>.v1.schema.json` or the generated
   `CanonicalSchema`) → HTTP route and `docs/generated/openapi.{json,yaml}` → MCP tool or
   resource → the Cockpit page that renders that capability's JSON. Every link is
   generated, and `majordomus capabilities validate`, `majordomus generate --check` and the
   `projection-closure` gate refuse a break in it.

The invariant, stated plainly: **the Cockpit does not execute development semantics. It
requests capabilities of the canonical runtime and subscribes to canonical state and
events.** The same sentence holds with "the CLI", "the HTTP API" or "MCP" in place of "the
Cockpit".

`docs/DEVELOPMENT_RUNTIME.md` carries the architecture, the ownership boundaries and the
measured inventory this decision rests on. `project.development-semantics-are-canonical@1`
makes it a rule, ratcheted against the debt above rather than asserted over it.

## Alternatives rejected

**Let the Cockpit call `bin/majordomus`.** The fastest path and the reason this decision
exists. A surface that shells out owns the argument construction, the exit-code
interpretation, the interactivity workaround and the error rendering — four semantics, in
the surface, undeclared. The command graph already refuses it for a stated reason on all
28 tool commands, and `project.commands-run-non-interactively@1` refuses the
interactivity half outright.

**Implement development semantics in the Cockpit and let the CLI catch up.** This is the
failure the repository was built to catch, and it has already happened here once: two
sessions built the same release subsystem in parallel. A second implementation of a
development semantic is a mirror in the sense of `docs/DYNAMICITY.md`, and mirrors drift
on the first edit that forgets one side.

**Port `lib/*.sh` to Rust wholesale first, then build the surface.** 13,873 lines with the
plan, the ledger, the doctrine dispatch and the finish contract inside them. A rewrite
that must land before anything else can start is a rewrite that blocks the work it was
meant to enable, and workers are extending these abstractions in parallel right now. The
decision names the boundary and lets each capability cross it on its own schedule; the
command graph reports how far the crossing has got.

**A new "development object" model beside the layer.** Rejected for the reason
`docs/DYNAMICITY.md` gives: a fact with two owners has none. The layer already types
every object involved, `share/kinds.yaml` already declares the contracts, and
`.ai/repo/knowledge/sources.yaml` already binds pathspec to kind. A parallel model would
be a second canonical owner by construction.

**Persist executions in a new store so the Cockpit can show history.** Tempting because
`executions list` visibly has nothing to read. Rejected as written: the durable record
already exists and is the ledger, and a second durable event log would reproduce the
`share/events.yaml` defect the vocabulary was introduced to fix. The execution store stays
bounded and in-process; durability is the ledger's job.

## Consequences

**What becomes possible.** A development surface can be built without deciding anything
about development semantics. The Cockpit issue page, the `plan` MCP tool, the HTTP route
and the CLI subcommand are four projections of one declaration, and adding the fifth costs
nothing. The workers extending issues, milestones, the context compiler and completion
gates in parallel have one place to converge and one name for each concept, so the
convergence is checkable rather than negotiated.

**What becomes measurable.** The distance still to travel is a number a command prints:
the development commands whose `projections.withheld` reason is *"no capability declares
this command line"*, and the tool-origin nodes that are still depth 1. Progress is a
shrinking baseline, not an assertion.

**What this costs.** Every development mutation that moves into the runtime needs a typed
input schema, benchmark cases, a use case that executes, and the interactivity removed
from the command that fronts it. That is the standing price of
`project.rust-canonical-declaration@1`, and it is why the rule ratchets rather than
blocks: the debt is 28 commands wide, and a rule that failed on all of it on the day it
landed would be turned off within a day.

**What stays broken for now.** The two writers of `docs/generated/` remain. The ledger and
the execution stream remain disjoint. Session contexts, knowledge, skills, prompts,
questions, history and search have a shell owner and no runtime capability at all. Each is
an entry in the inventory in `docs/DEVELOPMENT_RUNTIME.md` with what it would take to
close it; none is closed by this decision, and this decision does not pretend the boundary
it draws is where the code is today.

**What must not happen.** A surface that grows a development semantic of its own — a
status computed in a template, a transition decided in JavaScript, an issue field written
by anything but the runtime. The rule and its gate exist for that case, and the ratchet
means a new violation fails even while the old ones are tolerated.
