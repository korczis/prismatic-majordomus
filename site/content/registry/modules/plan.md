+++
title = "The plan and its derivations"
description = "The milestone and issue model of this repository, and everything derived from it that nobody authored: the status of each record, the dependency graphs above and below the milestone boundary, the topological execution waves, the roadmap order, the milestone being executed and the one issue to take next. Status is never stored — a record says what happened to it and the status follows from that and from the state of its dependencies — so no file can contradict the graph. This capability reads; writing a lifecycle marker into a record is `plan.transition`, which declares that it writes the repository."
weight = 27
slug = "plan"
[extra]
id = "plan"
source = "apps/majordomus-cli/src/capability/builtin/plan.rs"
+++
