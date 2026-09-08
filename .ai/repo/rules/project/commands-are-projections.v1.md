---
id: project.commands-are-projections
version: 1
kind: rule
title: A command is declared once and every surface spelling of it is computed
description: The command line is the one declaration of what a command is; the just bridge, shell completion, the MCP tool name, the Cockpit action and the documentation route are computed from it, and no surface keeps a catalogue of commands or a description of its own.
statement: A command is declared once, with the semantics of running it beside it; every other surface's spelling of that command is derived, and a surface that keeps its own list, description or argument definition is a bug.
status: active
class: blocking
depends_on: [project.interfaces-are-projections@1]
tags: [architecture, command, just, completion]
---

# Rationale

`project.interfaces-are-projections` settled this for capabilities. Commands were the
exception nobody had noticed: clap declared them, and the justfile then declared them a
second time — its own recipe names, its own descriptions, its own grouping — while shell
completion had no declaration at all and therefore did not exist. The descriptions had
already begun to differ, which is what a second declaration always produces given time.

One fact is worth stating plainly, because it is the reason the invariant is affordable:
what running a command *changes* cannot be read off any parser. `worktree list` and
`worktree remove` are the same shape to clap. That single semantic fact is declared beside
the command, and everything a surface needs to decide — whether to offer it, whether to ask
first, whether a machine may call it — follows from it.

# Required behaviour

A command of the Rust executable is declared in `apps/majordomus-cli/src/cli.rs`: its path,
its arguments, its accepted values and its help in the clap declaration, its examples and
its `Semantics` in `EXAMPLES`. `Semantics` is a field rather than a table, so a command
cannot be added without classifying it.

Everything else is computed by `apps/majordomus-cli/src/control/`: the canonical id, the
`just` recipe name, the MCP tool name, the Cockpit action, the documentation route, the
compatibility aliases, and whether the command appears on a surface at all. A capability's
declared HTTP and MCP exposure remains canonical where it exists; the algorithm fills in
only where nothing is declared. Two commands that would answer to one spelling on one
surface is a fatal diagnostic naming both, never a silent winner.

Exposure is derived from the classification and from nothing else: a read is offered to
every surface, a command that changes this machine is offered to every surface, and a
command that writes the repository or destroys work is offered on the command line and in
the bridge and to no machine surface. A surface may not add an exception; a command whose
exposure looks wrong is a command classified wrongly.

The `just` bridge is generated into `.majordomus/runtime/just/bridge.just`, which is
ignored: it is a function of the declaration and of the executable's version, both already
in the tree. It is never edited, and a recipe added to the justfile by hand for something
the executable already declares is a violation of this rule. Shell adapters under
`share/completion/` carry no command, no flag and no value: they report the words and the
cursor and render the answer, so they are installed once and never regenerated.

# Failure behaviour

`majordomus commands` exits `10` when the graph carries a fatal diagnostic — a command that
can be run without declared semantics, or two commands colliding on one surface — and names
the command and the surface in both cases. `majordomus commands explain <id>` answers where
a command is declared and why it does or does not appear on each surface, so the question
is never answered by grep.

# Verification

`bash test/run.sh 92_command_control_plane`, which proves the graph is valid, that nothing
mutating reaches a machine surface, that `just <recipe> <TAB>` and `majordomus <words>
<TAB>` return identical candidates, that every recipe name the justfile used to carry still
resolves, and that arguments survive the bridge unchanged. The Rust half is
`cargo test -p majordomus-cli control::`.
