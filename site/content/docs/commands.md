+++
title = "The command graph"
description = "the command graph: the three declarations it composes, stable identities, the effect model and the exposure policy derived from it, the generated workflow bridge and its compatibility aliases, the one completion engine every surface asks, repository entry, and what adding a command costs (nothing)"
weight = 30
[extra]
source = "docs/COMMANDS.md"
+++

{% raw %}

Every command this repository offers, from whichever program offers it, composed once and
projected everywhere. This document is the contract: what a command is, where the facts
about it come from, which surface carries it and why, and what you have to do to add one.

The short version of the last question is: **define it once, in the program that
implements it. Nothing else.**

## The problem it solves

Three programs answer to a person in this repository:

<div class="overflow-x-auto" tabindex="0">

| Program | What it is | Where its commands are declared |
|---|---|---|
| the Rust executable | `apps/majordomus-cli` — the read-only interfaces, generation, introspection, the servers | the clap tree in `apps/majordomus-cli/src/cli.rs` |
| the shell tool | `bin/majordomus` — the task lifecycle | `share/commands.yaml`, reconciled against the tool's dispatch table |
| the workflow runner | `just` — what the repository declares for a person to run | the `justfile` and `.just/*.just` |

</div>


Before the graph, each surface that wanted to *show* those commands kept its own list. The
`justfile` carried a recipe per command with a description copied from the command's help.
The website parsed a help message. A shell completion, had anyone written one, would have
carried a fourth copy. Every one of those lists was correct on the day it was written.

The graph replaces the lists with one composition and a set of projections.

```text
   clap tree            share/commands.yaml         just --dump
   (the executable)     (the shell tool)            (the workflows)
         │                     │                          │
         └──────────┬──────────┴──────────┬───────────────┘
                    ▼                     ▼
              contributors          capability registry
                    └──────────┬──────────┘
                               ▼
                        CommandGraph  ── fingerprint
                               │
    ┌──────────┬───────────────┼───────────────┬──────────────┐
    ▼          ▼               ▼               ▼              ▼
  bridge   completion      MCP · HTTP       Cockpit         docs
```

Nothing registers with the graph. Every node is *read* from a declaration that already
existed and is already authority for what it states.

## Identity

A command's identity is `<program>.<path joined by dots>`:

```text
executable.worktree.status      majordomus worktree status
tool.check                      majordomus check          (the shell tool)
workflow.site-build             just site-build
```

The program is part of the identity because the three are genuinely different programs, two
of which share a name on the path. The identity is never a display string: it survives a
reworded summary, a renamed group and a moved recipe, which is what every projection keys
on and what `commands explain` resolves.

## What a command carries

<div class="overflow-x-auto" tabindex="0">

| Fact | Where it comes from |
|---|---|
| path, arguments, defaults, value sets, help | the declaration — clap, the command registry, the runner's dump |
| effect: what running it changes | the capability behind it, or `command_graph/semantics.rs` beside the declaration |
| interactivity: terminal, long-running | the same |
| requirements: repository, layer, task, toolchain | the same |
| where an argument's values come from | inferred from the declaration first; annotated only for an identifier a registry here knows |
| aliases | clap's own `alias`, and the workflow aliases beside the command line |
| capability, schemas, MCP tool, HTTP route | the capability registry, joined through the command's `cli` exposure |
| every projection | derived by `command_graph/policy.rs` — never declared |

</div>


### Effects

```text
read-only              reads and answers; changes nothing anywhere
local mutation         writes what no commit carries: process memory, .ai/local/, a build directory
repository mutation    writes tracked files: generated artifacts, the worktree, git
network mutation       reaches the network with an effect on the far side
destructive            removes something a person would have to reconstruct
```

The order is the order of increasing consequence, and the exposure policy reads it.

### Which surface carries what

<div class="overflow-x-auto" tabindex="0">

| effect / interactivity | command line | workflow bridge | MCP · HTTP | Cockpit |
|---|---|---|---|---|
| read-only, non-interactive | yes | yes | yes, with a capability behind it | yes |
| local mutation | yes | yes | yes, with a capability behind it | yes |
| repository / network mutation | yes | yes | no | no |
| destructive | yes | yes, asking first | no | no |
| interactive | yes | yes | no | no |
| long-running | yes | yes | no | no |

</div>


Nothing configures a surface per command. `majordomus commands explain <id>` prints the
reason a surface withholds a command, and the reason is written once, in the policy.

## The workflow bridge

`just` is an ergonomic front door, not a place where commands are declared. Every recipe of
the bridge is one line that runs the canonical program with the caller's own arguments:

```just
# Where this call is — branch, worktree, canonical or not, uncommitted work — and how many errors the whole topology carries; exit 10 when this worktree is out of place
[group('worktree')]
worktree-status *args:
    @"{{majordomus_executable}}" worktree status "$@"
```

Three properties are worth naming.

**It cannot be a cycle.** A recipe runs a program, never another recipe, and a recipe the
bridge itself wrote is never read back as a declaration — the manifest beside the bridge
records exactly which names it wrote.

**Arguments are forwarded, not interpolated.** The root justfile sets `positional-arguments`
and every body forwards `"$@"`. A path with a space, a quoted string, `$(…)`, a backtick and
a leading dash all arrive as the single argument they were typed as. Interpolating
`{{args}}` would splice them into a shell command, and no amount of escaping in a generator
makes that safe.

**It is not tracked.** It lives under `.ai/local/cache/command-graph/`, which no commit
carries, because it is a pure function of the tree it sits in. Entering the repository
refreshes it when one of the declarations behind it has changed, and does a few `stat` calls
when none has.

### Names

The recipe name is the command path joined by `-`: `majordomus worktree status` becomes
`just worktree-status`. Two rules resolve the collisions that algorithm can produce.

- **A declared workflow owns its name.** A bridge that would take it is withheld, with an
  error naming both. `just bench` is the criterion microbenchmarks, declared in
  `.just/test.just`, and it keeps that name.
- **Two programs, one derived name: both are qualified.** `majordomus bench` exists in both
  programs, so neither keeps `bench`: they are `executable-bench` and `tool-bench`, and the
  graph says so.

### The older names

Every recipe name that predates the bridge still works. They are declared once, in
`WORKFLOW_ALIASES` beside the command line, and rendered into the bridge under the
`compatibility` group with the reason each one exists. An alias that names a command the
graph no longer carries fails the build rather than pointing at nothing.

## Completion

There is one completion engine, and the shell adapters know nothing.

```text
   zsh / bash
        │  words, cursor
        ▼
   generic adapter ──► majordomus completion query ──► CompletionEngine
                                                            │
                                              CommandGraph  +  value sources
```

An adapter reads the words being completed, finds the cursor, asks the executable and prints
what comes back. It carries no command, no flag and no identifier — a test asserts exactly
that — so a command added tomorrow is completed by an adapter loaded a year ago.

`just <TAB>` and `majordomus <TAB>` are the same query with a different surface: the recipe
name is resolved back to the canonical command and the *same* argument metadata answers what
follows. There is no second completion implementation to disagree.

Install it once, for every repository:

```sh
majordomus completion install --shell zsh     # or --shell bash, --shell fish
```

That writes the integration into the shell's startup file between `# >>> MAJORDOMUS >>>`
markers. Nothing outside them is read or rewritten, running it again changes nothing, and
`--remove` takes it out and leaves the file as it was; `--dry-run` says what would change
and writes nothing. The first write keeps a `.majordomus.bak` beside the original.

It is its own command, and nothing else calls it. `majordomus init` initialises a
*repository's* `.ai/` layer, and writing into a person's home directory as a side effect of
that would be a second, unasked-for act. Installing a shell integration is a decision, so it
is a command.

The line it writes is the one you would have written by hand:

```sh
eval "$(majordomus completion init --shell zsh)"
```

`majordomus`, not a path into a build directory: the integration is generic and outlives any
one checkout. Inside a repository, `.envrc` exports `MAJORDOMUS_COMPLETION_BIN` and the
adapter asks *that* executable instead — which is how the completion works in this
repository, where the name `majordomus` on the path is the shell tool and has no
`completion query` of its own.

### What it will not do

- **No network.** A value that can only be learned remotely is not offered.
- **No secret.** An argument classified as a secret is never enumerated, never cached and
  never logged. The graph refuses to build if one has a value source that would enumerate it.
- **No dependence on a server.** A running server changes nothing; nothing here starts one.
- **No index.** The completion never builds the repository index — the index takes seconds
  and answers a question about `.ai/`, which is not a question about what commands exist.

### Where values come from

Inference first: a value-enum argument carries its own values, a `PATH` placeholder is a
path, a `BRANCH` placeholder is a branch. Only an identifier whose set lives in a registry
here is annotated, in `VALUE_BINDINGS` beside the command line. A command author who gives an
argument a typed value has already said everything a completion needs.

## Repository entry

```sh
# .envrc
PATH_add bin
watch_file .ai/local/state/mcp/server.json
eval "$(bin/majordomus-env export --shell direnv --banner --bridge)"
```

One call: the assignments on stdout for `eval`, the banner on stderr, and the bridge
refreshed when a declaration behind it changed. Deciding whether a generated file is current
is the tool's work — a staleness rule implemented in the shell entry point would be a second
implementation running on every `cd` that nothing tests. The rule
`project.envrc-is-an-adapter` holds that shut.

Entering the repository never builds anything and never reaches the network. A checkout that
has not built the executable yet prints one line naming the recipe that builds it and exits 0.

## Adding a command

### To the Rust executable

1. Add the clap arm in `apps/majordomus-cli/src/cli.rs` and its implementation under
   `src/commands/`.
2. Add its example to `EXAMPLES` beside the declaration — the crate's own tests run it.
3. If it is anything other than read-only and non-interactive, add one entry to
   `SEMANTICS` in `src/command_graph/semantics.rs`.

Then: the command line, `--help`, the workflow bridge, the shell completion of both surfaces,
`majordomus commands`, `/api/v1/commands`, the MCP tools that serve the graph, the Cockpit and
the generated reference all carry it. **No other file is edited.**

### To the shell tool

Add the entry to `share/commands.yaml` and the arm to the dispatch table in `bin/majordomus`.
The `command_surface_complete` doctrine reconciles the two; the graph reads the registry.

### As a workflow

Write the recipe in `.just/<context>.just`. The runner reports it, the graph reads it, and
the bridge does not touch it.

## Pages

A command's page is not a new page tree. Each program's commands already have a reference
here, generated from the same declarations the graph reads, and the graph's only decision is
which one a command points at:

<div class="overflow-x-auto" tabindex="0">

| program | route |
|---|---|
| the Rust executable | `/docs/cli/<path>/` — one page per command, from the clap tree |
| the shell tool | `/commands/<id>/` — one page per command, from `share/commands.yaml` |
| a workflow | `/docs/commands/` — this page; a recipe has no reference of its own |

</div>


The graph itself is deliberately **not** a committed artifact. It carries the workflows the
runner reports, which depend on what is installed on the machine that built it; committing
one would be committing a machine's state and calling it a projection. It is served instead —
over HTTP, over MCP and from the command line — and the Cockpit renders it at
`/cockpit/commands`.

## Reading the graph

```sh
majordomus commands                          # every command, one line each
majordomus commands list --effect read-only  # only what changes nothing
majordomus commands show executable.serve    # one command, and every surface that carries it
majordomus commands explain executable.serve # and why each surface does or does not
majordomus commands graph --format json      # the whole document, fingerprinted
majordomus commands bridge                   # materialise the workflow bridge
majordomus commands bridge --check           # exit 10 when it is stale
```

The same graph over the other surfaces:

```text
GET /api/v1/commands           the index, filtered
GET /api/v1/command?id=…       one command in full
GET /api/v1/commands/graph     the whole document with its diagnostics
majordomus_commands            the MCP tool over the first of those
```

## Diagnostics

The build collects findings, and a projection refuses to write while any of them is an
error. The codes are stable, so a gate can match on them.

<div class="overflow-x-auto" tabindex="0">

| code | what it means |
|---|---|
| `duplicate-command-id` | two commands claim one identity |
| `duplicate-workflow-projection` | one recipe name would be written by two commands |
| `duplicate-mcp-projection` | one MCP tool name is claimed by two commands |
| `workflow-name-collision` | a declared workflow owns a name the bridge wanted |
| `workflow-name-qualified` | two programs' commands derived one name; both were qualified |
| `capability-names-no-command` | a capability claims a command line that does not exist |
| `semantics-names-no-command` | an annotation names a command that no longer exists |
| `alias-names-no-command` | a compatibility alias points at nothing |
| `execution-cycle` | a command projected as a recipe is executed through the runner |
| `secret-is-suggestible` | a secret argument has a value source that would enumerate it |

</div>


The first time the join ran it found a real one: a capability declared that it was reached as
`majordomus scope classify`, and the command line had no such command. That is a documented
command that did not exist, and nothing else in the repository could have noticed.

## The cache

`.ai/local/cache/command-graph/` holds the bridge, the manifest of what was written, and the
graph itself. All three are written atomically — rendered beside the destination, then
renamed — so two shells entering the repository at once cannot read a half-written file, and
neither needs a lock.

Two keys, for two questions:

- the **stamp**, over the size and modification time of the declarations and of the
  executable, answers "has anything changed?" in a few `stat` calls, on the hot path;
- the **fingerprint**, over the graph's own semantic content, answers "is this the same
  graph?" and is what a generated file, a client's cache and an integrity check key on.

Nothing time-dependent is in either, so two builds over one tree agree.

## Bootstrap

A fresh clone has no bridge and no executable.

```text
clone
  └─ direnv, or `just`
       └─ the executable is absent: one line naming `just build`, exit 0
            └─ just build
                 └─ just bridge   (or the next `cd`)
                      └─ every command of both programs, as a recipe
```

The root justfile imports the bridge optionally, so a clone without one still has a
justfile: `just build` and `just bridge` are the two recipes the bridge cannot provide,
because they are what produce it.

## What proves it

<div class="overflow-x-auto" tabindex="0">

| | |
|---|---|
| `scripts/ci/command-graph` | the gate: the graph builds without an error, and no workflow file carries a hand-written bridge |
| `test/cases/100_environment.sh` | the shell entry point is an adapter, and the check can fail |
| `test/cases/101_command_graph.sh` | the graph, the bridge, the completion, the secret sentinels, one repository at a time |
| `test/cases/102_completion_shell.sh` | the generated adapter, loaded into a real zsh, offering real candidates |
| `apps/majordomus-cli/benches/commands.rs` | the cost of composing, projecting and completing, measured rather than claimed |
| `apps/majordomus-cli/src/command_graph/` | the unit half: no adapter names a command, no mutation reaches a machine surface, no workflow is bridged back into a workflow, the fingerprint is stable |

</div>


Measured on this machine, macOS arm64. Two scales, because they answer different questions.

**In process, release build** (`cargo bench --bench commands`) — what the engine costs:

```text
compose the graph, command line only        1.16 ms
compose it with the capability join         1.18 ms
compose it with the shell tool's registry   1.76 ms
render the whole workflow bridge              71 µs
answer one completion                  114 ns – 4.4 µs
```

**End to end, debug build** — what a person waits for:

```text
majordomus --help                      8.5 ms
completion query (any shape)          14-16 ms
commands bridge, nothing changed       8.8 ms
commands graph, in full                128 ms   (two `just` subprocesses)
repository entry, warm                 113 ms
the bridge's overhead over a direct call 33 ms
```

The gap between the two is the point. A completion answers in microseconds and arrives in
fourteen milliseconds, so essentially all of it is process start and finding the repository —
which is why the fast load builds no index and spawns nothing, and why making the engine
cleverer would buy nothing at all.

## Related

- [`CAPABILITIES.md`](@/docs/capabilities.md) — the capability registry the graph joins against
- [`ENVIRONMENT.md`](@/docs/environment.md) — the typed snapshot repository entry renders
- [`DYNAMICITY.md`](@/docs/dynamicity.md) — the ownership rule this is an instance of
- [`CLI.md`](@/docs/cli-specification.md) — the shell tool's own command specification
{% endraw %}
