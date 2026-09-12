+++
title = "Completion gates"
description = "Completion as a state the repository decides rather than a claim a worker makes: the validation gates this repository declares in its CI model, which of them a task's own change set selects, what each one last reported, whether that verdict still describes the tree it was taken over, and therefore whether the task may be called finished. A gate that has never reported is `queued` and not `pass`, a run whose files have changed since is `stale` and refuses exactly as a failure does, and a gate the change cannot affect is `exempt` rather than unknown."
weight = 16
slug = "gates"
[extra]
id = "gates"
source = "apps/majordomus-cli/src/capability/builtin/gates.rs"
+++
