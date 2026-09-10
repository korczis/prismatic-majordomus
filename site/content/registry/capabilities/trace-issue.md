+++
title = "trace.issue"
description = "One issue with the branches that name it — local, and remote-tracking where only the remote still has the branch — and, for each, the commits it holds that the trunk did not: measured against the trunk while the branch is open, and against the first parent of the merge commit that brought it in once it is merged. A branch that reached the trunk without a merge commit of its own says so and claims nothing, because its commits cannot be told from the trunk's. The milestone comes from the canonical issue record, which is the one edge here that git does not hold, and `declared` says whether the project model has this id at all — a repository with no plan still gets the branches, and a typo still cannot read as work nobody did."
weight = 68
slug = "trace-issue"
[extra]
id = "trace.issue"
source = "apps/majordomus-cli/src/capability/builtin/trace.rs"
+++
