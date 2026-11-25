+++
title = "plan.collisions"
description = "Every collision between this head and the other lines of work, each naming its holder (ADR 0238). `id_reused` (refused): an issue record this head adds since its merge-base with the trunk that another line of history (the trunk, or any local or remote-tracking branch) also added under another title; a record inherited from the trunk, or the same record carried on two refs, is none. `retitled` (warned): a record the base already holds whose title this head changes, the hidden form of a collision after a rebase. `claimed_elsewhere`: an issue this head executes (named by its branch, by the I1405 rule or its slug, or by `issue`) that another open pull request names by its head branch, or by a title naming it among at most three ids, refused unless one of the two declares it supersedes the other; or that a live mesh claim of another session names, refused; or that only another branch names, warned. A head whose change is planning-only executes nothing. A forge observation older than `collisions.max_age` (`.ai/repo/ci/plan.yaml`) is stale, and a stale or unread forge never proves that no other pull request names an issue. With `gate`, the exit code is 10 on a refusal, 12 when the forge could not be read and the gate is `pr`, and 0 otherwise."
weight = 167
slug = "plan-collisions"
[extra]
id = "plan.collisions"
source = "apps/majordomus-cli/src/capability/builtin/plan.rs"
+++
