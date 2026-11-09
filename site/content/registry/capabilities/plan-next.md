+++
title = "plan.next"
description = "The lowest-wave READY issue of the active milestone, highest priority first, then the number of unfinished issues it unblocks, then id, whose work the trunk does not already hold. The active milestone can have nothing ready while another one does — one waiting on its own acceptance evidence, for instance — so the search widens to the whole plan rather than answering `none` and sending a worker away from work that is genuinely executable. Each candidate in that order is set against `plan.delivery` at the trunk head, read from the trunk's history and the release records only (no branch, forge or deployment, so the answer is a function of that commit): one the history shows merged, released or deployed is passed over and listed in `passed_over` with its stage, its proving commit and the remedy (record it with plan start, verify and done); a branch or an open pull request passes nothing over. `delivery` names the trunk ref and commit read, or says delivery was unread and the answer is the stamp ranking's. Cached per checkout state, so a merge that moves the trunk changes the answer without a restart. This is what an agent asks before it starts."
weight = 170
slug = "plan-next"
[extra]
id = "plan.next"
source = "apps/majordomus-cli/src/capability/builtin/plan.rs"
+++
