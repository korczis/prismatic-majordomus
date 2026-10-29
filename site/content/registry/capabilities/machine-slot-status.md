+++
title = "machine.slot.status"
description = "The slot directory as it stands: the limits and where they are declared, the free space on the volume measured, every holder with its pid, purpose, age and whether its process still runs (a pid that runs with another start time is a reused pid, and dead), and every waiter with how long it has waited and what it waits on (`machine.slot: 5 of 5 held`) — the progress a waiting session reports. Reads only: a dead holder is reported here and reclaimed by the next acquisition."
weight = 112
slug = "machine-slot-status"
[extra]
id = "machine.slot.status"
source = "apps/majordomus-cli/src/capability/builtin/machine_slot.rs"
+++
