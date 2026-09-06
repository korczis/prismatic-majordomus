+++
title = "The public command surface is declared, and reconciled against the dispatcher"
description = "There is one file that says what the public commands are and what each one means:"
weight = 77
[extra]
claim_id = "command-surface"
status = "guaranteed"
source = "docs/claims/command-surface.md"
+++
{% raw %}

## What it means

There is one file that says what the public commands are and what each one means:
`share/commands.yaml`. It carries every command's category, its lifecycle stage, whether
it mutates state, whether it needs an active task, what it reads, what it writes, the exit
codes it can produce, and the demonstration that represents it on the website.

It is not the authority on what runs. `bin/majordomus` decides that, and the two are held
to each other rather than derived one from the other: a command dispatched with no entry,
and an entry for a command nothing dispatches, are both failures.

## How it works

`test/cases/30_command_registry.sh` reads the dispatch table out of the binary and the
surface out of the registry, and compares them in both directions. It also checks what a
registry alone cannot: that ids are unique, that `class`, `visibility` and `requires_task`
stay inside their vocabularies, that every `stage` a command names is one the registry
declares, and that every `MJ_EX_*` constant a command's module references appears in that
command's declared exit codes.

"Internal" is defined rather than asserted. A command is public exactly when the usage text
lists it, and that is checked in both directions, so classifying a command internal cannot
become a way to keep it out of the help text while escaping the obligations a public
command carries. Only `help` is internal today.

## How to see it

```bash
bash test/run.sh 30_command_registry
sed -n '/^commands:/,$p' share/commands.yaml | grep '^  - id:'
grep -oE '^  [a-z|]+\)$' bin/majordomus | tr -d ' )' | tr '|' '\n' | sort
```

## What it does not cover

The registry describes the surface; it does not generate it. Dispatch is still a `case` in
`bin/majordomus`, and adding a command means editing both files — the reconciliation makes
forgetting one of them a failure, not an impossibility. The prose fields (`summary`,
`note`) are editorial and nothing checks that they are true.

## Why it exists

The surface existed in four places that could not be compared: the dispatch table, the
usage text, the `##` headings in `docs/CLI.md`, and the site generator's parse of the help
message. Reconciling them found two defects on the first run. `checkpoint` was documented as
unable to exit `10` when its module plainly can, and `majordomus version --bogus` printed
the version and exited `0` while every other command refuses an unknown option with `2`,
because `version` is dispatched ahead of the option parser and nothing had ever compared it
to the rest of the surface.
{% endraw %}
