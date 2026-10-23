+++
title = "plan.drift"
description = "Every contradiction between an issue's declared status and its delivery (plan.delivery), each with the objects that prove it: `merged_not_recorded` — READY, BLOCKED or ACTIVE, with a merge on the trunk naming it (pull request, merge commit, release) and no open pull request naming it; `active_without_work` — ACTIVE since before the window, with no branch, pull request, merge or live mesh claim naming it; `done_without_trace` — DONE with no merge naming it, advisory because record-only issues exist. Each finding carries its age, whether it is past the window `.ai/repo/ci/plan.yaml` declares (`drift.window`, seconds), the sources that were not read and could have dissolved it, and the command that settles it (`majordomus plan verify|done <id>`). With `gate`, judged as a gate: on a `head` every merged_not_recorded is pending; on the `trunk` one merged longer than the window ago is refused and a younger one pending; the other classes never refuse; a missing or unreadable window refuses (exit 12); refusals fail the gate (exit 10) only under `drift.enforcement: blocking`, and are reported with exit 0 under `advisory`. Observed live, or `at` a commit as plan.delivery is. Inferred on every call; it never writes a stamp."
weight = 148
slug = "plan-drift"
[extra]
id = "plan.drift"
source = "apps/majordomus-cli/src/capability/builtin/plan.rs"
+++
