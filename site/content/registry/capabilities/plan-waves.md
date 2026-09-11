+++
title = "plan.waves"
description = "The topological layering of the issue graph: an issue enters a wave only once every dependency has left it, so its wave is one past the longest path to it. Sharing a wave is a necessary condition for running two issues at once, not a sufficient one — overlapping scope serialises them, and every such overlap is reported beside the waves rather than left for two workers to discover in a merge conflict."
weight = 68
slug = "plan-waves"
[extra]
id = "plan.waves"
source = "apps/majordomus-cli/src/capability/builtin/plan.rs"
+++
