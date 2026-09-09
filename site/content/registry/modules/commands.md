+++
title = "Command graph"
description = "Every command this repository offers, from whichever program offers it: the Rust executable, the shell tool that carries the task lifecycle, and the workflows the repository declares for a person to run. Composed from the three declarations that already exist — the clap tree, the shipped command registry and the workflow runner's own dump — never from a list. Each command carries what running it changes, what it needs, where its argument values come from, and every surface that carries it, with the reason when one does not."
weight = 3
slug = "commands"
[extra]
id = "commands"
source = "apps/majordomus-cli/src/capability/builtin/commands.rs"
+++
