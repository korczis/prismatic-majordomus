+++
title = "plan.delivery"
description = "For every issue (or one), the furthest delivery stage proved — none, branch, pr_open, merged, released, deployed — each with the object that proved it and when it was observed: the branch that names the issue (by id, the I1405 rule, or by its slug), the open pull request in the recorded forge observation, the newest commit in the trunk's history that names it (a merge by its pull-request title or branch, any other commit by its subject, never by its body), the first release record (`.ai/repo/releases/v*.yaml`) whose commit contains that merge, and the published `latest.json` that names that release or a later one. Beside it, the issue's declared status, so a contradiction between the stamps and the work is one comparison away. Live by default: the trunk this clone knows, every branch, the forge observation `majordomus prs refresh` recorded, the release records, and the `latest.json` of the commit the last Pages deployment published. With `at`, the merges of that commit's history and the tree's release records and nothing live — every branch, forge and deployment source is reported unread — so only merged and released can be proved and the answer is a function of the commit alone. A source that was not read is reported unread with its reason, never as none: an issue it could have moved further is not settled and names it. Inferred on every call and stored nowhere; it never writes a stamp."
weight = 149
slug = "plan-delivery"
[extra]
id = "plan.delivery"
source = "apps/majordomus-cli/src/capability/builtin/plan.rs"
+++
