+++
title = "fleet.status"
description = "Asks every machine, in parallel and over ssh unless it is this one, what it is and runs: the destination that answered, its platform, the version its launcher runs, every majordomus serve running there, and for a hub its checkout's branch, commit and cleanliness and the version the hub answers at. A machine no destination reaches is reported with every destination's reason. Changes nothing."
weight = 68
slug = "fleet-status"
[extra]
id = "fleet.status"
source = "apps/majordomus-cli/src/capability/builtin/fleet.rs"
+++
