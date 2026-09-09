+++
title = "plan.issues"
description = "One record per issue with its derived status, its wave, the dependencies it declares, the ones that are not DONE (plus `milestone:<id>` when the gate holds the whole outcome back), the issues that depend on it, the paths it touches and its evidence tally. Filtering by `status: READY` is the ready set and by `status: BLOCKED` the blocked set; nothing here is a separate derivation."
weight = 39
slug = "plan-issues"
[extra]
id = "plan.issues"
source = "apps/majordomus-cli/src/capability/builtin/plan.rs"
+++
