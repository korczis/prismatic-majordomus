+++
title = "machine.slot.acquire"
description = "Take one of the machine's slots for a running process, named by its pid, with a purpose a person reads. Holders whose process is proven gone — the pid no longer runs, or runs with another start time — are reclaimed first and named. Below the declared free disk it refuses at once; with every slot held it waits in arrival order, listed as a waiter, up to the bound (the input's, else the policy's), and then refuses. A refusal is an answer, not an error: it names the limit, the measured value, every holder with its pid, purpose and age, and what to do instead. A process that already holds a slot is told so and holds one, not two."
weight = 103
slug = "machine-slot-acquire"
[extra]
id = "machine.slot.acquire"
source = "apps/majordomus-cli/src/capability/builtin/machine.rs"
+++
