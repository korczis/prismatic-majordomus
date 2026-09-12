+++
title = "The development runtime"
description = "the development runtime: which program owns the semantics of the plan, tasks, sessions, context, executions, peers, evidence and completion, the measured inventory of every one of them, the storage each is decided to keep, the two event tiers, the derivation chain anything exposed must follow, what each surface may and may not decide, and the ranked gaps between that boundary and the code"
weight = 36
[extra]
source = "docs/DEVELOPMENT_RUNTIME.md"
+++

{% raw %}

What owns the semantics of software development in this repository, where each one lives,
which surface may decide what, and how far the code is from that boundary today.

The companion documents are [`DYNAMICITY.md`](@/docs/dynamicity.md), which decides where a *fact*
lives, and [`CAPABILITIES.md`](@/docs/capabilities.md), which decides how an *operation* is
declared. This document is the third of that set: it decides where a *semantic* lives. The
decision is [ADR 0040](../.ai/repo/adrs/0040-development-semantics-are-capabilities-of-one-runtime.md);
the rule is `project.development-semantics-are-canonical@1`.

Every count in this document was measured. The commands are in
[How this was measured](#how-this-was-measured); none of them is copied from prose, and a
statement about behaviour that is not yet implemented is phrased as a target.

## The invariant

> The canonical development runtime is the capability registry of the Rust executable.
> Development semantics are capabilities of it. The Cockpit does not execute development
> semantics; it requests operations from the canonical runtime and subscribes to canonical
> state and events. The same sentence holds with "the CLI", "the HTTP API", "OpenAPI" or
> "MCP" in place of "the Cockpit".

A *development semantic* is any rule, derivation or transition over the plan, the active
task, compiled context, a session, an execution, a peer, an event, evidence, an obligation
or completion. "Is this issue ready?", "what does this task still owe?", "may this session
close?", "which wave does this issue belong to?" — each has exactly one correct answer,
and the runtime is where that answer is computed.

## The measurement this rests on

Two executables serve this repository, and the line between them is not a layering.

`bin/majordomus` is a shell tool implemented under `lib/`. Its own help groups its
commands as TASK, CONTEXT, MEMORY, RULES, PLAN and SYSTEM: `start`, `check`, `finish`,
`session`, `checkpoint`, `evidence`, `handover`, `decision`, `question`, `plan`, `adr`,
`usecase`. That list is the development lifecycle, and `lib/` is the only writer of what it
produces — `lib/plan.sh:514-515` is the one place an issue or milestone YAML is written.

`bin/majordomus-cli` is the Rust executable. It owns the capability registry and every
projection of it: MCP, HTTP, OpenAPI, Swagger, the Cockpit, the command line, the generated
reference. Its help states the split in the negative: *"The task lifecycle (init, start,
check, finish, doctor, ...) is the shell tool bin/majordomus in the same repository; this
executable does not implement those commands."*

These are the measurements that make the consequence exact. Each states what is true,
not how many; the command that reproduces the tally is in
[How this was measured](#how-this-was-measured).

<div class="overflow-x-auto" tabindex="0">

| Measurement | What it shows |
|---|---|
| the registry | built-in operations, plus one resource per object of the layer; `capabilities list` prints both tallies in its `summary` |
| built-in capabilities that can **mutate** anything | only `executions.start`, `executions.cancel` and `peers.announce`. Every other capability of the runtime is a query or a resource |
| the command graph | three origins — `executable`, `workflow` and `tool` |
| tool-origin commands reaching **any** machine surface | none. Every one of them carries a `projections.withheld` reason |
| the shape of the tool half of the graph | every tool node is a group at the first level, so `majordomus plan done <id>` is not a node at all; the executable half nests to its subcommands |
| durable executions | none — `executions list` answers *"an execution lives in the process that accepted it"* |
| exposure of the built-in capabilities | every one reaches HTTP; all but `health.live` and `health.ready` reach MCP; **fewer than half reach the command line** — `objects.get`, `repository.info`, every `plan.*` and `graph.*`, and `health.report` have no CLI projection |

</div>


The graph gives four withheld reasons: *"asks the person something; a request/response
surface would hang"*, *"effect RepositoryMutation is above the machine ceiling
LocalMutation"*, *"no capability declares this command line, so no machine surface has a
typed input schema to execute from"*, and *"groups other commands; nothing to execute"*.
Most of the graph is withheld from every machine surface, and the commonest reason is the
third.

So the runtime, today, cannot change a single development object, and none of the
lifecycle is reachable from the Cockpit, MCP or HTTP.

**The Cockpit is not the offender.** Its trees reference the layer exactly once, as a
display label in `apps/majordomus-cli/src/cockpit/pages.rs:1787`; it reads no state and
writes none. It is already the pure projection that [`COCKPIT.md`](@/docs/cockpit.md) and ADR 0013
require. The defect is on the other side: the runtime has no development semantics to
offer, so a Cockpit asked to become a development surface has two options — reimplement the
lifecycle, or shell out to `bin/majordomus`. Both create a second implementation, and the
second one drifts.

## The canonical model, in repository-native names

The repository already has a name for every stage a development surface needs. Introducing
a synonym for one of them is a defect, not an extension. The mapping below is the model; a
worker extending any stage converges on the name in the middle column.

<div class="overflow-x-auto" tabindex="0">

| Stage | Repository-native name | Canonical owner (target) | What exists today |
|---|---|---|---|
| Issue / Milestone | `issue`, `milestone` — **the plan** | `plan` capability module | 8 read-only capabilities; the write half is `lib/plan.sh` |
| Development task | **`task`** — the existing active task. *No new noun.* | a `task` capability module | `lib/start.sh`, `check.sh`, `finish.sh`; readable only through `continuity.state` |
| Compiled context | **`context`** — the context builder and its freeze | a `context` capability module | `lib/context.sh`, `lib/context_docs.sh`; the Rust half is `directories.list`, which reports *contracts*, not compiled context |
| Development session | **`session`** — the existing episode | a `session` capability module | `lib/session.sh`; closed records are readable as `session` resources, the open one through `continuity.state` |
| Workflow execution | **`execution`** (the plane) over a **`command`** of the graph | `executions` module | exists, and holds 2 of the 3 mutating capabilities; keeps nothing across a process |
| Agent / peer activity | **`peer`**, and `Actor` inside an execution | `peers` module | exists, in-memory; `peers.announce` is the third mutating capability |
| Capabilities | **`capability`** | `capabilities` module | exists and is canonical |
| Events + evidence | **`event`** (two tiers), **`evidence`**, **`obligation`** | ledger vocabulary + `obligations` module | ledger is durable and local; the execution stream is live and in-process; the two are disjoint |
| Completion evaluation | **`obligation` closure** and the **finish contract** | `obligations` module | `obligations.closure` reads it; `lib/finish.sh` decides it |

</div>


Two stages the mandate's pipeline names separately are one thing here, and saying so
matters more than the diagram: a *development task* is the existing `task`, not a new
object beside it, and a *workflow execution* is an `execution` of a `command` of the
command graph, not a third scheduler.

## Ownership boundaries

```text
                    ┌───────────────────────────────────────────────┐
                    │  THE LAYER  .ai/repo (shared) + .ai/local     │  storage
                    │  typed by share/kinds.yaml,                   │
                    │  discovered by .ai/repo/knowledge/sources.yaml│
                    └───────────────────────┬───────────────────────┘
                                            │  reads and writes
                    ┌───────────────────────▼───────────────────────┐
                    │  THE CANONICAL RUNTIME                        │
                    │  apps/majordomus-cli/src/capability/          │
                    │  one capability! per operation, typed in/out  │  semantics
                    │  exposure derived by command_graph/policy.rs  │
                    └──┬────────┬────────┬─────────┬────────┬───────┘
                       │        │        │         │        │         projections
                  ┌────▼──┐ ┌───▼───┐ ┌──▼───┐ ┌───▼───┐ ┌──▼─────┐
                  │Cockpit│ │HTTP+WS│ │OpenAPI│ │  MCP  │ │  CLI   │  surfaces
                  └───────┘ └───────┘ └───────┘ └───────┘ └────────┘
```

**The layer owns storage.** Every development object is a typed file of `.ai/`. Its kind is
declared in `share/kinds.yaml`, whose declarative kinds the registry projection lists, its
contract in `share/schemas/majordomus/<kind>/<kind>.v1.schema.json`, and its location in
`.ai/repo/knowledge/sources.yaml` as a `:(glob)` pathspec. Three files agree or the object
has no type and no authority.

**The runtime owns semantics.** One `capability!` declaration per operation, with a typed
input and output. Where it is exposed is *derived* from its effect and interactivity by
`command_graph/policy.rs`, never chosen per surface.

**A surface owns presentation and nothing else.** It renders capability output and posts
capability input. It computes no status, decides no transition, reads and writes no file of
the layer, and spawns no program to obtain a semantic. A surface that shells out owns the
argument construction, the exit-code interpretation and the error rendering — three
semantics, in the surface, undeclared.

### Surface responsibilities

<div class="overflow-x-auto" tabindex="0">

| Surface | May do | May never do |
|---|---|---|
| Cockpit (`src/cockpit`, `share/cockpit`) | render capability JSON; post capability input; subscribe to `/events` | read or write `.ai/**`; compute a status or a transition; spawn a `majordomus` process |
| HTTP + WebSocket (`src/http`) | route to capabilities; carry the typed event stream | hold a route no capability declares |
| OpenAPI / Swagger | describe the registry | describe an operation the registry does not have |
| MCP (`src/mcp`) | expose capabilities as tools and resources | expose a tool that is not a capability |
| CLI (`src/cli.rs`) | the one projection declared twice, reconciled by `projection-closure` | grow a command with no capability behind it |
| Shell tool (`bin/majordomus`, `lib/**`) | *target:* invoke runtime capabilities, as `lib/context.sh` and `lib/capture.sh` already do through `lib/rust_bin.sh` | remain the only writer of a development object |

</div>


## Storage: decided, and reused

No new datastore. Every decision below is the storage the repository already has.

<div class="overflow-x-auto" tabindex="0">

| What | Canonical storage | Shared? | Why this one |
|---|---|---|---|
| Issues, milestones | `.ai/repo/project/{issues,milestones}/*.yaml` | tracked | already the canonical plan; schema `majordomus.{issue,milestone}/v1`; status is never stored, it is derived from timestamps and evidence |
| Active task | `.ai/local/state/current.yaml` | local | a task is one checkout's claim; sharing it would claim paths for every worktree |
| Development session, closed | `.ai/repo/sessions/*.md`, kind `session` | tracked | a closed episode is a durable shared record of what happened |
| Development session, open | `.ai/local/state/session-current.yaml` (+ `sessions-open/`) | local | an open episode belongs to the process holding it |
| Compiled context provenance | `.ai/local/session-contexts/<episode>` | local | it names this machine and freezes a projection at one moment; `.ai/README.md` forbids publishing it |
| Durable events | `.ai/local/state/ledger.jsonl`, vocabulary `share/events.yaml` (every name it registers) | local, append-only | already the canonical record; ledger *order* is load-bearing for `mj_record_rank` |
| Live execution events | the in-process store, streamed over `/events` (15 typed messages) | neither | bounded by design; a second durable log would reproduce the defect `share/events.yaml` was introduced to fix |
| Actor / peer state | the in-memory board (`src/peers.rs`) | neither | a peer *is* a connection; its durable trace is the ledger envelope's `by` and the session record's `worker` |
| Completion evidence | `evidence[]` inside the issue or milestone it discharges | tracked | evidence that lives away from the obligation it discharges is evidence nobody joins |
| Obligation vocabulary | `share/obligations.yaml` | shipped | a contract, so it stays literal |
| Artifacts | `docs/generated/**`, `site/data/**` via `generate::Target` | tracked | one writer, transactional, `--check`able |

</div>


**The two event tiers, named.** A durable state change is a ledger event whose name is
registered in `share/events.yaml`. The typed execution stream is the live view of a runtime
in motion and is never the record. A development capability that mutates state appends to
the ledger; the stream carries the same change as it happens. Neither tier may hold a
durable fact the other cannot see — which today it does, in both directions.

**Executions stay non-durable.** `executions list` visibly has nothing to read, and the
temptation is a store. Rejected as written: the durable record already exists and is the
ledger. The execution store stays bounded and in-process; durability is the ledger's job.

## Data flow, mutation flow, event flow

**Read.** A surface asks a capability. The capability reads the index — objects discovered
from `sources.yaml`, typed by `kinds.yaml`, validated against the kind's schema — or a
derivation over it (`plan.rs`, `graph.rs`, `product.rs`, `why.rs`, `obligations.rs`). No
surface reads a file.

<pre class="mermaid">
flowchart LR
  layer[".ai/**"] --&gt;|discovery| index["index"]
  index --&gt;|derivation| capability["capability"]
  capability --&gt;|projection| surface["surface"]
</pre>


**Mutate.** *Target flow.* A surface posts a typed input to a capability of kind `command`.
The capability validates against its input schema, applies the transition to the object in
its canonical storage, appends a registered ledger event, and returns the new state. The
same call over MCP, HTTP, the CLI and the Cockpit is one code path.

<pre class="mermaid">
flowchart LR
  surface["surface"] --&gt;|typed input| capability["capability"]
  capability --&gt; object["object in .ai/**"]
  object --&gt; ledger["ledger event"]
  ledger --&gt; events["/events"]
</pre>


*Actual flow today.* For the lifecycle's mutating commands, the surface is a terminal
and the capability does not exist: `bin/majordomus` dispatches into `lib/<name>.sh`, which
writes the object and appends the ledger line itself. That is the debt this document
records, ratcheted in `.ai/repo/development-semantics-baseline.txt`.

**Events.** Durable: `mj_ledger_append` refuses a name that `share/events.yaml` does not
declare and one missing a required field; `history --event` refuses to filter on an
undeclared name; `history --validate` reports a stored line no reader recognises. Live:
`ExecutionStore::publish` applies an event to the snapshot under one lock, refusing a
transition `ExecutionState::may_move_to` does not allow, assigns the sequence, retains the
event and hands it to subscribers — so a client reading the snapshot and a client reading
the stream cannot disagree.

## Consistency guarantees

<div class="overflow-x-auto" tabindex="0">

| Guarantee | Mechanism | Where |
|---|---|---|
| One typed definition per operation | the registry refuses to build on a capability composed outside its module, a duplicate id, or a benchmark policy contradicting the kind | `capability/registry.rs` |
| A projection cannot disagree with the registry | `capabilities validate`, `generate --check`, and the `projection-closure` gate over the one interface declared twice (the CLI) | `capabilities/`, `scripts/ci/projection-check` |
| An object cannot be untyped | three files must agree — manifest section, `sources.yaml` pathspec, `kinds.yaml` kind | `discovery/`, `metadata/` |
| Status cannot drift from facts | no status is stored anywhere; `lib/project.awk` derives BLOCKED / READY / ACTIVE / VERIFY / DONE / CANCELLED from timestamps, evidence and dependencies | `.ai/repo/project/project.yaml` |
| An unknown key is an error | generated per-kind allow-lists under `share/allow/` | `share/allow/*.txt` |
| A durable event cannot be mistyped | the registered vocabulary, enforced on append and on read | `share/events.yaml` |
| Snapshot and stream cannot disagree | one lock, one publish, refused illegal transitions, monotone sequence | `execution/store.rs` |
| A generated file cannot be hand-edited | transactional generation plus a fingerprint or diff against a fresh generation | `generate.rs`, the pre-commit hook |
| Live state cannot outlive its owner | the lease is read once; a server nobody owns has a bounded life | `lease.rs`, ADR 0035 |

</div>


What is **not** guaranteed today, stated so the list above is not read as more than it is:
a development object written by `lib/` is not validated against its capability's input
schema, because there is no capability; and a fact that reaches the ledger does not reach
the execution stream, or the reverse.

## The derivation chain every exposed object follows

<pre class="mermaid">
flowchart TD
  decl["canonical declaration&lt;br&gt;capability! under src/capability/builtin/&amp;lt;module&amp;gt;.rs&lt;br&gt;(or, for a layer object, its kind in share/kinds.yaml)"]
  schema["serialization schema&lt;br&gt;CanonicalSchema from the typed input/output&lt;br&gt;(or share/schemas/majordomus/&amp;lt;kind&amp;gt;/&amp;lt;kind&amp;gt;.v1.schema.json)"]
  http["HTTP route + OpenAPI&lt;br&gt;src/http/router.rs&lt;br&gt;docs/generated/openapi.json, openapi.yaml"]
  mcp["MCP tool or resource&lt;br&gt;majordomus://&amp;lt;kind&amp;gt;/&amp;lt;identity&amp;gt;,&lt;br&gt;or a tool by capability id"]
  cockpit["Cockpit contract&lt;br&gt;the page that renders that capability's JSON"]
  decl --&gt; schema
  schema --&gt; http
  http --&gt; mcp
  mcp --&gt; cockpit
</pre>


Every link is generated. Nothing on the chain is written by hand, and a hand-written entry
anywhere on it is a violation of `project.rust-canonical-declaration@1` (ADR 0004) as much
as of ADR 0040. The chain is checked by `majordomus capabilities validate`, `majordomus
generate --check`, and the `projection-closure` and `rust-generated` gates.

For a layer object the chain begins one step earlier, at the document schema: a Markdown
kind is described in `<kind>.v1.proto` (`Header` = front matter, `Body` = sections), from
which the JSON Schema, the front-matter allow-list under `share/allow/` and the body
section requirements under `share/sections/` are all generated.

## Extensibility: adding a development semantic

Short by design. If this list grows, the architecture has regressed.

1. Declare one `capability!` in its module's file under
   `apps/majordomus-cli/src/capability/builtin/`, with its typed input, typed output and
   the input's `BenchmarkCases`. A new module is one `module!` plus one name in
   `compose_modules!`.
2. If it mutates, give it `kind: command`, and append a ledger event — adding its name to
   `share/events.yaml` with the command that writes it and the payload keys it requires.
3. Run `majordomus generate`, then `majordomus capabilities validate`. Every surface —
   HTTP, OpenAPI, MCP, the Cockpit page, the generated reference, the benchmark target —
   follows without a second registration.
4. Run `majordomus usecase impact` and close any coverage gap it names with a use case that
   executes.
5. If a shell command fronted the semantic, point it at the capability and remove its entry
   from `.ai/repo/development-semantics-baseline.txt`. The baseline may shrink and may not
   grow.

No other file is edited for the operation to exist everywhere it should.

## The inventory

Measured, not recalled. **Owner** is who decides the semantic today; **Mutation path** is
the only thing that may write it. A concept whose owner is `lib/*.sh` is debt against
ADR 0040, not an architecture.

### The development lifecycle

<div class="overflow-x-auto" tabindex="0">

| Concept | Canonical owner today | Storage | Schema / type | Discovery | Consumers | Mutation path | Events | Tests | Docs | Drift / duplication |
|---|---|---|---|---|---|---|---|---|---|---|
| issue | read `src/plan.rs`; **write `lib/plan.sh`** | `.ai/repo/project/issues/*.yaml` | `majordomus.issue/v1` | `sources.yaml` class `issue` | `plan.*` queries, one resource per issue, the site, the GitHub projection | `lib/plan.sh:514-515` only | `plan_start`, `plan_verify`, `plan_evidence`, `plan_done` | `test/cases/*`, `tests/` | [`PLANNING.md`](@/docs/planning.md) | **write half unreachable from every machine surface** |
| milestone | as issue | `.ai/repo/project/milestones/*.yaml` | `majordomus.milestone/v1` | class `milestone` | `plan.model`, `plan.roadmap`, one resource each | `lib/plan.sh` | as issue | as issue | [`ROADMAP.md`](@/docs/roadmap.md) | as issue |
| task (active) | `lib/start.sh`, `check.sh`, `finish.sh` | `.ai/local/state/current.yaml` | `majordomus.current/v1` | not indexed (local) | `continuity.state`, `obligations.closure`, `scope` | `lib/` only | `task.started`, `task.checkpoint`, `task.evidence`, `task.finished` | `test/cases/32_refusal_lifecycle.sh` | [`CONTINUITY.md`](@/docs/continuity.md) | no capability; a refused `finish` writes no event at all |
| session (closed) | `lib/session.sh` | `.ai/repo/sessions/*.md` | `session/v1` | class `session` | 12 resources, `objects.*`, site | `lib/session.sh` | `session.closed` | `test/cases/*` | [`CONTINUITY.md`](@/docs/continuity.md) | no Rust module; write unreachable |
| knowledge (candidate) | `lib/knowledge.sh` | `.ai/repo/knowledge/candidates/*.md` | `knowledge/v1` | `sources.yaml` class `candidates` | `knowledge_base.candidates`, `knowledge_base.record`, `knowledge_base.status`, one resource per record, the briefing | `lib/knowledge.sh` only, at the episode boundary and by `knowledge derive|promote|reject` | `knowledge.derived`, `knowledge.promoted`, `knowledge.rejected` | `test/cases/*` | [`KNOWLEDGE.md`](@/docs/knowledge.md) | write unreachable from every machine surface, by decision: promotion is a person's act |
| session (open) | `lib/session.sh` | `.ai/local/state/session-current.yaml`, `sessions-open/` | `majordomus.session-record/v1` | not indexed | `continuity.state` | `lib/session.sh` | `session.started` | ” | ” | ” |
| session context | **`lib/session_context.sh` only** | `.ai/local/session-contexts/` | `majordomus.session-context/v1` | not indexed | the briefing, a person | `lib/` only | — | ” | `.ai/README.md` | **no Rust owner and no capability at all** |
| checkpoint | `lib/checkpoint.sh` | `.ai/local/state/checkpoints/` | — | not indexed | `continuity.state`, `context` | `lib/` only | `task.checkpoint` | ” | [`CONTINUITY.md`](@/docs/continuity.md) | no capability |
| handover | `lib/handover.sh` | `.ai/local/state/handovers/` | — | not indexed | `continuity.state`, `context` | `lib/` only | `task.handed_over` | ” | ” | no capability |
| decision (local) | `lib/decision.sh` | `.ai/local/state/decisions.md` | — | not indexed | `context`, `adr propose` | `lib/` only | `decision.recorded` | `test/cases/99_adr.sh` | [`DOCTRINE.md`](@/docs/doctrine.md) | two names for one concept: `lib/decision.sh` vs `lib/adr.sh`, and `release/changelog.rs` still references `.ai/repo/decisions/` |
| question | `lib/question.sh` | `.ai/local/state/open-questions.md` | — | not indexed | `continuity.state` counts blockers only | `lib/` only | `question.opened`, `question.resolved` | ” | [`CONTINUITY.md`](@/docs/continuity.md) | no capability |
| evidence | `lib/evidence.sh` | `evidence[]` in the plan object | inside `issue`/`milestone` schema | via the object | `plan.*`, `obligations.closure` | `lib/evidence.sh`, `lib/plan.sh` | `task.evidence`, `plan_evidence` | ” | [`PLANNING.md`](@/docs/planning.md) | no capability |
| obligation / closure | `capability/builtin/obligations.rs` (read) + `lib/finish.sh` (decide) | `share/obligations.yaml` + local state | shipped vocabulary | registry | `obligations.*`, `finish` | `lib/finish.sh` | `task.finished` | `test/cases/32` | [`CONTINUITY.md`](@/docs/continuity.md) | **the read and the decision are two implementations of one contract** |
| ADR | `lib/adr.sh` | `.ai/repo/adrs/????-*.md` | `adr/v1` | class `adr` | one resource each, `graph.get?id=adrs`, `release.changelog` | `lib/adr.sh` | `adr.proposed` | `test/cases/99_adr.sh` | [`DOCTRINE.md`](@/docs/doctrine.md) | no capability; number allocation is unguarded — `0039` is claimed twice across branches today |

</div>


### The runtime

<div class="overflow-x-auto" tabindex="0">

| Concept | Canonical owner | Storage | Schema / type | Discovery | Consumers | Mutation path | Events | Tests | Docs | Drift |
|---|---|---|---|---|---|---|---|---|---|---|
| capability | `src/capability/` | composed at process start | `CanonicalSchema` per capability | `compose_modules!` | every surface | source only | — | `tests/projections.rs` | [`CAPABILITIES.md`](@/docs/capabilities.md) | canonical; the model |
| command | three declarations composed: `cli.rs`, `share/commands.yaml`, the `justfile` | tracked | `command/v1` for the shell half | `commands.graph` | CLI, workflow bridge, completion, docs | source only | — | `test/cases/34_command_fixtures.sh` | [`COMMANDS.md`](@/docs/commands.md) | **the tool half is depth 1: `plan done` is not a node** |
| execution | `src/execution/` | **in-process only** | typed input per capability | registry | `executions.*`, `/events`, Cockpit | `executions.start`, `.cancel` | the stream's typed messages | `tests/` | [`EXECUTIONS.md`](@/docs/executions.md) | **nothing durable; the 15 messages are in no ledger vocabulary** |
| event (durable) | `share/events.yaml` + `mj_ledger_append` | `.ai/local/state/ledger.jsonl` | a registered vocabulary | registry walk | `history`, `continuity.state`, `obligations` | `lib/` only | itself | `test/cases/*` | [`SCHEMAS.md`](@/docs/schemas.md) | **disjoint from the execution stream in both directions** |
| peer / actor | `src/peers.rs`, `execution/model.rs::Actor` | in-memory | typed | — | `peers.*`, Cockpit, `check --overlap` | `peers.announce` | — | `tests/` | AGENTS.md | an announcement belongs to a connection and is lost on reconnect |
| worktree | `src/worktree/` | derived from git; lock under `<git-common-dir>/majordomus/locks/` | typed topology | `git worktree list --porcelain` | `worktree.*`, the pre-commit guard, Cockpit | `worktree create/migrate/repair` | — | `tests/`, `test/cases/*` | [`WORKTREES.md`](@/docs/worktrees.md) | two legacy layouts still recognised, deliberately |
| context (compiled) | `lib/context.sh`, `lib/context_docs.sh` | assembled per call | budgeted assembly | `.ai/**/README.md`, kind `context` | a worker, the briefing | — (read-only) | — | `test/cases/*` | [`CONTEXT.md`](@/docs/context.md) | **the Rust half (`directories.list`) reports contracts, not compiled context**; `DYNAMICITY.md` already marks the provider table a *target* |
| repository / index | `src/repository.rs`, `src/index.rs` | `.ai/manifest.yaml` + the trees it names | `manifest/v1` | manifest, then `sources.yaml` | everything | — | — | `tests/` | [`SCHEMAS.md`](@/docs/schemas.md) | `benchmarks`, `ci`, `providers` and `workspaces` are on disk but in no manifest section, alongside several loose baseline files |
| shared server / lease | `src/lease.rs`, `src/shared.rs` | `.ai/local/state/mcp/server.json` | `LeaseDocument` | — | every attached client | `serve`, `mcp` | — | `test/cases/108` | [`ENTRY.md`](@/docs/entry.md), ADR 0035 | one server per *checkout*, not per repository |

</div>


### Governance and surfaces

<div class="overflow-x-auto" tabindex="0">

| Concept | Canonical owner | Storage | Consumers | Mutation path | Docs | Drift |
|---|---|---|---|---|---|---|
| rule / doctrine | `.ai/repo/rules/{project,vendor}/` | tracked Markdown, identity `id`+`version` | `check`, `finish`, `doctor`, `watch`, the site, one resource each | `lib/rules.sh` (vendor update) | [`DOCTRINE.md`](@/docs/doctrine.md) | the effective set is the vendored baseline plus this repository's own, each `blocking` or `advisory`; every tool-enforced rule is vendored and **no project rule is tool-enforced** — the doctrine, not a defect: a project rule gets a CI gate. `majordomus rules list --json` prints the split |
| policy / profile | `.ai/repo/policy.yaml`, `profiles/*.yaml` | tracked | provider projections, `finish`, context | hand-edited + `update` | `.ai/README.md` | — |
| skill | `.ai/repo/skills/<id>/SKILL.md` | tracked | one resource each, `lib/skills.sh` | `lib/skills.sh` | `.ai/repo/skills/README.md` | no Rust module; only skill→skill edges in `graph.rs` |
| knowledge | `.ai/repo/knowledge/` | tracked | `objects.*` | `lib/knowledge.sh` | — | **compiler is shell-only, no capability** |
| use case | `.ai/repo/use-cases/*.md` | tracked | one resource each, the coverage gate, the site | `lib/usecase.sh` | [`USE_CASES.md`](@/docs/use-cases.md) | `lib/usecase.sh` is a **second writer of `docs/generated/`** |
| artifact | `generate::Target` (15 targets) | `docs/generated/**`, `site/data/**` | site, docs, `artifacts.list` | `majordomus generate` | [`DYNAMICITY.md`](@/docs/dynamicity.md) | **two writers**: `src/generate.rs` and a shell set including `lib/usecase.sh`, `bin/majordomus`, `scripts/derive`, `scripts/generate-site-data` |
| Cockpit | `src/cockpit/` | none | a person | — | [`COCKPIT.md`](@/docs/cockpit.md) | **clean** — one layer reference, a display label at `pages.rs:1787` |
| quality / gates | `.ai/repo/ci/gates.yaml` | tracked | `validate.yml`, `just gate` | hand-edited + `ci-plan --check` | [`CI.md`](@/docs/ci.md) | `ci-plan --check` prints the gate and class tallies; the cases live in `test/cases/` and `apps/majordomus-cli/tests/` |

</div>


## Gaps against the target pipeline

Ranked by what blocks a development surface most.

1. **No mutating development capability exists.** Only the execution and peer planes
   declare a capability of kind `command`. Every issue transition, session lifecycle event,
   checkpoint, handover, evidence attachment and completion decision is unreachable from
   MCP, HTTP and the Cockpit. *Closing it:* a `kind: command` capability per transition,
   ratcheted off `.ai/repo/development-semantics-baseline.txt`.
2. **The command graph does not see the lifecycle's subcommands.** Every tool node is a
   group at the first level; `majordomus plan done <id>` is not a node. So the exposure policy has nothing to
   judge and the completion engine nothing to offer. *Closing it:* declare the shell
   subcommands in `share/commands.yaml`, or make each one a capability and let the graph
   derive it.
3. **Most lifecycle commands are interactive.** The graph withholds them as *"asks the person
   something; a request/response surface would hang"*. Interactivity is not a property a
   surface can work around. *Closing it:* `project.commands-run-non-interactively@1`,
   currently advisory.
4. **Two disjoint event models.** A durable ledger vocabulary and a live stream
   vocabulary, with no bridge between them. A Cockpit cannot show one activity feed. *Closing it:* a mutating development
   capability appends a registered ledger event and publishes the same change to the
   stream; neither tier holds a durable fact the other cannot see.
5. **Completion is decided in two places.** `obligations.closure` reads the contract;
   `lib/finish.sh` decides it. Two implementations of one contract. *Closing it:* the
   decision moves into the `obligations` module and `finish` asks it.
6. **Compiled context has no capability.** `directories.list` reports context *contracts*;
   the compiled, budgeted context a worker actually gets is `lib/context.sh` only, and
   `DYNAMICITY.md` already lists the provider table as an entity with no owner.
7. **Executions are not durable.** By decision, not by omission — but it means the Cockpit
   cannot show what ran before the current process. *Closing it:* the ledger, per §4.
8. **Two writers of `docs/generated/`.** `src/generate.rs` plus a shell set. Existing debt,
   already in `HARDCODING_LEDGER.yaml` territory, and it will bite any new generated
   development artifact.
9. **The CLI is the least complete consumer, not the most.** Fewer than half the built-in
   capabilities have a CLI projection — `objects.get`, `repository.info`, every `plan.*`
   and `graph.*` and `health.report` have none. So a person at a terminal reaches *less* of the canonical
   runtime than an MCP client does, which inverts the usual assumption that the CLI is the
   reference surface and the others catch up. *Closing it:* the exposure policy decides this
   from effect and interactivity; a query withheld from the CLI is either a policy decision
   worth stating or an omission worth fixing, and today nothing says which.
10. **Concepts with a shell owner and no runtime capability at all:** session contexts,
    knowledge, skills, prompts, questions, history, search.
11. **ADR number allocation is unguarded.** `0039` is claimed by two different files on two
    branches today. A development surface that proposes decisions makes this worse.

## Enforcement

The rule is `project.development-semantics-are-canonical@1`, `class: blocking`. Per this
repository's own doctrine — and consistent with every project rule, none of which carries
an `x-majordomus` block — it is enforced by a CI gate rather than a `lib/` validator.

`scripts/development-semantics-check` is the gate, wired as `development-semantics` in the
`structure` job of `.ai/repo/ci/gates.yaml`. It reads tracked files only and needs no build.
It decides two things:

- **Backing.** A public command of `share/commands.yaml` whose `class` mutates and whose
  `writes` names a development object tree, for which `docs/generated/registry.json`
  declares no capability of kind `command` in the matching module.
- **Surface purity.** A reference to `.ai/repo`, `.ai/local` or a `majordomus` executable
  inside `apps/majordomus-cli/src/cockpit/**` or `share/cockpit/**`, excluding a generator's
  own provenance header.

Every finding it reports today is in `.ai/repo/development-semantics-baseline.txt`: the
unbacked mutating commands — `adr`, `checkpoint`, `decision`, `evidence`, `finish`,
`handover`, `init`, `migrate`, `plan`, `question`, `rules`, `session`, `start`, `update`
and `usecase` — and the one Cockpit display label. A finding not in the baseline fails; a
baseline line matching nothing also fails, so the ratchet tightens instead of rotting.

Two things the gate does **not** decide, so that its silence is not read as a pass:

- **Whether a derivation in a surface is a semantic or presentation.** A template that
  pluralises a count is not a second implementation; one that computes readiness is. A gate
  that guessed would be unauditable. A reviewer decides, exactly as
  `majordomus.decision-threshold@1` leaves its own threshold to a reviewer.
- **Who writes a development object.** A write in shell has no reliable syntactic shape,
  and a grep for the object trees would report every reader as a writer. Deciding it would
  take the writers declared as data — `share/commands.yaml`'s `writes` field extended to
  name the writing function, reconciled against `lib/` the way `exit_codes` already is.
  That is an open item, not a check.

## How this was measured

```sh
# the two programs and the split between them
bin/majordomus --help
bin/majordomus-cli --help

# what the registry holds, which capabilities can mutate, and the module/kind breakdown
bin/majordomus-cli capabilities list --format json > caps.json
jq -r '{count, summary}' caps.json
jq -r '.capabilities[] | select(.provenance.source=="builtin")
        | "\(.id)\t\(.kind)\t\((.exposure|keys)|join(","))"' caps.json
jq -r '.capabilities | group_by(.module)
        | map("\(.[0].module)\t\(length)") | .[]' caps.json

# the commands, their origins, every withheld reason, and the depth per origin
bin/majordomus-cli commands graph --format json > cg.json
jq -r '.commands|group_by(.origin)|map("\(.[0].origin)\t\(length)")|.[]' cg.json
jq -r '[.commands[].projections.withheld|select(.)]
        | group_by(.)|map("\(length)\t\(.[0])")|.[]' cg.json
jq -r '.commands|group_by(.origin)
        | map({o:.[0].origin,
               d:(map(.path|length)|group_by(.)|map({d:.[0],n:length}))})|.[]' cg.json

# executions keep nothing across a process; the stream's message vocabulary
bin/majordomus-cli executions list
bin/majordomus-cli executions protocol

# the effective rules, the class split, and which of them the tool enforces
bin/majordomus rules list --json

# the durable ledger event names
grep -oE '^\s+- id: [a-z._]+' share/events.yaml

# the generation targets
bin/majordomus-cli generate --help

# the web surfaces and their mounts
bin/majordomus-cli web list

# who writes what
grep -rln 'project/issues\|project/milestones' lib/ bin/ scripts/
grep -rln 'docs/generated' lib/ bin/ scripts/
grep -rn '\.ai/\|bin/majordomus' apps/majordomus-cli/src/cockpit/ share/cockpit/*.js

# sizes and the test surface
cat lib/*.sh | wc -l ; ls test/cases/*.sh | wc -l
ls apps/majordomus-cli/tests/*.rs | wc -l

# the gate over this document's own rule
scripts/development-semantics-check
```

Counts of anything in this repository go stale; measure rather than trust a number written
here, including these.
{% endraw %}
