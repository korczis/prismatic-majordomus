+++
title = "Commands"
description = "the canonical command graph: what is declared once, how the `just` bridge, shell completion, MCP and the documentation are computed from it, how exposure follows the effect classification, and how to add a command or a workflow"
weight = 28
[extra]
source = "docs/COMMANDS.md"
+++

{% raw %}

Everything this repository can be asked to run is one graph, and every surface that spells
a command reads it.

`docs/CAPABILITIES.md` describes the same architecture one level down, for *capabilities* —
the things the executable can be asked to answer. This document is about *commands* — the
things a person runs — and about the surfaces they reach: the command line, the `just`
bridge, shell completion, MCP, the Cockpit and the generated reference.

## What is canonical, and where it lives

<div class="overflow-x-auto" tabindex="0">

| fact | canonical source |
|---|---|
| command path, arguments, defaults, accepted values | the clap declaration in `apps/majordomus-cli/src/cli.rs` |
| one-line and long description | the same declaration |
| examples | `cli::EXAMPLES`, beside the declaration; executed by `tests/cli_examples.rs` |
| what running it changes | `Semantics` in the same `EXAMPLES` entry |
| compatibility names | `aliases` in the same entry |
| identity, HTTP route, MCP tool of a capability | the capability registry |
| workflows outside the executable | `just --dump --dump-format json` |
| `just` recipe name, MCP tool name, Cockpit action, docs route | computed by `src/control/projection.rs` |

</div>


Nothing else holds any of these. A surface that kept its own copy would be the defect
`project.commands-are-projections` names.

## The one thing that must be declared

Everything about a command can be read off the declaration except what running it changes:
`worktree list` and `worktree remove` are the same shape to a parser. That is declared once,
beside the command's examples:

```rust
CommandExamples {
    command: "worktree remove",
    aliases: &[],
    semantics: Semantics::of(EffectClass::Destructive),
    examples: &[/* ... */],
},
```

`Semantics` is a field, not a table, so the compiler asks for it. The effect classes are
`ReadOnly`, `LocalMutation`, `RepositoryMutation` and `Destructive`; the interactivity is
`Batch`, `LongRunning` or `Interactive`.

Everything a surface needs follows from it:

<div class="overflow-x-auto" tabindex="0">

| classification | command line | `just` | completion | MCP / Cockpit | asks first |
|---|---|---|---|---|---|
| read-only | yes | yes | yes | yes | no |
| local change | yes | yes | yes | yes | no |
| repository change | yes | yes | yes | **no** | no |
| destructive | yes | yes | yes | **no** | **yes** |
| long-running | yes | yes | yes | **no** | no |

</div>


A command that changes the repository is described to a machine and never executed by one.
A command that never returns on its own is not a request/response call whatever it changes.
There is no per-surface override: a command whose exposure looks wrong is classified wrong.

## Adding a command

1. Declare it in `cli.rs`, as clap.
2. Add its `CommandExamples` entry: its `semantics`, any `aliases`, and at least one
   example. `cli::validate` refuses a runnable command without one, and the example tests
   run the argv you wrote against the built executable.
3. Implement it under `src/commands/`.

That is all. `just bridge` re-materialises the recipe; completion offers it immediately; the
documentation route, the MCP tool name where the classification allows one, and the Cockpit
action are computed. No justfile edit, no completion edit, no MCP registration, no route
table, no docs navigation entry.

`apps/majordomus-cli/tests/control_plane.rs` proves it: it composes the graph over a command
line this crate does not ship and asserts that each projection knows about it.

## Adding a workflow that is not the executable

Some steps are scripts and should stay scripts. Write the recipe in the justfile as usual;
`majordomus commands --workflows` discovers it from `just`'s own dump, with its doc comment
as its summary and its `[group()]` as its group. This repository's convention is read as
semantics: a recipe carrying `[confirm]` destroys something. A discovered workflow is never
offered to a machine surface — the executable cannot know what a script does.

## Completion

One engine answers every surface. A shell adapter reports the words and the cursor; it
carries no command, no flag and no value, and is installed once:

```sh
majordomus completion script zsh  > ~/.zsh/completions/_majordomus
majordomus completion script bash > ~/.bash_completion.d/majordomus
```

The engine resolves the surface's spelling to a canonical node first, so
`just worktree-status --format <TAB>` and `majordomus worktree status --format <TAB>` return
the same candidates; `test/cases/92_command_control_plane.sh` asserts that equality.

Values come from the argument's own declaration: a value-enum offers its values, a value
name of `PATH`, `DIR` or `FILE` asks the shell for a path, and the value names `CAPABILITY`,
`KIND`, `MOMENT`, `BRANCH` and `COMMAND` name a registry this repository already holds. No
list is kept for completion anywhere.

What the engine will not do: reach the network, start a server, build anything, or read the
value of an environment variable. Answers that would need the repository's index — the kinds
of the layer, the Why catalogue — are empty rather than slow. A completion that stalls is
worse than one that stays quiet.

## The `just` bridge

`.majordomus/runtime/just/bridge.just` is generated: one recipe per canonical command, its
description the command's own, its group its namespace, a `[confirm]` on anything
destructive, and its arguments forwarded through `set positional-arguments` and `"$@"` so a
space, a quote, a `$` or a UTF-8 word arrives exactly as it was typed.

It is ignored by git and imported optionally, so a fresh clone lists its recipes before the
bridge exists. `just bridge` materialises it — atomically, and only when the bytes would
differ, so entering the repository never dirties the tree and an unchanged graph costs a
read.

A recipe calls the executable. The executable never calls `just`. `control::just` has a test
for that, because the cycle is the failure this design exists to avoid.

## Reading the graph

```sh
majordomus commands                      # every command, its effect, its surfaces
majordomus commands explain worktree.remove
majordomus commands graph                # the whole document, with its fingerprint
majordomus commands projection just      # the bridge, without writing it
majordomus commands materialise          # write it
majordomus commands --workflows          # include what just holds
```

`explain` answers the question that is otherwise answered by grep: where a command is
declared, what running it changes, every surface it appears on — and, for the surfaces it
does not appear on, the reason.

## When it fails

`majordomus commands` exits `10` and names the problem:

- `COMMAND_WITHOUT_SEMANTICS` — a command that can be run does not say what running it
  changes. Add `semantics` to its `EXAMPLES` entry.
- `PROJECTION_COLLISION` — two commands would answer to one spelling on one surface. Both
  are named, with the surface. Nothing is resolved by declaration order.

## Cost

The graph is a walk of the clap declaration and the builtin registry: no repository index,
no subprocess, no file. That is what lets a keystroke pay for it. Discovering the workflows
`just` holds is the one thing that spawns a process, and only when asked for.
{% endraw %}
