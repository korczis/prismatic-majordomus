+++
title = "plan.roadmap"
description = "The milestone graph laid out by rank, with `order` breaking ties inside a rank only, and the first unblocked unfinished milestone as `now` and the one after it as `next`. Nothing in the sequence is authored: a milestone whose prerequisites are not real cannot be nominated, which is what makes `each step is gated by the previous one being real` an invariant rather than a sentence."
weight = 49
slug = "plan-roadmap"
[extra]
id = "plan.roadmap"
source = "apps/majordomus-cli/src/capability/builtin/plan.rs"
+++
