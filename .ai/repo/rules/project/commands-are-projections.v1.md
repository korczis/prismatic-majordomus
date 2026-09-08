---
id: project.commands-are-projections
version: 1
kind: rule
title: A command is declared once, and every surface that shows it is a projection
description: Every command this repository offers is declared in the program that implements it and nowhere else; the workflow runner, the shell completion, the machine surfaces, the Cockpit and the documentation are projections of one composed graph, so adding a command requires no second registration and no surface can carry a command catalogue that goes stale.
statement: A command is declared once, in the program that implements it; every other surface derives its name, its description, its arguments and its availability from the composed command graph, and none keeps a catalogue of its own.
status: active
class: blocking
depends_on: [project.interfaces-are-projections@1, project.hot-path-reads-once@1]
tags: [commands, projections, completion, workflows]
---

# Rationale

Three programs answer to a person here, and every surface that wants to *show* what they
offer is tempted to keep a list. The temptation is reasonable each time: a `just` recipe per
command is four lines and reads well; a completion script with the command names in it works
immediately; a documentation page with a table of commands is easy to write.

Every one of those lists is correct on the day it is written. What happens afterwards is not
a hypothesis — this repository has the evidence. A capability declared that it was reached as
`majordomus scope classify`; the command line had no such command, and had never had one.
Nothing noticed, because nothing joined the two declarations. The generated reference showed
a command that could not be run.

The general shape is the one `DYNAMICITY.md` names: a fact written in two places, one of
which changes. A command's name, its one-line description, its arguments and the values they
accept are facts the implementing program already knows, and every copy of them decays
silently — a recipe that outlives the command it spells, a completion that offers a flag that
was removed, a page that documents a command that never existed.

The cost of the alternative is small and one-directional. The graph composes three
declarations that already exist and are already authority: the clap tree, the shipped command
registry the shell tool is reconciled against, and the workflow runner's own structured dump.
Nothing registers with it. Adding a command to any of those puts it on every surface the
exposure policy admits, and removing one takes it off.

# Required behaviour

**A command is declared in one place: the program that implements it.**

- the Rust executable's commands are the clap declaration in `apps/majordomus-cli/src/cli.rs`;
- the shell tool's commands are `share/commands.yaml`, reconciled against its dispatch table;
- a workflow is a recipe in `justfile` or `.just/*.just`.

**Semantics that a declaration cannot carry are annotated once, beside it.** What running a
command changes, whether it holds a terminal, what it requires, and where an argument's values
come from live in `apps/majordomus-cli/src/command_graph/semantics.rs`, next to the command
line they describe. The default is what most commands are, so only a departure is written
down, and an annotation that names a command which no longer exists fails the build.

**Every other surface derives.** Specifically, none of these may carry a command's name,
description or arguments:

- a workflow file may not declare a recipe whose body is a call to either program — that is a
  hand-written bridge, and the generated one already carries it, with the description its own
  declaration gives;
- a shell completion adapter may not contain a command, a flag, an identifier or a value set;
- an MCP tool list, an HTTP route table, an OpenAPI document, a Cockpit palette and a
  documentation index may not be hand-maintained for a command that the graph carries;
- a compatibility alias for an older recipe name is declared once, beside the command line,
  and rendered into the generated bridge — never maintained in a workflow file.

**Projections are derived, not declared.** Which surface carries a command follows from what
running it changes and how it behaves towards a terminal, decided in one function
(`command_graph/policy.rs`). No command configures a surface, and every surface that withholds
a command carries the reason.

**Generated projections are not tracked.** The workflow bridge is a pure function of the tree
it sits in; it lives under `.ai/local/cache/`, it is written atomically, and it is refreshed
when a declaration behind it has changed. Entering the repository must not dirty the working
tree.

# Failure behaviour

The gate `command-graph` (`scripts/ci/command-graph`) fails, exit 10, when:

- the graph carries an error — a duplicate identity, a recipe name two commands would take, a
  capability that claims a command line the executable does not declare, an annotation or an
  alias that names a command that no longer exists, or a secret argument with a value source
  that would enumerate it;
- a workflow file declares a recipe whose body calls one of the two programs directly, with
  the two bootstrap recipes that produce the bridge named as the exception.

`test/cases/101_command_graph.sh` is the behavioural half: it proves the graph builds in a
disposable repository, that the bridge it writes forwards arguments rather than interpolating
them, that a second materialisation writes nothing, that adding a command to the declaration
reaches the workflow projection and the completion without another edit, and that the check
can fail — a hand-written bridge introduced into a copy is rejected by the same gate.

There is no runtime enforcement of the derivation itself. The projections *are* the
derivation: there is nothing to check at run time, because there is no second copy that could
disagree.

# Verification

```sh
scripts/ci/command-graph                       # the gate
bash test/run.sh 101_command_graph             # the behavioural case
majordomus commands graph --check              # the graph alone; exit 10 on an error
majordomus commands explain <id>               # where one command is projected, and why
```

`apps/majordomus-cli/src/command_graph/` carries the unit half: that no adapter names a
command, that a mutation never reaches a machine surface, that a workflow is never bridged
back into a workflow, and that the fingerprint of a graph over one tree is stable.
