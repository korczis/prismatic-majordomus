+++
title = "gates.completion"
description = "The active task's own change set — from the commit it started at to the working tree, uncommitted files included — put through the CI model: which gates it selects and why, what each one last reported and over which files, which verdicts have gone stale because the tree moved underneath them, which required gates have never reported at all, and which obligations the change implies whether or not the task declared them. `finishable` is false only when a required gate is known to be failing, stale or blocked by one that is; absence of a verdict is reported as unverified rather than silently accepted. The done invariant is the questions of share/completion.yaml, in its order, each answered from the source it names; `stage` is the stage of the work those answers fold to, and `complete` is true only when every question passes or is exempt — finishable and complete are two different words."
weight = 74
slug = "gates-completion"
[extra]
id = "gates.completion"
source = "apps/majordomus-cli/src/capability/builtin/gates.rs"
+++
