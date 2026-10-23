+++
title = "machine.slot.release"
description = "Give back the slot a process holds, and withdraw it from the queue if it waits; the next waiter is admitted on its next look. Releasing a slot nobody holds answers `released: false` and changes nothing."
weight = 104
slug = "machine-slot-release"
[extra]
id = "machine.slot.release"
source = "apps/majordomus-cli/src/capability/builtin/machine_slot.rs"
+++
