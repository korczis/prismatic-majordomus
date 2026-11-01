+++
title = "majordomus proceed step"
description = "One step, as one work order, read only: `do` (the capability to run next and its input), `fan_out` (what may run beside it), `report` (what to announce) and `next_due_at` (when to step again); for an intent, `done` once its realisation is met. The same state answers the same order and key. Exit 0 when work is ordered or the order is done, 10 when the step waits"
weight = 240
[extra]
route = "/docs/cli/proceed/step/"
command = "majordomus proceed step"
source = "apps/majordomus-cli/src/cli.rs"
+++
