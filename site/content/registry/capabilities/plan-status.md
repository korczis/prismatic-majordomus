+++
title = "plan.status"
description = "Every milestone with its derived status and its issues counted by status, the milestone a worker is executing now, the next ready issue in full, and the plan's own totals. The next ready issue is `plan.next`'s, chosen by the same function: the stamp ranking, then delivery at the trunk head, passing over an issue the trunk's history shows merged, released or deployed. The counts are keyed by the declared vocabulary, which travels with the answer, so a status added to the engine appears here without anything being edited. Cached per checkout state, so a merge that moves the trunk changes the answer without a restart."
weight = 173
slug = "plan-status"
[extra]
id = "plan.status"
source = "apps/majordomus-cli/src/capability/builtin/plan.rs"
+++
