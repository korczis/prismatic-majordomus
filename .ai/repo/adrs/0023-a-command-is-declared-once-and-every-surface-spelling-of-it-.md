---
schema: adr/v1
id: adr-0023
kind: adr
title: A command is declared once and every surface spelling of it is computed
status: proposed
date: 2026-09-08
provenance:
  origin: authored
---

# 23. A command is declared once and every surface spelling of it is computed

## Context

`project.interfaces-are-projections` settled the question for capabilities in ADR 2 and
ADR 4: a capability is one typed descriptor, and MCP, HTTP, OpenAPI, the Cockpit and the
generated reference are derived from it. Commands were the exception that had gone
unnoticed, because they looked like they were already declared once.

They were not. clap declared the command line; the justfile then declared the same
commands a second time, in its own names, with its own descriptions and its own grouping —
seventy recipes, thirteen of them a second spelling of something the executable already
knew. The two descriptions had already begun to differ. Shell completion, meanwhile, had no
declaration at all and therefore did not exist: a repository whose whole argument is that
one definition should reach every surface offered its own operators nothing when they
pressed TAB.

The obvious repair — write a completion script, and keep the justfile in step by hand — is
the wrong one twice. A generated static completion script is a third catalogue of commands,
stale from the moment a command is added; and a justfile kept in step by hand is the second
catalogue that produced the drift in the first place.

One fact stood in the way of deriving everything: what running a command *changes* cannot
be read off any parser. `worktree list` and `worktree remove` are the same shape to clap.
Every surface decision that matters — whether a machine may call it, whether to ask before
running it, whether to expose it at all — depends on exactly that fact and on nothing else.

## Decision

A command of the Rust executable is declared once, in `apps/majordomus-cli/src/cli.rs`: its
path, arguments, accepted values and help in the clap declaration, and beside it, in the
same `EXAMPLES` entry that already carries its examples, the `Semantics` of running it —
its effect class and how it occupies the caller. `Semantics` is a mandatory field rather
than a lookup table, so a command cannot be added without being classified.

`apps/majordomus-cli/src/control/` composes the canonical command graph from three sources
that already own their facts: the clap declaration walked by `cli::tree()`, the capability
registry for identity and declared HTTP and MCP exposure, and `just`'s own structured dump
for workflows the repository keeps outside the executable. It adds no store of its own.
Every surface spelling — the `just` recipe name, the MCP tool name, the Cockpit action, the
documentation route, the compatibility aliases — is computed by an algorithm with collision
detection, and a declared exposure always wins over a computed one.

Exposure is derived from the classification alone. A read or a local change is offered to
every surface; a command that writes tracked files or destroys work is offered on the
command line and in the `just` bridge and to no machine surface, with the refusal stated in
`majordomus commands explain`. Only what cannot be undone asks for confirmation.

Completion is a runtime query, not a generated catalogue. One engine answers every surface:
a `just` recipe resolves to the node the command line walks to, so the two cannot drift. The
shell adapters under `share/completion/` carry no command, flag or value; they report the
words and the cursor and render the answer, and are installed once.

The `just` bridge is materialised into `.majordomus/runtime/just/bridge.just`, which is
ignored by git and imported optionally, so a fresh clone still lists its recipes and
entering a repository never dirties the tree.

## Alternatives rejected

**A generated completion script per shell.** It is a catalogue of commands living outside
the declaration, stale the moment anything changes, and it cannot offer a value that
depends on the repository — a branch, a capability id — because a static file cannot know
one. Rejected for the same reason a hand-written OpenAPI document was rejected in ADR 4.

**Deriving the effect from the command's name.** `remove`, `create`, `write` and `update`
are suggestive and not reliable, and the failure mode is silent: a destructive command that
reads as a query is offered to a machine. A guess in that direction cannot be taken back,
so the classification is declared and the compiler requires it.

**Making `just` the canonical source and having Majordomus read it.** It inverts the
dependency — the presentation surface would define the operation — and creates the cycle
this architecture forbids: `just` calling Majordomus calling `just`. `just` remains the
ergonomic surface and the home of genuinely external workflows, which are discovered from
its own dump rather than listed anywhere.

**Keeping the bridge as a tracked, drift-checked artifact.** It is a pure function of the
declaration and of the executable's version, both already in the tree, so committing it
would commit a derivation of what sits beside it — and would make `cd` into the repository
a source of dirty diffs. Runtime materialisation with a byte comparison costs nothing when
nothing changed.

## Consequences

Adding a command to the executable adds its `just` recipe, its completion, its MCP tool
where the classification allows one, its Cockpit action and its documentation route, with
no other file edited. `apps/majordomus-cli/tests/control_plane.rs` proves this over a
command line this crate does not ship, which is the only form of that proof that cannot be
faked by having remembered to update a projection.

Three costs are real. A new command must be classified, and a wrong classification is now
load-bearing: it decides exposure everywhere. Two commands that would answer to one
spelling on one surface now fail the graph instead of one quietly winning — which is
correct, and which broke two existing pairs (`distribution`/`distribution show` and
`why`/`why list`) on the first run. And a fresh clone must run `just bridge` once before
the bridged recipes exist, which is why the import is optional and the justfile keeps its
own bootstrap.

The justfile shrank by the thirteen recipes that were a second declaration, and `bench` had
to be given up by the criterion microbenchmarks — two different things had held one name,
and the graph is what made that visible.

`docs/COMMANDS.md` is the reference; `project.commands-are-projections` is the enforceable
rule; `test/cases/92_command_control_plane.sh` is the behavioural proof.
