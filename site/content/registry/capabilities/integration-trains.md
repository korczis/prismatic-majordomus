+++
title = "integration.trains"
description = "Every train of open pull requests — a group joined by confirmed dependencies: a base branch that is another open pull request's head, or a `Stacked on #N` / `Depends on #N` line in the body — in landing order. Per link: number, title, author, head and base branch, head commit, the forge's page and checks, disposition, lane, reasons, next action, required-check state and every check on the head, relation to master with how far behind it is and whether it carries master, the prepared head under refs/prep/<n> with whether it is the head the forge serves (pushed) or not yet (pending), its position and depth, whether its turn has come, what it stands on and what it carries, its queue rank and whether a cycle catches it. Per train: roots, tops, the next link the order allows, the edges (STACKED_ON or DEPENDS_ON, satisfied when the link below landed), and the pull requests beneath it that are no longer open. Beside them the prepared heads no train holds, the tallies and the queue's diagnostics. Read from the recorded observation, the queue `majordomus prs status` prints and the local refs: it fetches, pushes and moves nothing. `observed: false` with the reason when this checkout has recorded no observation."
weight = 79
slug = "integration-trains"
[extra]
id = "integration.trains"
source = "apps/majordomus-cli/src/capability/builtin/integration.rs"
+++
