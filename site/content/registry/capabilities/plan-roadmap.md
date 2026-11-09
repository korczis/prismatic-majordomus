+++
title = "plan.roadmap"
description = "The milestone graph laid out by rank, with `order` breaking ties inside a rank only, and the first unblocked unfinished milestone as `now` and the one after it as `next`. Nothing in the sequence is authored: a milestone whose prerequisites are not real cannot be nominated, which is what makes `each step is gated by the previous one being real` an invariant rather than a sentence. Beside it, `delivered` lists under each open milestone the issues the trunk's history shows merged, released or deployed while their status is not DONE, each with its proving commit and the remedy, so the milestone named as now is read together with its drift; `delivery` names the trunk commit read (`plan.delivery` at the trunk head, history and release records only). Cached per checkout state."
weight = 172
slug = "plan-roadmap"
[extra]
id = "plan.roadmap"
source = "apps/majordomus-cli/src/capability/builtin/plan.rs"
+++
